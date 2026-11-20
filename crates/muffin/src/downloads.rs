//! Bounded download bookkeeping for native WebView download callbacks.
//!
//! The browser UI only receives `DownloadItem`; URLs and filesystem paths stay
//! in this native module. WebView completion callbacks identify transfers by
//! URL only. Duplicate normalized URLs are rejected at begin. A late completion
//! callback for an earlier rejected duplicate can still be ambiguous; callers
//! must not reuse a URL while an earlier callback may still arrive.

use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use url::Url;

const MAX_PENDING: usize = 16;
const MAX_RECENT: usize = 64;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Downloading,
    Finished,
    Failed,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DownloadItem {
    pub id: u32,
    pub name: String,
    pub status: DownloadStatus,
    pub bytes: u64,
}

#[derive(Debug)]
struct Record {
    item: DownloadItem,
    url: String,
    staged_path: PathBuf,
    stage: Option<TempDir>,
    completed_path: Option<PathBuf>,
}

pub struct Downloads {
    root: PathBuf,
    canonical_root: Option<PathBuf>,
    records: Vec<Record>,
    next_id: u32,
}

impl Downloads {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            canonical_root: None,
            records: Vec::new(),
            next_id: 1,
        }
    }

    /// Allocate a unique, not-yet-existing target under a private transfer
    /// directory. Only credential-free HTTP(S) URLs are accepted.
    pub fn begin(&mut self, url: &str, suggested: &Path) -> Option<PathBuf> {
        if !self.root.is_absolute() {
            return None;
        }
        if self
            .records
            .iter()
            .filter(|r| r.item.status == DownloadStatus::Downloading)
            .count()
            >= MAX_PENDING
        {
            return None;
        }
        let parsed = Url::parse(url).ok()?;
        if !["http", "https"].contains(&parsed.scheme())
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return None;
        }
        let normalized = parsed.to_string();
        if self
            .records
            .iter()
            .any(|r| r.item.status == DownloadStatus::Downloading && r.url == normalized)
        {
            return None;
        }
        let root = self.ensure_root()?.to_path_buf();
        let name = safe_name(suggested);
        let stage = tempfile::Builder::new()
            .prefix(".muffin-download-")
            .tempdir_in(&root)
            .ok()?;
        let staged_path = stage.path().join(name.as_str());
        if fs::symlink_metadata(&staged_path).is_ok() {
            return None;
        }
        let id = self.allocate_id();
        self.records.push(Record {
            item: DownloadItem {
                id,
                name,
                status: DownloadStatus::Downloading,
                bytes: 0,
            },
            url: normalized,
            staged_path: staged_path.clone(),
            stage: Some(stage),
            completed_path: None,
        });
        self.trim_recent();
        Some(staged_path)
    }

    /// Complete a transfer identified by its URL. WRY currently supplies no
    /// unique transfer identifier, so duplicate pending URLs are rejected.
    pub fn finish(&mut self, url: &str, success: bool) {
        let Ok(parsed) = Url::parse(url) else { return };
        let normalized = parsed.to_string();
        let Some(index) = self
            .records
            .iter()
            .position(|r| r.item.status == DownloadStatus::Downloading && r.url == normalized)
        else {
            return;
        };
        let published = success.then(|| self.publish_file(index)).flatten();
        if let Some((path, bytes)) = published {
            let record = &mut self.records[index];
            record.item.status = DownloadStatus::Finished;
            record.item.bytes = bytes;
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                record.item.name = name.to_owned();
            }
            record.completed_path = Some(path);
            record.stage.take();
        } else {
            self.fail_at(index);
        }
        self.trim_recent();
    }

    pub fn items(&self) -> Vec<DownloadItem> {
        self.records.iter().map(|r| r.item.clone()).collect()
    }

    pub fn completed_path(&self, id: u32) -> Option<&Path> {
        self.records
            .iter()
            .find(|r| r.item.id == id && r.item.status == DownloadStatus::Finished)
            .and_then(|r| r.completed_path.as_deref())
    }

    fn ensure_root(&mut self) -> Option<&Path> {
        if self.canonical_root.is_none() {
            if fs::create_dir_all(&self.root).is_err() {
                return None;
            }
            let meta = fs::symlink_metadata(&self.root).ok()?;
            if meta.file_type().is_symlink() || !meta.is_dir() {
                return None;
            }
            self.canonical_root = Some(fs::canonicalize(&self.root).ok()?);
        }
        self.canonical_root.as_deref()
    }

    fn allocate_id(&mut self) -> u32 {
        let id = self.next_id.max(1);
        self.next_id = id.wrapping_add(1).max(1);
        id
    }

    fn publish_file(&self, index: usize) -> Option<(PathBuf, u64)> {
        let root = self.canonical_root.as_deref()?;
        let record = &self.records[index];
        let stage = record.stage.as_ref()?;
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
        if !canonical_dir.starts_with(root) || record.staged_path.parent() != Some(stage_path) {
            return None;
        }
        let Ok(meta) = fs::symlink_metadata(&record.staged_path) else {
            return None;
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            return None;
        }
        let bytes = meta.len();
        for suffix in 0..10_000u32 {
            let visible = root.join(collision_name(&record.item.name, suffix));
            match fs::hard_link(&record.staged_path, &visible) {
                Ok(()) => return Some((visible, bytes)),
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
            if let Some(index) = self
                .records
                .iter()
                .position(|r| r.item.status != DownloadStatus::Downloading)
            {
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

    #[test]
    fn rejects_unsafe_urls_and_sanitizes_filename() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        assert!(downloads
            .begin("file:///etc/passwd", Path::new("passwd"))
            .is_none());
        assert!(downloads
            .begin("https://user:pass@example.test/a", Path::new("a"))
            .is_none());
        let path = downloads
            .begin("https://example.test/a", Path::new("../../bad name?.txt"))
            .unwrap();
        assert_eq!(path.file_name().unwrap(), "bad_name_.txt");
        assert!(path.starts_with(fs::canonicalize(dir.path()).unwrap()));
    }

    #[test]
    fn collisions_get_distinct_destinations_and_success_is_retained() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let a = downloads
            .begin("https://example.test/a", Path::new("report.pdf"))
            .unwrap();
        let b = downloads
            .begin("https://example.test/b", Path::new("report.pdf"))
            .unwrap();
        assert_ne!(a, b);
        File::create(&a).unwrap().write_all(b"pdf").unwrap();
        downloads.finish("https://example.test/a", true);
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
        downloads.finish("https://example.test/b", true);
        let second = downloads
            .items()
            .into_iter()
            .find(|i| i.name == "report.pdf" && i.id != item.id)
            .unwrap();
        assert_eq!(
            downloads.completed_path(second.id).unwrap(),
            fs::canonicalize(dir.path().join("downloads"))
                .unwrap()
                .join("report (1).pdf")
        );
    }

    #[test]
    fn failure_missing_file_rejected_symlink_and_duplicate_url_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        let failed = downloads
            .begin("https://example.test/fail", Path::new("partial"))
            .unwrap();
        File::create(&failed)
            .unwrap()
            .write_all(b"partial")
            .unwrap();
        downloads.finish("https://example.test/fail", false);
        assert!(!failed.exists());

        let missing = downloads
            .begin("https://example.test/missing", Path::new("gone"))
            .unwrap();
        downloads.finish("https://example.test/missing", true);
        assert!(!missing.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = downloads
                .begin("https://example.test/link", Path::new("link"))
                .unwrap();
            let outside = dir.path().join("outside");
            File::create(&outside).unwrap().write_all(b"keep").unwrap();
            symlink(&outside, &link).unwrap();
            downloads.finish("https://example.test/link", true);
            assert!(!link.exists());
            assert_eq!(fs::read(&outside).unwrap(), b"keep");
        }

        let x = downloads
            .begin("https://example.test/same", Path::new("one"))
            .unwrap();
        assert!(downloads
            .begin("https://example.test/same", Path::new("two"))
            .is_none());
        File::create(&x).unwrap();
        downloads.finish("https://example.test/same", true);
        assert!(!x.exists());
        assert_eq!(
            downloads
                .items()
                .iter()
                .find(|i| i.name == "one")
                .unwrap()
                .status,
            DownloadStatus::Finished
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
            assert!(downloads
                .begin("https://example.test/a", Path::new("a"))
                .is_none());
        }
    }

    #[test]
    fn bounds_pending_and_recent_items() {
        let dir = tempfile::tempdir().unwrap();
        let mut downloads = Downloads::new(dir.path().join("downloads"));
        for n in 0..MAX_PENDING {
            let url = format!("https://example.test/pending/{n}");
            assert!(downloads.begin(&url, Path::new("pending.bin")).is_some());
        }
        assert_eq!(downloads.items().len(), MAX_PENDING);
        assert!(downloads
            .begin("https://example.test/overflow", Path::new("overflow"))
            .is_none());

        let mut recent = Downloads::new(dir.path().join("recent"));
        for n in 0..(MAX_RECENT + 2) {
            let url = format!("https://example.test/recent/{n}");
            let path = recent.begin(&url, Path::new("item.bin")).unwrap();
            File::create(path).unwrap();
            recent.finish(&url, true);
        }
        assert_eq!(recent.items().len(), MAX_RECENT);
    }
}
