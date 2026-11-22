//! Bounded, consent-gated downloads. UI records contain no URL or path.

use serde::Serialize;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use url::Url;

const MAX_PENDING: usize = 16;
const MAX_RECENT: usize = 64;
pub(crate) const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    AwaitingApproval,
    Downloading,
    Finished,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    fn is_active(self) -> bool {
        matches!(self, Self::AwaitingApproval | Self::Downloading)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DownloadItem {
    pub id: u32,
    pub name: String,
    pub host: String,
    pub status: DownloadStatus,
    pub bytes: u64,
    pub retryable: bool,
}

#[derive(Debug)]
struct Record {
    item: DownloadItem,
    staged_path: Option<PathBuf>,
    stage: Option<TempDir>,
    destination_root: Option<PathBuf>,
    completed_path: Option<PathBuf>,
    completed_identity: Option<(u64, u64)>,
}

pub struct Downloads {
    default_root: PathBuf,
    canonical_default_root: Option<PathBuf>,
    custom_root: Option<PathBuf>,
    records: Vec<Record>,
    next_id: u32,
}

impl Downloads {
    pub fn new(root: PathBuf) -> Self {
        Self {
            default_root: root,
            canonical_default_root: None,
            custom_root: None,
            records: Vec::new(),
            next_id: 1,
        }
    }

    pub fn set_custom_root(&mut self, root: Option<PathBuf>) {
        self.custom_root = root;
    }

    pub fn root(&self) -> &Path {
        self.custom_root.as_deref().unwrap_or(&self.default_root)
    }

    pub fn is_custom_root(&self) -> bool {
        self.custom_root.is_some()
    }

    /// Return the canonical path for an existing, non-symlink folder usable by WKDownload.
    pub fn canonical_folder(path: &Path) -> Option<PathBuf> {
        if !path.is_absolute() || path.to_str().is_none() {
            return None;
        }
        let metadata = fs::symlink_metadata(path).ok()?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return None;
        }
        let canonical = fs::canonicalize(path).ok()?;
        canonical.to_str()?;
        Some(canonical)
    }

    /// Record an engine-proposed transfer without touching the filesystem.
    /// The returned ID is the internal object identity carried to completion.
    pub fn offer(&mut self, url: &Url, suggested: &Path) -> Option<u32> {
        if !["http", "https"].contains(&url.scheme())
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || self
                .records
                .iter()
                .filter(|r| r.item.status.is_active())
                .count()
                >= MAX_PENDING
        {
            return None;
        }
        let id = self.allocate_id();
        self.records.push(Record {
            item: DownloadItem {
                id,
                name: safe_name(suggested),
                host: safe_host(url),
                status: DownloadStatus::AwaitingApproval,
                bytes: 0,
                retryable: false,
            },
            staged_path: None,
            stage: None,
            destination_root: None,
            completed_path: None,
            completed_identity: None,
        });
        self.trim_recent();
        Some(id)
    }

    /// Allocate a randomized private staging destination only after consent.
    pub fn approve(&mut self, id: u32) -> Option<PathBuf> {
        let index = self
            .records
            .iter()
            .position(|r| r.item.id == id && r.item.status == DownloadStatus::AwaitingApproval)?;
        let prepared = (|| {
            let root = self.root_for_approval()?;
            let stage = tempfile::Builder::new()
                .prefix(".muffin-download-")
                .tempdir_in(&root)
                .ok()?;
            let staged_path = stage.path().join(self.records[index].item.name.as_str());
            if fs::symlink_metadata(&staged_path).is_ok() {
                return None;
            }
            Some((root, stage, staged_path))
        })();
        if let Some((root, stage, path)) = prepared {
            let record = &mut self.records[index];
            record.staged_path = Some(path.clone());
            record.stage = Some(stage);
            record.destination_root = Some(root);
            record.item.status = DownloadStatus::Downloading;
            Some(path)
        } else {
            self.records[index].item.status = DownloadStatus::Failed;
            self.trim_recent();
            None
        }
    }

    pub fn dismiss(&mut self, id: u32) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|r| r.item.id == id && r.item.status == DownloadStatus::AwaitingApproval)
        else {
            return false;
        };
        record.item.status = DownloadStatus::Cancelled;
        self.trim_recent();
        true
    }

    /// Complete the exact offered transfer. Unknown and terminal IDs are no-ops.
    pub fn finish(&mut self, id: u32, success: bool) {
        let Some(index) = self
            .records
            .iter()
            .position(|r| r.item.id == id && r.item.status == DownloadStatus::Downloading)
        else {
            return;
        };
        let published = success.then(|| self.publish_file(index)).flatten();
        if let Some((path, bytes, identity)) = published {
            let record = &mut self.records[index];
            record.item.status = DownloadStatus::Finished;
            record.item.bytes = bytes;
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                record.item.name = name.to_owned();
            }
            record.completed_path = Some(path);
            record.completed_identity = Some(identity);
            record.stage.take();
        } else {
            self.fail_at(index);
        }
        self.trim_recent();
    }

    pub fn mark_retryable(&mut self, id: u32, retryable: bool) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.item.id == id && record.item.status == DownloadStatus::Failed)
        else {
            return false;
        };
        record.item.retryable = retryable;
        true
    }

    /// Consume the user's one-shot retry capability before starting a new request.
    pub fn consume_retry(&mut self, id: u32) -> bool {
        let Some(record) = self.records.iter_mut().find(|record| {
            record.item.id == id
                && record.item.status == DownloadStatus::Failed
                && record.item.retryable
        }) else {
            return false;
        };
        record.item.retryable = false;
        true
    }

    pub fn is_retryable(&self, id: u32) -> bool {
        self.records
            .iter()
            .any(|record| record.item.id == id && record.item.retryable)
    }

    pub fn items(&self) -> Vec<DownloadItem> {
        self.records.iter().map(|r| r.item.clone()).collect()
    }

    pub fn update_progress(&mut self, id: u32, bytes: u64) -> bool {
        let Some(record) = self.records.iter_mut().find(|record| {
            record.item.id == id && record.item.status == DownloadStatus::Downloading
        }) else {
            return false;
        };
        let bytes = bytes.min(MAX_FILE_BYTES);
        if bytes <= record.item.bytes {
            return false;
        }
        record.item.bytes = bytes;
        true
    }

    pub fn cancel_transfer(&mut self, id: u32) -> bool {
        let Some(record) = self.records.iter_mut().find(|record| {
            record.item.id == id && record.item.status == DownloadStatus::Downloading
        }) else {
            return false;
        };
        record.stage.take();
        record.item.status = DownloadStatus::Cancelled;
        self.trim_recent();
        true
    }

    pub fn clear_terminal(&mut self) -> bool {
        let previous = self.records.len();
        self.records.retain(|record| record.item.status.is_active());
        self.records.len() != previous
    }

    pub fn completed_path(&self, id: u32) -> Option<&Path> {
        self.records
            .iter()
            .find(|r| r.item.id == id && r.item.status == DownloadStatus::Finished)
            .and_then(|r| r.completed_path.as_deref())
    }

    pub fn completed_identity(&self, id: u32) -> Option<(u64, u64)> {
        self.records
            .iter()
            .find(|record| record.item.id == id && record.item.status == DownloadStatus::Finished)
            .and_then(|record| record.completed_identity)
    }

    fn root_for_approval(&mut self) -> Option<PathBuf> {
        if let Some(custom) = &self.custom_root {
            return Self::canonical_folder(custom);
        }
        if self.canonical_default_root.is_none() {
            if !self.default_root.is_absolute() {
                return None;
            }
            if fs::create_dir_all(&self.default_root).is_err() {
                return None;
            }
            let meta = fs::symlink_metadata(&self.default_root).ok()?;
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return None;
            }
            let canonical = fs::canonicalize(&self.default_root).ok()?;
            canonical.to_str()?;
            self.canonical_default_root = Some(canonical);
        }
        self.canonical_default_root.clone()
    }

    fn allocate_id(&mut self) -> u32 {
        loop {
            let id = self.next_id.max(1);
            self.next_id = id.wrapping_add(1).max(1);
            if !self.records.iter().any(|r| r.item.id == id) {
                return id;
            }
        }
    }

    fn publish_file(&self, index: usize) -> Option<(PathBuf, u64, (u64, u64))> {
        let record = &self.records[index];
        let root = record.destination_root.as_deref()?;
        let stage = record.stage.as_ref()?;
        let staged_path = record.staged_path.as_deref()?;
        let stage_path = stage.path();
        let Ok(dir_meta) = fs::symlink_metadata(stage_path) else {
            return None;
        };
        if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() {
            return None;
        }
        let Ok(canonical_dir) = fs::canonicalize(stage_path) else {
            return None;
        };
        if !canonical_dir.starts_with(root) || staged_path.parent() != Some(stage_path) {
            return None;
        }
        let Ok(meta) = fs::symlink_metadata(staged_path) else {
            return None;
        };
        if !meta.is_file() {
            return None;
        }
        let bytes = meta.len();
        if bytes > MAX_FILE_BYTES {
            return None;
        }
        let identity = (meta.dev(), meta.ino());
        for suffix in 0..10_000u32 {
            let visible = root.join(collision_name(&record.item.name, suffix));
            match fs::hard_link(staged_path, &visible) {
                Ok(()) => return Some((visible, bytes, identity)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    }

    fn fail_at(&mut self, index: usize) {
        let record = &mut self.records[index];
        record.stage.take();
        record.item.status = DownloadStatus::Failed;
        record.item.bytes = 0;
    }

    fn trim_recent(&mut self) {
        while self.records.len() > MAX_RECENT {
            if let Some(index) = self.records.iter().position(|r| !r.item.status.is_active()) {
                self.records.remove(index);
            } else {
                break;
            }
        }
    }
}

fn safe_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let mut name: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(180)
        .collect();
    while name.starts_with('.') {
        name.remove(0);
    }
    while name.ends_with('.') || name.ends_with(' ') {
        name.pop();
    }
    if name.is_empty() || name == ".." {
        "download".into()
    } else {
        name
    }
}

fn safe_host(url: &Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    if let Some(port) = url.port() {
        let default = match url.scheme() {
            "http" => Some(80),
            "https" => Some(443),
            _ => None,
        };
        if Some(port) != default {
            return format!("{host}:{port}");
        }
    }
    host
}

fn collision_name(name: &str, suffix: u32) -> String {
    if suffix == 0 {
        return name.to_owned();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    format!("{stem} ({suffix}){ext}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn offer_is_fs_free_and_same_url_gets_distinct_ids() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        assert!(downloads
            .offer(&url("file:///etc/passwd"), Path::new("passwd"))
            .is_none());
        assert!(downloads
            .offer(&url("https://user:pass@example.test/a"), Path::new("a"))
            .is_none());
        let first = downloads
            .offer(
                &url("https://example.test:8443/a"),
                Path::new("../../bad name?.txt"),
            )
            .unwrap();
        let second = downloads
            .offer(&url("https://example.test:8443/a"), Path::new("same.txt"))
            .unwrap();
        assert_ne!(first, second);
        assert!(!dir.path().join("downloads").exists());
        let rows = downloads.items();
        assert_eq!(rows[0].host, "example.test:8443");
        assert_eq!(rows[0].name, "bad_name_.txt");
        assert_eq!(rows[0].status, DownloadStatus::AwaitingApproval);
    }

    #[test]
    fn progress_cancel_and_clear_preserve_pending_transfers_and_published_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let address = url("https://example.test/download");
        let pending = downloads.offer(&address, Path::new("review.txt")).unwrap();
        let cancelled = downloads.offer(&address, Path::new("partial.txt")).unwrap();
        let finished = downloads.offer(&address, Path::new("keep.txt")).unwrap();
        let partial = downloads.approve(cancelled).unwrap();
        fs::write(&partial, b"12").unwrap();
        assert!(downloads.update_progress(cancelled, 2));
        assert!(!downloads.update_progress(cancelled, 1));
        assert!(!downloads.update_progress(pending, 1));
        assert_eq!(downloads.items()[1].bytes, 2);

        let complete = downloads.approve(finished).unwrap();
        fs::write(complete, b"keep").unwrap();
        downloads.finish(finished, true);
        let published = downloads.completed_path(finished).unwrap().to_path_buf();
        let metadata = fs::symlink_metadata(&published).unwrap();
        assert_eq!(
            downloads.completed_identity(finished),
            Some((metadata.dev(), metadata.ino()))
        );

        assert!(downloads.clear_terminal());
        assert_eq!(downloads.items().len(), 2);
        assert_eq!(fs::read(&published).unwrap(), b"keep");
        assert!(downloads.cancel_transfer(cancelled));
        downloads.finish(cancelled, true);
        assert!(!downloads.cancel_transfer(cancelled));
        assert!(!downloads.update_progress(cancelled, 3));
        assert!(!partial.exists());
        assert_eq!(downloads.items()[1].status, DownloadStatus::Cancelled);
        assert!(downloads.clear_terminal());
        assert!(!downloads.clear_terminal());
        assert_eq!(downloads.items()[0].id, pending);
    }

    #[test]
    fn collisions_get_distinct_destinations_and_success_is_retained() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let ida = downloads
            .offer(&url("https://example.test/a"), Path::new("report.pdf"))
            .unwrap();
        let idb = downloads
            .offer(&url("https://example.test/b"), Path::new("report.pdf"))
            .unwrap();
        let a = downloads.approve(ida).unwrap();
        let b = downloads.approve(idb).unwrap();
        assert_ne!(a, b);
        File::create(&a).unwrap().write_all(b"pdf").unwrap();
        downloads.finish(ida, true);
        let item = downloads
            .items()
            .into_iter()
            .find(|i| i.status == DownloadStatus::Finished)
            .unwrap();
        assert_eq!(item.bytes, 3);
        let visible_a = downloads.completed_path(item.id).unwrap().to_path_buf();
        assert_eq!(
            visible_a,
            fs::canonicalize(dir.path().join("downloads"))
                .unwrap()
                .join("report.pdf")
        );
        assert!(!a.exists());
        File::create(&b).unwrap();
        downloads.finish(idb, true);
        let second = downloads
            .items()
            .into_iter()
            .find(|i| i.id != item.id && i.status == DownloadStatus::Finished)
            .unwrap();
        assert_eq!(second.name, "report (1).pdf");
        assert_eq!(
            downloads.completed_path(second.id).unwrap(),
            fs::canonicalize(dir.path().join("downloads"))
                .unwrap()
                .join("report (1).pdf")
        );
    }

    #[test]
    fn approved_transfers_keep_the_root_selected_at_approval() {
        let dir = tempfile::tempdir().unwrap();
        let default = dir.path().join("default");
        let custom = dir.path().join("custom");
        fs::create_dir(&custom).unwrap();
        let mut downloads = Downloads::new(default.clone());
        let first = downloads
            .offer(&url("https://example.test/a"), Path::new("same.txt"))
            .unwrap();
        let first_stage = downloads.approve(first).unwrap();
        assert!(first_stage.starts_with(fs::canonicalize(&default).unwrap()));

        downloads.set_custom_root(Some(custom.clone()));
        assert!(downloads.is_custom_root());
        assert_eq!(downloads.root(), custom);
        let second = downloads
            .offer(&url("https://example.test/b"), Path::new("same.txt"))
            .unwrap();
        let second_stage = downloads.approve(second).unwrap();
        assert!(second_stage.starts_with(fs::canonicalize(&custom).unwrap()));

        fs::write(&first_stage, b"default payload").unwrap();
        fs::write(&second_stage, b"custom payload").unwrap();
        downloads.finish(first, true);
        downloads.finish(second, true);
        assert_eq!(
            fs::read(downloads.completed_path(first).unwrap()).unwrap(),
            b"default payload"
        );
        assert_eq!(
            fs::read(downloads.completed_path(second).unwrap()).unwrap(),
            b"custom payload"
        );
        assert!(downloads
            .completed_path(first)
            .unwrap()
            .starts_with(fs::canonicalize(default).unwrap()));
        assert!(downloads
            .completed_path(second)
            .unwrap()
            .starts_with(fs::canonicalize(custom).unwrap()));
    }

    #[test]
    fn custom_root_duplicate_and_existing_names_never_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let custom = dir.path().join("custom");
        fs::create_dir(&custom).unwrap();
        fs::write(custom.join("report.txt"), b"user file").unwrap();
        let mut downloads = Downloads::new(dir.path().join("default"));
        downloads.set_custom_root(Some(custom.clone()));

        let first = downloads
            .offer(&url("https://example.test/a"), Path::new("report.txt"))
            .unwrap();
        let second = downloads
            .offer(&url("https://example.test/b"), Path::new("report.txt"))
            .unwrap();
        for (id, payload) in [(first, b"first".as_slice()), (second, b"second".as_slice())] {
            let staged = downloads.approve(id).unwrap();
            fs::write(staged, payload).unwrap();
            downloads.finish(id, true);
        }
        assert_eq!(fs::read(custom.join("report.txt")).unwrap(), b"user file");
        assert_eq!(fs::read(custom.join("report (1).txt")).unwrap(), b"first");
        assert_eq!(fs::read(custom.join("report (2).txt")).unwrap(), b"second");
    }

    #[test]
    fn missing_custom_root_fails_approval_without_fallback_or_creation() {
        let dir = tempfile::tempdir().unwrap();
        let default = dir.path().join("default");
        let missing = dir.path().join("removed-custom-folder");
        let mut downloads = Downloads::new(default.clone());
        downloads.set_custom_root(Some(missing.clone()));
        let id = downloads
            .offer(&url("https://example.test/file"), Path::new("file"))
            .unwrap();
        assert!(downloads.approve(id).is_none());
        assert!(!missing.exists());
        assert!(!default.exists());
        assert_eq!(downloads.items()[0].status, DownloadStatus::Failed);
    }

    #[test]
    fn failure_missing_file_and_rejected_symlink_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let failed_id = downloads
            .offer(&url("https://example.test/fail"), Path::new("partial"))
            .unwrap();
        let failed = downloads.approve(failed_id).unwrap();
        File::create(&failed)
            .unwrap()
            .write_all(b"partial")
            .unwrap();
        downloads.finish(failed_id, false);
        assert!(!failed.exists());

        let missing_id = downloads
            .offer(&url("https://example.test/missing"), Path::new("gone"))
            .unwrap();
        let missing = downloads.approve(missing_id).unwrap();
        downloads.finish(missing_id, true);
        assert!(!missing.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link_id = downloads
                .offer(&url("https://example.test/link"), Path::new("link"))
                .unwrap();
            let link = downloads.approve(link_id).unwrap();
            let outside = dir.path().join("outside");
            File::create(&outside).unwrap().write_all(b"keep").unwrap();
            symlink(&outside, &link).unwrap();
            downloads.finish(link_id, true);
            assert!(!link.exists());
            assert_eq!(fs::read(&outside).unwrap(), b"keep");
        }

        assert_eq!(
            downloads
                .items()
                .iter()
                .find(|i| i.id == missing_id)
                .unwrap()
                .status,
            DownloadStatus::Failed
        );
    }

    #[test]
    fn refuses_symlinked_download_root() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("target");
            fs::create_dir(&target).unwrap();
            let link = dir.path().join("link");
            symlink(&target, &link).unwrap();
            let mut downloads = Downloads::new(link);
            let id = downloads
                .offer(&url("https://example.test/a"), Path::new("a"))
                .unwrap();
            assert!(downloads.approve(id).is_none());
            assert_eq!(downloads.items()[0].status, DownloadStatus::Failed);
        }
    }

    #[test]
    fn bounds_pending_and_recent_items() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        for n in 0..MAX_PENDING {
            let url = format!("https://example.test/pending/{n}");
            assert!(downloads
                .offer(&Url::parse(&url).unwrap(), Path::new("pending.bin"))
                .is_some());
        }
        assert_eq!(downloads.items().len(), MAX_PENDING);
        assert!(downloads
            .offer(&url("https://example.test/overflow"), Path::new("overflow"))
            .is_none());

        let mut recent = Downloads::new(dir.path().join("recent"));
        for n in 0..(MAX_RECENT + 2) {
            let url = format!("https://example.test/recent/{n}");
            let id = recent
                .offer(&Url::parse(&url).unwrap(), Path::new("item.bin"))
                .unwrap();
            let path = recent.approve(id).unwrap();
            File::create(path).unwrap();
            recent.finish(id, true);
        }
        assert_eq!(recent.items().len(), MAX_RECENT);
    }

    #[test]
    fn denial_dismissal_missing_id_and_finish_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("downloads");
        let mut downloads = Downloads::new(root.clone());
        let denied = downloads
            .offer(&url("https://example.test/deny"), Path::new("deny"))
            .unwrap();
        assert!(downloads.dismiss(denied));
        assert!(!downloads.dismiss(denied));
        downloads.finish(denied, true);
        assert!(!root.exists());
        let id = downloads
            .offer(&url("https://example.test/allow"), Path::new("allow"))
            .unwrap();
        assert!(downloads.approve(999_999).is_none());
        assert!(downloads.approve(id).is_some());
        downloads.finish(999_999, true);
        assert_eq!(
            downloads
                .items()
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .status,
            DownloadStatus::Downloading
        );
    }

    #[test]
    fn retryability_is_failure_only_and_consumed_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let failed = downloads
            .offer(&url("https://example.test/fail"), Path::new("fail"))
            .unwrap();
        let staged = downloads.approve(failed).unwrap();
        fs::write(staged, b"partial").unwrap();
        downloads.finish(failed, false);
        assert!(downloads.mark_retryable(failed, true));
        assert!(downloads.items()[0].retryable);
        assert!(downloads.consume_retry(failed));
        assert!(!downloads.consume_retry(failed));
        assert!(!downloads.items()[0].retryable);

        let success = downloads
            .offer(&url("https://example.test/success"), Path::new("ok"))
            .unwrap();
        let staged = downloads.approve(success).unwrap();
        fs::write(staged, b"ok").unwrap();
        downloads.finish(success, true);
        assert!(!downloads.mark_retryable(success, true));
        assert!(!downloads.items()[1].retryable);
        assert!(downloads.clear_terminal());
        assert!(!downloads.items().iter().any(|item| item.id == failed));
    }

    #[test]
    fn completed_file_size_limit_rejects_oversize_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let id = downloads
            .offer(&url("https://example.test/large"), Path::new("large.bin"))
            .unwrap();
        let stage = downloads.approve(id).unwrap();
        File::create(&stage)
            .unwrap()
            .set_len(MAX_FILE_BYTES + 1)
            .unwrap();
        downloads.finish(id, true);
        assert_eq!(downloads.items()[0].status, DownloadStatus::Failed);
        assert!(downloads.completed_path(id).is_none());
    }
}
