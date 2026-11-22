use std::{fs, os::unix::fs::MetadataExt, path::Path, process::Command};

pub fn open(path: &Path, identity: (u64, u64)) -> Result<(), &'static str> {
    let viewer = viewer_for(path, identity)?;
    Command::new("/usr/bin/open")
        .args(["-b", viewer])
        .arg(path)
        .spawn()
        .map_err(|_| "Muffin couldn’t open this file.")?;
    Ok(())
}

fn viewer_for(path: &Path, identity: (u64, u64)) -> Result<&'static str, &'static str> {
    let changed = "This downloaded file was moved or replaced.";
    let metadata = fs::symlink_metadata(path).map_err(|_| changed)?;
    if !metadata.is_file()
        || (metadata.dev(), metadata.ino()) != identity
        || fs::canonicalize(path).ok().as_deref() != Some(path)
    {
        return Err(changed);
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if metadata.mode() & 0o111 != 0 {
        return Err("Use Show in folder to review this file.");
    }
    let viewer = match extension.to_ascii_lowercase().as_str() {
        "txt" => "com.apple.TextEdit",
        "pdf" | "png" | "jpg" | "jpeg" | "gif" | "tif" | "tiff" | "heic" => "com.apple.Preview",
        _ => return Err("Use Show in folder to review this file."),
    };
    Ok(viewer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn only_opens_the_original_regular_document_in_its_viewer() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let saved = root.join("receipt.TXT");
        fs::write(&saved, "download fixture").unwrap();
        let metadata = fs::metadata(&saved).unwrap();
        let identity = (metadata.dev(), metadata.ino());
        assert_eq!(viewer_for(&saved, identity), Ok("com.apple.TextEdit"));

        fs::set_permissions(&saved, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(viewer_for(&saved, identity).is_err());
        fs::set_permissions(&saved, fs::Permissions::from_mode(0o600)).unwrap();

        let installer = root.join("installer.pkg");
        fs::hard_link(&saved, &installer).unwrap();
        assert!(viewer_for(&installer, identity).is_err());

        let original = root.join("original.txt");
        fs::rename(&saved, &original).unwrap();
        symlink(&original, &saved).unwrap();
        assert!(viewer_for(&saved, identity).is_err());
        fs::remove_file(&saved).unwrap();
        fs::write(&saved, "different file").unwrap();
        assert!(viewer_for(&saved, identity).is_err());
    }
}
