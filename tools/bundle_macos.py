"""Wrap an existing Apple Silicon Muffin executable in a local app bundle."""

import argparse
from pathlib import Path
import plistlib
import re
import shutil
import tomllib


ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--bundle-id", default="local.muffin.browser.development")
    parser.add_argument("--name", default="Muffin Development")
    parser.add_argument("--web-handler", action="store_true",
                        help="declare HTTP and HTTPS handling in the app bundle")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not binary.is_file() or args.out.suffix != ".app" or args.out.exists() or args.out.is_symlink():
        parser.error("supply an existing binary and a new .app path")
    if not re.fullmatch(r"[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+", args.bundle_id):
        parser.error("supply a reverse-DNS bundle identifier")
    with (ROOT / "crates/muffin/Cargo.toml").open("rb") as manifest:
        version = tomllib.load(manifest)["package"]["version"]
    contents = args.out / "Contents"
    executables = contents / "MacOS"
    executables.mkdir(parents=True)
    shutil.copy2(binary, executables / "muffin")
    metadata = {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleExecutable": "muffin",
        "CFBundleIdentifier": args.bundle_id,
        "CFBundleName": args.name,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "NSCameraUsageDescription": "A website you visit can ask to use your camera.",
        "NSMicrophoneUsageDescription": "A website you visit can ask to use your microphone.",
    }
    if args.web_handler:
        metadata["CFBundleURLTypes"] = [{
            "CFBundleURLName": args.bundle_id,
            "CFBundleTypeRole": "Viewer",
            "CFBundleURLSchemes": ["http", "https"],
        }]
    with (contents / "Info.plist").open("wb") as stream:
        plistlib.dump(metadata, stream)


if __name__ == "__main__":
    main()
