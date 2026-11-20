"""Stage a platform build record from a successful, read-only GitHub Actions job."""

from datetime import datetime, timezone
from hashlib import file_digest
import json
import os
from pathlib import Path
import platform
import subprocess
import sys


def digest(path):
    with path.open("rb") as stream:
        return file_digest(stream, "sha256").hexdigest()


def main():
    if os.environ.get("GITHUB_ACTIONS") != "true":
        sys.exit("build records can only be produced by a GitHub Actions job")
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        sys.exit("this build record describes the Apple Silicon production binary")

    workspace = Path(os.environ["GITHUB_WORKSPACE"])
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=workspace, text=True).strip()
    if head != os.environ["GITHUB_SHA"]:
        sys.exit("the verified checkout does not match GITHUB_SHA")
    subprocess.run(["git", "diff", "--exit-code", "HEAD"], cwd=workspace, check=True)
    run = int(os.environ["GITHUB_RUN_ID"])
    attempt = int(os.environ["GITHUB_RUN_ATTEMPT"])
    run_url = f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{run}/attempts/{attempt}"
    label = f"macos-arm64-build-{run:020}-{attempt:04}"
    root = workspace / "target" / "ci-evidence"
    verified_log = root / "verify.log"
    binary = workspace / "target" / "release" / "muffin"
    if not verified_log.is_file() or not verified_log.stat().st_size or not binary.is_file():
        sys.exit("verified job must retain its command log and release binary")
    binary_type = subprocess.check_output(["file", "-b", str(binary)], text=True).strip()
    if "Mach-O 64-bit executable arm64" not in binary_type:
        sys.exit("the output does not identify an arm64 Mach-O executable")

    date = datetime.now(timezone.utc).date().isoformat()
    artifact = Path("docs/reports/ci") / f"{label}.json"
    artifact_path = root / artifact
    artifact_path.parent.mkdir(parents=True, exist_ok=True)
    receipt = {
        "source_commit": head,
        "run_url": run_url,
        "host_arch": platform.machine(),
        "macos_version": platform.mac_ver()[0],
        "binary_type": binary_type,
        "binary_sha256": digest(binary),
        "command": "cargo build --workspace --locked --release",
        "verifier": "python3 tools/verify.py",
        "verify_log_sha256": digest(verified_log),
        "date": date,
    }
    artifact_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    record = {
        "id": label,
        "platform": "macos",
        "level": "build",
        "result": "pass",
        "environment": "ci_runner",
        "date": date,
        "commit": head,
        "artifact": artifact.as_posix(),
        "artifact_sha256": digest(artifact_path),
        "run_url": run_url,
    }
    record_path = root / "docs/reports/results" / f"{label}.json"
    record_path.parent.mkdir(parents=True, exist_ok=True)
    record_path.write_text(json.dumps([record], indent=2) + "\n", encoding="utf-8")
    print(f"Staged {record_path.relative_to(root)} for review and status generation.")


if __name__ == "__main__":
    main()
