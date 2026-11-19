#!/usr/bin/env python3
"""Create a platform-labelled M0b validation package and SHA-256 sidecar."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile


PLATFORMS = {
    "macos-arm64": {"os": "macOS", "architecture": "aarch64", "suffix": ".zip", "kind": "app"},
    "windows-x64": {"os": "Windows", "architecture": "x86_64", "suffix": ".zip", "kind": "windows"},
    "ubuntu-24.04-x64": {"os": "Linux", "architecture": "x86_64", "suffix": ".tar.gz", "kind": "linux"},
}
APP_IDENTIFIER = "org.muffin.m0b-validation"
APP_NAME = "Muffin"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def identity_for(binary: Path, expected_commit: str) -> dict:
    result = subprocess.run(
        [str(binary), "--identity"], check=True, capture_output=True, text=True, timeout=20
    )
    lines = result.stdout.splitlines()
    if len(lines) != 1 or not lines[0].startswith("identity "):
        raise ValueError("binary --identity must print exactly one `identity {json}` line")
    identity = json.loads(lines[0][len("identity "):])
    if not isinstance(identity, dict):
        raise ValueError("binary identity payload must be a JSON object")
    required = {
        "app_commit", "binary_sha256", "os", "os_version", "host_architecture",
        "binary_architecture", "engine_version", "dependencies", "locked",
    }
    missing = required - identity.keys()
    if missing:
        raise ValueError(f"binary identity is missing fields: {', '.join(sorted(missing))}")
    for field in ("app_commit", "binary_sha256", "os", "os_version", "host_architecture", "binary_architecture", "engine_version"):
        if not isinstance(identity[field], str) or not identity[field]:
            raise ValueError(f"binary identity field {field} must be a non-empty string")
    if not isinstance(identity["dependencies"], dict):
        raise ValueError("binary identity dependencies must be a JSON object")
    if identity["app_commit"] != expected_commit:
        raise ValueError("binary app_commit does not match --source-commit")
    actual_digest = sha256(binary)
    if identity["binary_sha256"] != actual_digest:
        raise ValueError("binary identity SHA-256 does not match the executable file")
    if identity["locked"] != {"tao": "0.37.2", "wry": "0.57.2"}:
        raise ValueError("binary identity does not confirm the required locked TAO and WRY versions")
    return identity


def normalized_arch(value: str) -> str:
    return {"arm64": "aarch64", "amd64": "x86_64"}.get(value.lower(), value.lower())


def copy_executable(binary: Path, destination: Path) -> None:
    shutil.copyfile(binary, destination)
    destination.chmod(destination.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def make_zip(root: Path, archive: Path, top: Path) -> None:
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(root)
            archive_name = (top / relative).as_posix()
            info = zipfile.ZipInfo(archive_name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (path.stat().st_mode & 0xFFFF) << 16
            output.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def make_tar(root: Path, archive: Path, top: Path) -> None:
    with archive.open("wb") as raw:
        import gzip

        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as output:
                for path in sorted(root.rglob("*")):
                    if not path.is_file():
                        continue
                    relative = path.relative_to(root)
                    info = output.gettarinfo(str(path), arcname=(top / relative).as_posix())
                    info.mtime = 0
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    with path.open("rb") as source:
                        output.addfile(info, source)


def package(
    binary: Path, target: str, source_commit: str, output_dir: Path, stage_dir: Path | None = None
) -> tuple[Path, Path, Path | None]:
    if target not in PLATFORMS:
        raise ValueError(f"unsupported target: {target}")
    if len(source_commit) != 40 or any(ch not in "0123456789abcdef" for ch in source_commit):
        raise ValueError("--source-commit must be a lowercase 40-character Git SHA")
    binary = binary.resolve(strict=True)
    if not binary.is_file():
        raise ValueError("--binary must name a file")

    spec = PLATFORMS[target]
    identity = identity_for(binary, source_commit)
    if (
        identity["os"].lower() != spec["os"].lower()
        or normalized_arch(identity["host_architecture"]) != spec["architecture"]
        or normalized_arch(identity["binary_architecture"]) != spec["architecture"]
    ):
        raise ValueError("binary identity platform does not match --target")

    output_dir.mkdir(parents=True, exist_ok=True)
    archive = output_dir / f"muffin-m0b-validation-{target}{spec['suffix']}"
    sidecar = output_dir / f"muffin-m0b-validation-{target}.metadata.json"
    with tempfile.TemporaryDirectory(prefix="muffin-m0b-package-") as temp:
        stage = Path(temp)
        if spec["kind"] == "app":
            app = stage / f"{APP_NAME}.app"
            contents = app / "Contents"
            executable = contents / "MacOS" / APP_NAME
            resources = contents / "Resources"
            executable.parent.mkdir(parents=True)
            resources.mkdir(parents=True)
            copy_executable(binary, executable)
            with (contents / "Info.plist").open("wb") as plist:
                plistlib.dump({
                    "CFBundleExecutable": APP_NAME,
                    "CFBundleIdentifier": APP_IDENTIFIER,
                    "CFBundleName": APP_NAME,
                    "CFBundlePackageType": "APPL",
                    "CFBundleShortVersionString": "0.0.0",
                    "CFBundleVersion": "0.0.0",
                    "LSMinimumSystemVersion": "12.0",
                    "NSHighResolutionCapable": True,
                }, plist, sort_keys=True)
            top = Path()
            root = stage
            manifest_target = "Muffin.app/Contents/Resources/BUILD-METADATA.json"
        else:
            root = stage / "muffin-m0b-validation"
            root.mkdir()
            executable_name = "Muffin.exe" if spec["kind"] == "windows" else "Muffin"
            executable = root / executable_name
            copy_executable(binary, executable)
            top = Path("muffin-m0b-validation")
            manifest_target = "BUILD-METADATA.json"

        metadata = {
            "schema": 1,
            "application": "Muffin M0b Validation",
            "package": archive.name,
            "package_format": "zip" if spec["suffix"] == ".zip" else "tar.gz",
            "target": target,
            "source_commit": source_commit,
            "binary_sha256": identity["binary_sha256"],
            "binary_path": str(executable.relative_to(stage)),
            "native_identity": identity,
            "macos_bundle_identifier": APP_IDENTIFIER if spec["kind"] == "app" else None,
            "package_sha256": None,
        }
        manifest_path = stage / manifest_target
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        manifest_path.write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        staged_binary = None
        if stage_dir is not None:
            if stage_dir.exists() and any(stage_dir.iterdir()):
                raise ValueError("--stage-dir must be absent or empty")
            stage_dir.mkdir(parents=True, exist_ok=True)
            shutil.copytree(stage, stage_dir, dirs_exist_ok=True)
            staged_binary = stage_dir / executable.relative_to(stage)
        if spec["suffix"] == ".zip":
            make_zip(root, archive, top)
        else:
            make_tar(root, archive, top)

    metadata["package_sha256"] = sha256(archive)
    sidecar.write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return archive, sidecar, staged_binary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--target", required=True, choices=sorted(PLATFORMS))
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument("--stage-dir", type=Path,
                        help="optional unpacked package tree for running the exact packaged executable")
    args = parser.parse_args()
    try:
        archive, sidecar, staged_binary = package(
            args.binary, args.target, args.source_commit, args.out_dir, args.stage_dir
        )
    except (OSError, ValueError, subprocess.SubprocessError, json.JSONDecodeError) as error:
        print(f"package error: {error}", file=sys.stderr)
        return 2
    print(f"package={archive}")
    print(f"metadata={sidecar}")
    if staged_binary:
        print(f"staged_binary={staged_binary}")
    print(f"package_sha256={sha256(archive)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
