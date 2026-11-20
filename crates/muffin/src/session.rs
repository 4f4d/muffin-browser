use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::tabs::SessionDocument;

const MAX_SESSION_BYTES: u64 = 1024 * 1024;
const MAX_WINDOWS: usize = 8;
const MAX_TABS_PER_WINDOW: usize = 100;

pub(crate) struct SessionStore {
    path: PathBuf,
}

#[derive(Debug)]
enum LoadError {
    Damaged(String),
    Unavailable(String),
}

impl SessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(Path::new(&home).join("Library/Application Support/Muffin/session.json"))
    }

    pub fn load_recovering(&self) -> Result<Option<SessionDocument>, String> {
        match self.load() {
            Ok(session) => Ok(session),
            Err(LoadError::Unavailable(error)) => Err(error),
            Err(LoadError::Damaged(error)) => {
                let parent = self.path.parent().ok_or("session path has no parent")?;
                let preservation = tempfile::Builder::new()
                    .prefix("damaged-session-")
                    .tempdir_in(parent)
                    .map_err(|error| error.to_string())?;
                fs::rename(&self.path, preservation.path().join("session.json"))
                    .map_err(|error| error.to_string())?;
                let _ = preservation.keep();
                eprintln!("Invalid session preserved for recovery: {error}");
                Ok(None)
            }
        }
    }

    fn load(&self) -> Result<Option<SessionDocument>, LoadError> {
        let file = match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&self.path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(LoadError::Unavailable(error.to_string())),
        };
        if !file
            .metadata()
            .map_err(|error| LoadError::Unavailable(error.to_string()))?
            .is_file()
        {
            return Err(LoadError::Unavailable(
                "session path is not a regular file".into(),
            ));
        }
        let mut bytes = Vec::new();
        file.take(MAX_SESSION_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| LoadError::Unavailable(error.to_string()))?;
        if bytes.len() as u64 > MAX_SESSION_BYTES {
            return Err(LoadError::Unavailable("session file exceeds 1 MiB".into()));
        }
        let decoded: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| LoadError::Damaged(error.to_string()))?;
        if decoded
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|version| version != 1)
        {
            return Err(LoadError::Unavailable("unsupported session version".into()));
        }
        let session: SessionDocument = serde_json::from_value(decoded)
            .map_err(|error| LoadError::Damaged(error.to_string()))?;
        validate(&session).map_err(LoadError::Damaged)?;
        Ok(Some(session))
    }

    pub fn save(&self, session: &SessionDocument) -> Result<(), String> {
        validate(session)?;
        let parent = self.path.parent().ok_or("session path has no parent")?;
        ensure_directories(parent)?;
        let bytes = serde_json::to_vec(session).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SESSION_BYTES {
            return Err("session file exceeds 1 MiB".into());
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temporary.write_all(&bytes).map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary
            .persist(&self.path)
            .map_err(|e| e.error.to_string())?;
        Ok(())
    }
}

fn validate(session: &SessionDocument) -> Result<(), String> {
    if session.version != 1 || session.windows.len() > MAX_WINDOWS {
        return Err("unsupported session version or too many windows".into());
    }
    for window in &session.windows {
        if window.tabs.is_empty() || window.tabs.len() > MAX_TABS_PER_WINDOW {
            return Err("invalid tab count in session".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut found_selected = false;
        let mut encountered_unpinned = false;
        for tab in &window.tabs {
            if tab.id == 0
                || tab.id >= u32::MAX - MAX_TABS_PER_WINDOW as u32
                || tab.title.len() > 512
                || tab.url.len() > 4096
                || !valid_url(&tab.url)
                || !ids.insert(tab.id)
            {
                return Err("invalid tab metadata in session".into());
            }
            if encountered_unpinned && tab.pinned {
                return Err("pinned tabs must precede unpinned tabs".into());
            }
            encountered_unpinned |= !tab.pinned;
            found_selected |= tab.id == window.selected_tab;
        }
        if !found_selected {
            return Err("selected tab is missing from session".into());
        }
    }
    Ok(())
}

fn valid_url(value: &str) -> bool {
    if value == "about:blank" {
        return true;
    }
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url.has_host()
        && url.username().is_empty()
        && url.password().is_none()
        && !value.chars().any(char::is_control)
}

fn ensure_directories(path: &Path) -> Result<(), String> {
    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        missing.push(current.to_path_buf());
        current = current.parent().ok_or("storage directory has no parent")?;
    }
    let metadata = fs::symlink_metadata(current).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("storage path contains a symlink or non-directory".into());
    }
    for directory in missing.into_iter().rev() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(&directory).map_err(|e| e.to_string())?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let metadata = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("storage path contains a symlink or non-directory".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::{SavedTab, SavedWindow};

    #[test]
    fn save_load_round_trip_persists_only_navigation_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let store = SessionStore::new(directory.path().join("session.json"));
        let session = SessionDocument {
            version: 1,
            windows: vec![SavedWindow {
                tabs: vec![SavedTab {
                    id: 5,
                    title: "Saved title".into(),
                    url: "https://example.test/path".into(),
                    pinned: true,
                }],
                selected_tab: 5,
            }],
        };
        store.save(&session).unwrap();
        assert_eq!(store.load().unwrap(), Some(session));
        let text = fs::read_to_string(directory.path().join("session.json")).unwrap();
        assert!(!text.contains("cookie"));
        assert!(!text.contains("localStorage"));
    }

    #[test]
    fn invalid_or_oversized_sessions_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        fs::write(&path, vec![b' '; (MAX_SESSION_BYTES + 1) as usize]).unwrap();
        assert!(SessionStore::new(path).load().is_err());
    }

    #[test]
    fn corrupt_session_is_preserved_while_future_versions_stay_in_place() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let store = SessionStore::new(path.clone());
        let damaged = b"{truncated";
        fs::write(&path, damaged).unwrap();
        assert!(store.load_recovering().unwrap().is_none());
        assert!(!path.exists());
        let preserved = fs::read_dir(directory.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(fs::read(preserved.join("session.json")).unwrap(), damaged);
        let future = br#"{"version":2,"windows":[]}"#;
        fs::write(&path, future).unwrap();
        assert!(store.load_recovering().is_err());
        assert_eq!(fs::read(path).unwrap(), future);
    }
}
