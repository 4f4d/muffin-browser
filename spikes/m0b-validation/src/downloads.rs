use crate::policy::Policy;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
};
use tempfile::Builder;

type Cancel = Box<dyn FnOnce()>;

pub type SharedDownloads = Rc<RefCell<Downloads>>;

struct Transfer {
    path: PathBuf,
    cancel: Option<Cancel>,
    received: Option<u64>,
}

pub struct Downloads {
    folder: PathBuf,
    next: u32,
    active: BTreeMap<u32, Transfer>,
    finished: u32,
    failures: u32,
    notices: bool,
    // WKDownload's completion callback omits its destination. One in-flight
    // request per opaque key avoids ambiguous completion, including across tabs.
    keys: BTreeMap<[u8; 32], u32>,
}

impl Downloads {
    pub fn new(folder: PathBuf, notices: bool) -> Self {
        Self {
            folder,
            notices,
            next: 0,
            active: BTreeMap::new(),
            finished: 0,
            failures: 0,
            keys: BTreeMap::new(),
        }
    }

    pub fn begin(
        &mut self,
        policy: &Policy,
        url: &str,
        suggested: &Path,
    ) -> Option<(u32, PathBuf)> {
        if !policy.accepts(url) || !(url.starts_with("http://") || url.starts_with("https://")) {
            // URLs from the engine normally use lowercase schemes. Fail closed otherwise.
            return None;
        }
        let key = Sha256::digest(url.as_bytes()).into();
        if self.active.len() >= 12 || self.keys.contains_key(&key) {
            return None;
        }
        let id = self.next.checked_add(1)?;
        let name = safe_name(
            suggested
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(""),
        );
        // Atomically allocate a directory first. WKDownload needs a destination file
        // that does not exist and must never overwrite an existing user file.
        let directory = Builder::new()
            .prefix("muffin-")
            .tempdir_in(&self.folder)
            .ok()?;
        let path = directory.keep().join(name);
        self.next = id;
        self.keys.insert(key, id);
        self.active.insert(
            id,
            Transfer {
                path: path.clone(),
                cancel: None,
                received: None,
            },
        );
        if self.notices {
            println!("download_started id={id}");
        }
        Some((id, path))
    }

    #[cfg(target_os = "linux")]
    pub fn diagnostics_enabled(&self) -> bool {
        self.notices
    }

    pub fn set_folder(&mut self, folder: PathBuf) {
        self.folder = folder;
    }

    #[cfg(not(target_os = "macos"))]
    pub fn track(&mut self, id: u32, cancel: Cancel) {
        if let Some(transfer) = self.active.get_mut(&id) {
            transfer.cancel = Some(cancel);
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn progress(&mut self, id: u32, bytes: u64) {
        if let Some(transfer) = self.active.get_mut(&id) {
            transfer.received = Some(bytes);
        }
    }

    #[cfg(target_os = "macos")]
    pub fn by_path(&self, path: &Path) -> Option<u32> {
        self.active
            .iter()
            .find_map(|(id, transfer)| (transfer.path == path).then_some(*id))
    }

    #[cfg(target_os = "macos")]
    pub fn by_url(&self, url: &str) -> Option<u32> {
        let key: [u8; 32] = Sha256::digest(url.as_bytes()).into();
        self.keys.get(&key).copied()
    }

    pub fn complete(&mut self, id: u32, success: bool) {
        if self.active.remove(&id).is_some() {
            self.keys.retain(|_, entry| *entry != id);
            if success {
                self.finished += 1;
            } else {
                self.failures += 1;
            }
            if self.notices {
                println!("download_finished id={id} success={success}");
            }
        }
    }

    pub fn interrupted(&self) -> u32 {
        self.failures
    }

    pub fn native_bytes(&self) -> u64 {
        self.active
            .values()
            .filter_map(|transfer| transfer.received)
            .sum()
    }

    pub fn finished(&self) -> u32 {
        self.finished
    }

    pub fn cancelable(&self) -> bool {
        self.active
            .values()
            .any(|transfer| transfer.cancel.is_some())
    }

    pub fn take_cancellations(&mut self) -> Vec<Cancel> {
        self.active
            .values_mut()
            .filter_map(|transfer| transfer.cancel.take())
            .collect()
    }

    pub fn status(&self) -> String {
        if !self.active.is_empty() {
            let bytes = self
                .active
                .values()
                .map(|transfer| {
                    transfer
                        .received
                        .or_else(|| transfer.path.metadata().ok().map(|data| data.len()))
                        .unwrap_or(0)
                })
                .sum::<u64>();
            format!(
                "Downloads active: {} · {} KiB received",
                self.active.len(),
                bytes / 1024
            )
        } else if self.finished != 0 || self.failures != 0 {
            format!(
                "Downloads finished: {} · interrupted: {}",
                self.finished, self.failures
            )
        } else {
            String::new()
        }
    }
}

fn safe_name(suggested: &str) -> String {
    // A constant prefix avoids Windows reserved device names. Keep one safe
    // suffix, strip whitespace/control characters, and never accept separators.
    let name: String = suggested
        .chars()
        .take(96)
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let name = name.trim_matches(['.', '_', '-']);
    if name.is_empty() {
        "download.bin".into()
    } else {
        format!("download-{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_names_are_data_not_paths() {
        assert_eq!(safe_name("../CON.txt"), "download-CON.txt");
        assert_eq!(safe_name(r"..\a:b.txt"), "download-a_b.txt");
        assert_eq!(safe_name(".."), "download.bin");
        assert!(safe_name(&"a".repeat(200)).len() <= 105);
    }

    #[test]
    fn destination_is_unique_and_completion_releases_the_request() {
        let root = tempfile::tempdir().unwrap();
        let mut transfers = Downloads::new(root.path().into(), false);
        let url = "https://example.test/file";
        let policy = Policy::browsing();
        let (id, first) = transfers
            .begin(&policy, url, Path::new("sample.txt"))
            .unwrap();
        assert!(first.starts_with(root.path()) && !first.exists());
        assert!(transfers
            .begin(&policy, url, Path::new("sample.txt"))
            .is_none());
        transfers.complete(id, true);
        let (next, second) = transfers
            .begin(&policy, url, Path::new("sample.txt"))
            .unwrap();
        assert_ne!(id, next);
        assert_ne!(first, second);
        assert_eq!(first.file_name(), second.file_name());
        assert!(transfers
            .begin(&policy, "file:///my-file", Path::new("sample"))
            .is_none());
    }
}
