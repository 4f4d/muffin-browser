//! macOS WKWebView PDF export with bounded data and same-directory atomic save.

use std::{io::Write, path::PathBuf, ptr::NonNull};

const MAX_PDF_BYTES: usize = 128 * 1024 * 1024;

/// Render the current WKWebView page to PDF and atomically create `destination`.
/// The destination must not already exist. Errors deliberately omit its path.
pub fn save(
    view: &wry::WebView,
    destination: PathBuf,
    done: impl Fn(Result<(), String>) + 'static,
) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_foundation::{NSData, NSError};
    use objc2_web_kit::WKPDFConfiguration;
    use wry::WebViewExtMacOS;

    let Some(main_thread) = MainThreadMarker::new() else {
        done(Err("PDF export must start on the macOS main thread".into()));
        return;
    };
    let configuration = unsafe { WKPDFConfiguration::new(main_thread) };
    let callback = RcBlock::new(move |data: *mut NSData, error: *mut NSError| {
        if !error.is_null() || data.is_null() {
            done(Err("WebKit could not create a PDF".into()));
            return;
        }
        let Some(data) = (unsafe { data.as_ref() }) else {
            done(Err("WebKit returned no PDF data".into()));
            return;
        };
        let length = data.length();
        if length == 0 || length > MAX_PDF_BYTES {
            done(Err(
                "PDF output is empty or exceeds the 128 MiB limit".into()
            ));
            return;
        }
        let mut bytes = vec![0u8; length];
        let Some(buffer) = NonNull::new(bytes.as_mut_ptr().cast()) else {
            done(Err("Unable to allocate PDF output".into()));
            return;
        };
        unsafe { data.getBytes_length(buffer, length) };
        if !bytes.starts_with(b"%PDF-") || !bytes.windows(5).rev().any(|w| w == b"%%EOF") {
            done(Err("WebKit returned invalid PDF data".into()));
            return;
        }
        match publish(&destination, &bytes) {
            Ok(()) => done(Ok(())),
            Err(message) => done(Err(message)),
        }
    });

    unsafe {
        view.webview()
            .createPDFWithConfiguration_completionHandler(Some(&configuration), &callback);
    }
}

fn publish(destination: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "PDF destination has no parent directory".to_string())?;
    let meta = std::fs::symlink_metadata(parent)
        .map_err(|_| "PDF destination directory is unavailable".to_string())?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("PDF destination directory is invalid".into());
    }
    if std::fs::symlink_metadata(destination).is_ok() {
        return Err("PDF destination already exists".into());
    }
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| "Unable to stage PDF in destination directory".to_string())?;
    staged
        .write_all(bytes)
        .and_then(|_| staged.as_file().sync_all())
        .map_err(|_| "Unable to write PDF output".to_string())?;
    staged
        .persist_noclobber(destination)
        .map_err(|_| "Unable to publish PDF without replacing an existing file".to_string())?;
    Ok(())
}
