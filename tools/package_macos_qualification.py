"""Build and check a local macOS Apple Silicon qualification package."""

import argparse
from hashlib import file_digest
import json
import os
from pathlib import Path
import platform
import plistlib
import subprocess
import sys
from tempfile import TemporaryDirectory

from verify import ROOT


BUNDLE_ID = "local.muffin.browser.qualification"
BUNDLE_NAME = "Muffin Qualification.app"
BINARY_PATH = Path("Contents/MacOS/muffin")
PLIST_PATH = Path("Contents/Info.plist")


def run(*command, **options):
    return subprocess.run(command, cwd=ROOT, check=True, **options)


def output(*command):
    return run(*command, capture_output=True, text=True).stdout.strip()


def digest(path):
    with path.open("rb") as stream:
        return file_digest(stream, "sha256").hexdigest()


def check_bundle(bundle, expected_binary, expected_plist):
    files = {path.relative_to(bundle) for path in bundle.rglob("*") if not path.is_dir()}
    if files != {BINARY_PATH, PLIST_PATH} or any(path.is_symlink() for path in bundle.rglob("*")):
        raise RuntimeError("unexpected files in the qualification bundle")
    binary = bundle / BINARY_PATH
    plist = bundle / PLIST_PATH
    if (digest(binary), digest(plist)) != (expected_binary, expected_plist) or not os.access(binary, os.X_OK):
        raise RuntimeError("archive round trip changed a bundle payload or executable permission")
    if output("/usr/bin/lipo", "-archs", binary) != "arm64":
        raise RuntimeError("qualification bundle must contain a thin arm64 executable")
    run("/usr/bin/plutil", "-lint", plist)
    with plist.open("rb") as stream:
        metadata = plistlib.load(stream)
    if metadata.get("CFBundleIdentifier") != BUNDLE_ID or metadata.get("CFBundleExecutable") != "muffin":
        raise RuntimeError("qualification bundle identity is invalid")
    urls = metadata.get("CFBundleURLTypes")
    if urls != [{"CFBundleURLName": BUNDLE_ID, "CFBundleTypeRole": "Viewer",
                 "CFBundleURLSchemes": ["http", "https"]}] or "CFBundleDocumentTypes" in metadata:
        raise RuntimeError("the bundle must declare HTTP and HTTPS URL handling only")
    if not metadata.get("NSCameraUsageDescription") or not metadata.get("NSMicrophoneUsageDescription"):
        raise RuntimeError("camera and microphone usage descriptions are required")
    return metadata["CFBundleShortVersionString"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True, help="new local output directory")
    args = parser.parse_args()
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        parser.error("packaging requires Apple Silicon macOS")
    if output("git", "status", "--porcelain", "--untracked-files=normal"):
        parser.error("package from a clean source checkout")
    out = args.out.absolute()
    if out.exists() or out.is_symlink() or not out.parent.is_dir():
        parser.error("choose a new output directory whose parent exists")
    commit = output("git", "rev-parse", "HEAD")
    out.mkdir(mode=0o700)
    log = out / "verify.log"
    with log.open("wb") as stream:
        run(sys.executable, "tools/verify.py", "--artifacts", out / "native", stdout=stream,
            stderr=subprocess.STDOUT)
    smoke_path = out / "native/smoke.json"
    with smoke_path.open() as stream:
        smoke = json.load(stream)
    if (smoke.get("source_commit") != commit or not smoke.get("source_worktree_clean")
            or smoke.get("outcome") != "passed"
            or output("git", "status", "--porcelain", "--untracked-files=normal")
            or output("git", "rev-parse", "HEAD") != commit):
        raise RuntimeError("the verifier must identify the same clean source")

    release = ROOT / "target/release/muffin"
    binary_sha = digest(release)
    bundle = out / BUNDLE_NAME
    run(sys.executable, "tools/bundle_macos.py", "--binary", release, "--out", bundle,
        "--bundle-id", BUNDLE_ID, "--name", "Muffin Qualification", "--web-handler")
    plist_sha = digest(bundle / PLIST_PATH)
    version = check_bundle(bundle, binary_sha, plist_sha)
    archive = out / f"muffin-{version}-macos-arm64-qualification.zip"
    run("/usr/bin/ditto", "-c", "-k", "--keepParent", bundle, archive)

    with TemporaryDirectory(prefix="muffin-archive-", dir=out) as temporary:
        scratch = Path(temporary)
        run("/usr/bin/ditto", "-x", "-k", archive, scratch)
        if {path.name for path in scratch.iterdir()} != {BUNDLE_NAME}:
            raise RuntimeError("unexpected archive root")
        extracted = scratch / BUNDLE_NAME
        check_bundle(extracted, binary_sha, plist_sha)
        isolated = scratch / "control"
        isolated.mkdir()
        probe = subprocess.run([extracted / BINARY_PATH, "--test-control", isolated],
                               capture_output=True, text=True, timeout=5)
        if (probe.returncode != 1 or probe.stderr.strip() != "Muffin could not start: unknown or malformed argument"
                or any(isolated.iterdir())):
            raise RuntimeError("packaged release executable must reject debug control")

    receipt = {
        "kind": "local_qualification_archive",
        "source_commit": commit,
        "source_worktree_clean": True,
        "app_version": version,
        "bundle_id": BUNDLE_ID,
        "host_arch": platform.machine(),
        "binary_arch": "arm64",
        "os_version": platform.mac_ver()[0],
        "os_build": output("/usr/bin/sw_vers", "-buildVersion"),
        "webkit_bundle_version": smoke["webkit_bundle_version"],
        "binary_sha256": binary_sha,
        "binary_bytes": release.stat().st_size,
        "plist_sha256": plist_sha,
        "bundle_payload_bytes": sum((bundle / file).stat().st_size for file in (BINARY_PATH, PLIST_PATH)),
        "archive": archive.name,
        "archive_bytes": archive.stat().st_size,
        "archive_sha256": digest(archive),
        "verifier_log_sha256": digest(log),
        "native_debug_smoke_sha256": digest(smoke_path),
        "archive_round_trip": "passed",
        "extracted_release_rejected_debug_control": True,
        "distribution_signing": "not_performed",
    }
    (out / "qualification.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print(archive)


if __name__ == "__main__":
    main()
