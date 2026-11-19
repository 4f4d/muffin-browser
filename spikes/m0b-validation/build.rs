use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=MUFFIN_SOURCE_COMMIT");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let sha = env::var("MUFFIN_SOURCE_COMMIT").unwrap_or_default();
    if sha.is_empty() {
        println!("cargo:rustc-env=MUFFIN_BUILD_COMMIT=unversioned");
        return;
    }
    assert!(
        sha.len() == 40
            && sha
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    let git = |args: &[&str]| {
        let output = Command::new("git").args(args).output().expect("git");
        assert!(output.status.success(), "source identity Git check failed");
        String::from_utf8(output.stdout).expect("Git output")
    };
    assert_eq!(git(&["rev-parse", "HEAD"]).trim(), sha);
    assert!(
        git(&["status", "--porcelain=v1", "--", "."]).is_empty(),
        "commit all validation source before producing an identified build"
    );
    println!("cargo:rustc-env=MUFFIN_BUILD_COMMIT={sha}");
}
