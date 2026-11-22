"""Run the production browser's M1 engineering checks from the repository root."""

import argparse
import os
from pathlib import Path
import platform
import subprocess
from tempfile import TemporaryDirectory


ROOT = Path(__file__).resolve().parents[1]


def run(*command, cwd=ROOT):
    print("+", " ".join(command), flush=True)
    environment = os.environ.copy()
    if command[0] == "cargo":
        environment["CARGO_TARGET_DIR"] = str(ROOT / "target")
    subprocess.run(command, cwd=cwd, env=environment, check=True)


def check_release_control_excluded():
    with TemporaryDirectory(prefix="muffin-release-check-") as directory:
        result = subprocess.run(
            [str(ROOT / "target/release/muffin"), "--test-control", directory],
            cwd=ROOT, timeout=5, capture_output=True, text=True, check=False,
        )
        if (
            result.returncode != 1
            or result.stderr.strip() != "Muffin could not start: unknown or malformed argument"
            or any(Path(directory).iterdir())
        ):
            raise RuntimeError("the release binary must reject debug test control")
    print("The release binary rejected the debug test-control argument.", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--portable", action="store_true",
        help="run only the portable status, fixture and chrome checks",
    )
    parser.add_argument("--artifacts", type=Path, default=ROOT / "target/dev-verification")
    args = parser.parse_args()
    if not args.portable and (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        parser.error("the complete native verifier requires Apple Silicon macOS")

    run("python3", "-m", "unittest", "discover", "-s", "tools/tests", "-v")
    run("python3", "tools/gen_status.py", "--check")
    run("python3", "-m", "unittest", "discover", "-s", "tests/native", "-v")
    run("npm", "ci", cwd=ROOT / "ui")
    run("npm", "run", "check", cwd=ROOT / "ui")
    run("npm", "test", cwd=ROOT / "ui")
    run("npm", "audit", "--audit-level=high", cwd=ROOT / "ui")
    run("npm", "run", "build", cwd=ROOT / "ui")
    if args.portable:
        return

    run("cargo", "fmt", "--all", "--check")
    run("cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings")
    run("cargo", "test", "--workspace", "--locked")
    run("cargo", "audit")
    run("cargo", "build", "--workspace", "--locked")
    run("cargo", "build", "--workspace", "--locked", "--release")
    check_release_control_excluded()
    run(
        "python3", "tests/native/run_smoke.py", "--tabs", "--restart", "--profile", "--page-tools", "--privacy",
        "--out", str(args.artifacts.resolve()),
    )


if __name__ == "__main__":
    main()
