use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, process::Command};

pub fn report(engine: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut executable = File::open(std::env::current_exe()?)?;
    let mut buffer = [0; 32768];
    let mut digest = Sha256::new();
    loop {
        let count = executable.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    #[cfg(target_os = "macos")]
    let host = command("uname", &["-m"]);
    #[cfg(target_os = "linux")]
    let (os, host) = (command("lsb_release", &["-ds"]), command("uname", &["-m"]));
    #[cfg(target_os = "windows")]
    let (os, host) = (
        command("cmd", &["/d", "/c", "ver"]),
        std::env::var("PROCESSOR_ARCHITEW6432")
            .or_else(|_| std::env::var("PROCESSOR_ARCHITECTURE"))
            .unwrap_or_default(),
    );
    #[cfg(target_os = "macos")]
    let os = format!(
        "{} {}",
        command("sw_vers", &["-productVersion"]),
        command("sw_vers", &["-buildVersion"])
    );
    #[cfg(target_os = "linux")]
    let dependencies = serde_json::json!({
        "gtk": format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version()),
        "kernel": command("uname", &["-r"]), "session": session(), "webkit": "WebKitGTK 4.1",
    });
    #[cfg(target_os = "macos")]
    let dependencies = serde_json::json!({"ui": "AppKit", "webkit": "system WebKit"});
    #[cfg(target_os = "windows")]
    let dependencies = serde_json::json!({"webkit": "WebView2 Evergreen Runtime"});
    println!(
        "identity {}",
        serde_json::json!({
            "app_commit": env!("MUFFIN_BUILD_COMMIT"),
            "binary_sha256": format!("{:x}", digest.finalize()),
            "os": std::env::consts::OS,
            "os_version": os,
            "host_architecture": host,
            "binary_architecture": std::env::consts::ARCH,
            "engine_version": engine,
            "dependencies": dependencies,
            "locked": {"tao": "0.37.2", "wry": "0.57.2"}
        })
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn session() -> &'static str {
    match std::env::var("GDK_BACKEND").ok().as_deref() {
        Some("x11") => "forced x11",
        Some("wayland") => "forced wayland",
        _ => match std::env::var("XDG_SESSION_TYPE").ok().as_deref() {
            Some("x11") => "x11",
            Some("wayland") => "wayland",
            _ => "unknown",
        },
    }
}

fn command(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|output| !output.is_empty())
        .unwrap_or_else(|| "unknown".into())
}
