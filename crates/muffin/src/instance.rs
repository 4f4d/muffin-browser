//! Per-user macOS instance lock and bounded URL-forwarding socket.
//!
//! The endpoint lives in the application's owner-private support directory. Its
//! protocol accepts only versioned HTTP(S) URLs and never carries shell commands.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use url::Url;

const PROTOCOL_VERSION: u8 = 1;
const MAX_FRAME_BYTES: usize = 64 * 1024;
const MAX_URLS: usize = 16;
const MAX_URL_BYTES: usize = 4096;
const MAX_SOCKET_PATH_BYTES: usize = 103;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

const LOCK_FILE: &str = "instance.lock";
const SOCKET_FILE: &str = "instance.sock";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    urls: Vec<String>,
}

impl Request {
    fn from_urls(urls: Vec<Url>) -> Result<Self, String> {
        if urls.len() > MAX_URLS {
            return Err("too many URLs in instance-forward request".into());
        }
        let request = Self {
            version: PROTOCOL_VERSION,
            urls: urls.into_iter().map(|url| url.into()).collect(),
        };
        validate_request(&request)?;
        let encoded = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        if encoded.len() > MAX_FRAME_BYTES {
            return Err("instance-forward request exceeds its size limit".into());
        }
        Ok(request)
    }

    fn urls(&self) -> Result<Vec<Url>, String> {
        validate_request(self)?;
        self.urls
            .iter()
            .map(|raw| Url::parse(raw).map_err(|error| error.to_string()))
            .collect()
    }
}

fn validate_request(request: &Request) -> Result<(), String> {
    if request.version != PROTOCOL_VERSION || request.urls.len() > MAX_URLS {
        return Err("unsupported or oversized instance-forward request".into());
    }
    for raw in &request.urls {
        if raw.is_empty() || raw.len() > MAX_URL_BYTES || raw.chars().any(char::is_control) {
            return Err("invalid forwarded URL length or characters".into());
        }
        let parsed = Url::parse(raw).map_err(|error| error.to_string())?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("forwarded URLs must be credential-free HTTP(S) addresses".into());
        }
    }
    Ok(())
}

pub(crate) enum Instance {
    Forwarded,
    Primary(Primary),
}

pub(crate) struct Primary {
    socket_path: PathBuf,
    lock: File,
    stopping: std::sync::Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
}

impl Drop for Primary {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
        match fs::symlink_metadata(&self.socket_path) {
            Ok(metadata)
                if metadata.file_type().is_socket()
                    && metadata.uid() == unsafe { libc::getuid() } =>
            {
                let _ = fs::remove_file(&self.socket_path);
            }
            _ => {}
        }
        unsafe { libc::flock(self.lock.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub(crate) fn start(
    directory: &Path,
    urls: Vec<Url>,
    on_forward: impl Fn(Vec<Url>) -> bool + Send + 'static,
) -> Result<Instance, String> {
    let request = Request::from_urls(urls)?;
    if !directory.is_absolute() {
        return Err("application support directory must be absolute".into());
    }
    validate_private_directory(directory)?;
    let lock_path = directory.join(LOCK_FILE);
    let socket_path = directory.join(SOCKET_FILE);
    if socket_path.as_os_str().as_bytes().len() > MAX_SOCKET_PATH_BYTES {
        return Err("application support path is too long for a Unix-domain socket".into());
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&lock_path)
        .map_err(|error| format!("open instance lock: {error}"))?;
    validate_owned_regular_file(&lock)?;

    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
            forward_request(&socket_path, &request)?;
            return Ok(Instance::Forwarded);
        }
        return Err(format!("acquire instance lock: {error}"));
    }

    remove_owned_stale_socket(&socket_path)?;
    let listener = UnixListener::bind(&socket_path)
        .map_err(|error| format!("bind instance socket: {error}"))?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("secure instance socket: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("configure instance socket: {error}"))?;

    let stopping = std::sync::Arc::new(AtomicBool::new(false));
    let worker_stopping = stopping.clone();
    let server = thread::Builder::new()
        .name("muffin-instance-forward".into())
        .spawn(move || serve(listener, worker_stopping, on_forward))
        .map_err(|error| format!("start instance-forward listener: {error}"))?;
    Ok(Instance::Primary(Primary {
        socket_path,
        lock,
        stopping,
        server: Some(server),
    }))
}

fn validate_private_directory(directory: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("inspect application support directory: {error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(
            "application support directory must be an owner-owned private directory".into(),
        );
    }
    Ok(())
}

fn validate_owned_regular_file(file: &File) -> Result<(), String> {
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("instance lock must be an owner-owned private regular file".into());
    }
    Ok(())
}

fn remove_owned_stale_socket(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_socket() && metadata.uid() == unsafe { libc::getuid() } =>
        {
            fs::remove_file(path).map_err(|error| format!("remove stale instance socket: {error}"))
        }
        Ok(_) => Err("refusing to replace a non-owned or non-socket instance endpoint".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("inspect instance socket: {error}")),
    }
}

fn forward_request(socket_path: &Path, request: &Request) -> Result<(), String> {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut stream = loop {
        match UnixStream::connect(socket_path) {
            Ok(stream) => break stream,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(format!("connect to primary Muffin instance: {error}")),
        }
    };
    verify_peer_uid(&stream)?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
        .map_err(|error| format!("set instance-forward timeout: {error}"))?;
    let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    if payload.is_empty() || payload.len() > MAX_FRAME_BYTES {
        return Err("instance-forward request exceeds its size limit".into());
    }
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .and_then(|()| stream.write_all(&payload))
        .map_err(|error| format!("send instance-forward request: {error}"))?;
    let mut response = [0u8; 1];
    stream
        .read_exact(&mut response)
        .map_err(|error| format!("receive instance-forward acknowledgement: {error}"))?;
    if response[0] != 1 {
        return Err("primary Muffin instance rejected the forwarded request".into());
    }
    Ok(())
}

fn serve(
    listener: UnixListener,
    stopping: std::sync::Arc<AtomicBool>,
    on_forward: impl Fn(Vec<Url>) -> bool + Send + 'static,
) {
    while !stopping.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let valid = verify_peer_uid(&stream).is_ok()
                    && stream.set_nonblocking(false).is_ok()
                    && stream
                        .set_read_timeout(Some(IO_TIMEOUT))
                        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
                        .is_ok();
                if !valid {
                    continue;
                }
                match read_request(&mut stream).and_then(|request| request.urls()) {
                    Ok(urls) => {
                        let accepted = on_forward(urls);
                        let _ = stream.write_all(&[u8::from(accepted)]);
                    }
                    Err(_) => {
                        let _ = stream.write_all(&[0]);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(40));
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn read_request(stream: &mut UnixStream) -> Result<Request, String> {
    let mut header = [0u8; 4];
    stream
        .read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME_BYTES {
        return Err("invalid instance-forward frame length".into());
    }
    let mut payload = vec![0u8; length];
    stream
        .read_exact(&mut payload)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&payload).map_err(|error| error.to_string())
}

fn verify_peer_uid(stream: &UnixStream) -> Result<(), String> {
    let mut uid = 0u32;
    let mut gid = 0u32;
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0
        || uid != unsafe { libc::getuid() }
    {
        return Err("instance-forward peer does not belong to this user".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_accepts_web_addresses_and_an_empty_raise_request() {
        assert!(Request::from_urls(vec![Url::parse("https://example.test/path").unwrap()]).is_ok());
        assert!(Request::from_urls(Vec::new()).unwrap().urls.is_empty());
    }

    #[test]
    fn protocol_rejects_local_schemes_credentials_unknown_fields_and_limits() {
        for value in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:pass@example.test/",
            "data:text/plain,hello",
        ] {
            assert!(
                Request::from_urls(vec![Url::parse(value).unwrap()]).is_err(),
                "{value}"
            );
        }
        assert!(
            serde_json::from_str::<Request>(r#"{"version":1,"urls":[],"command":"quit"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<Request>(r#"{"version":2,"urls":[]}"#)
                .unwrap()
                .urls()
                .is_err()
        );
        assert!(Request::from_urls(
            (0..MAX_URLS + 1)
                .map(|_| Url::parse("https://example.test").unwrap())
                .collect()
        )
        .is_err());
    }

    #[test]
    fn private_directory_validation_rejects_symlinks_and_group_access() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            validate_private_directory(directory.path()).is_ok(),
            "{:?}",
            validate_private_directory(directory.path())
        );
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(validate_private_directory(directory.path()).is_err());
    }

    #[test]
    fn one_primary_accepts_only_same_user_bounded_forward_requests() {
        use std::sync::mpsc;

        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let (sender, receiver) = mpsc::channel();
        let primary = start(directory.path(), Vec::new(), move |urls| {
            sender.send(urls).is_ok()
        })
        .unwrap();
        assert!(matches!(primary, Instance::Primary(_)));

        let request = vec![Url::parse("https://example.test/a").unwrap()];
        let secondary = start(directory.path(), request.clone(), |_| true).unwrap();
        assert!(matches!(secondary, Instance::Forwarded));
        assert_eq!(receiver.recv_timeout(IO_TIMEOUT).unwrap(), request);
    }

    #[test]
    fn accepted_peer_can_send_a_valid_frame_after_connecting() {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let _primary = start(directory.path(), Vec::new(), |_| true).unwrap();
        let mut peer = UnixStream::connect(directory.path().join(SOCKET_FILE)).unwrap();
        peer.set_read_timeout(Some(IO_TIMEOUT)).unwrap();
        thread::sleep(Duration::from_millis(120));
        let payload = serde_json::to_vec(&Request::from_urls(Vec::new()).unwrap()).unwrap();
        peer.write_all(&(payload.len() as u32).to_be_bytes())
            .unwrap();
        peer.write_all(&payload).unwrap();
        let mut answer = [0];
        peer.read_exact(&mut answer).unwrap();
        assert_eq!(answer, [1]);
    }
}
