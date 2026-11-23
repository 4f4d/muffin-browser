"""Send loopback URLs through macOS Launch Services to an isolated debug bundle."""

import argparse
from hashlib import file_digest, sha256
import json
from pathlib import Path
import platform
import plistlib
import subprocess
import sys
from tempfile import TemporaryDirectory
from uuid import uuid4

from fixture import browser_fixture
from run_smoke import observation, send_command, until


ROOT = Path(__file__).resolve().parents[2]


def run(*command, **options):
    return subprocess.run(command, check=True, cwd=ROOT, timeout=12, **options)


def source_identity():
    commit = run("git", "rev-parse", "HEAD", capture_output=True, text=True).stdout.strip()
    clean = not run("git", "status", "--porcelain", capture_output=True, text=True).stdout.strip()
    return commit, clean


def exercise(binary, out, steps):
    with TemporaryDirectory(prefix="muffin-launchservices-", dir="/private/tmp") as directory, browser_fixture() as servers:
        scratch = Path(directory)
        control = scratch / "control"
        control.mkdir()
        bundle = scratch / "Muffin Launch Services Fixture.app"
        run(sys.executable, "tools/bundle_macos.py", "--binary", binary, "--out", bundle,
            "--bundle-id", f"local.muffin.browser.fixture-{uuid4().hex}", "--web-handler")
        first, second = servers
        with (out / "browser.log").open("wb") as log:
            process = subprocess.Popen([bundle / "Contents/MacOS/muffin", "--test-control", control],
                                       stdout=log, stderr=log)
            try:
                def expect(label, **fields):
                    def matches():
                        state = observation(control)
                        return all(state.get(key) == value for key, value in fields.items())

                    until(process, label, matches)
                    steps.append(label)

                expect("fixture started on Home", status="home", chrome_ready=True, window_count=1)
                first_url = first.origin + "/first"
                run("/usr/bin/open", "-a", bundle, first_url, capture_output=True)
                expect("Launch Services opened HTTP in the running regular window",
                       status="ready", url=first_url, private_window=False, window_count=1)
                send_command(control, "press_fixture_shortcut", shortcut="private_window")
                expect("private window received focus",
                       chrome_ready=True, private_window=True, window_count=2)
                second_url = second.origin + "/second"
                run("/usr/bin/open", "-a", bundle, second_url, capture_output=True)
                expect("external HTTP selected the regular window with a private window open",
                       status="ready", url=second_url, private_window=False, window_count=2)
                send_command(control, "quit_test")
                if process.wait(timeout=8):
                    raise RuntimeError("fixture did not shut down cleanly")
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/muffin")
    parser.add_argument("--verified-smoke", type=Path, required=True,
                        help="native/smoke.json from the full verifier for this source and debug binary")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        parser.error("use the Apple Silicon debug build on macOS")
    commit, clean = source_identity()
    if not clean:
        parser.error("run from a clean source checkout")
    binary = args.binary.resolve(strict=True)
    with binary.open("rb") as stream:
        binary_sha = file_digest(stream, "sha256").hexdigest()
    raw_smoke = args.verified_smoke.read_bytes()
    smoke = json.loads(raw_smoke)
    if (smoke.get("outcome") != "passed" or smoke.get("source_commit") != commit
            or smoke.get("source_worktree_clean") is not True or smoke.get("binary_sha256") != binary_sha):
        parser.error("the native smoke receipt must identify the same clean source and debug binary")
    args.out.mkdir()
    with Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").open("rb") as stream:
        webkit_version = plistlib.load(stream)["CFBundleVersion"]
    result = {
        "kind": "native_debug_loopback_launchservices",
        "source_commit": commit,
        "source_worktree_clean": True,
        "binary_sha256": binary_sha,
        "native_debug_smoke_sha256": sha256(raw_smoke).hexdigest(),
        "host_arch": platform.machine(),
        "macos_version": platform.mac_ver()[0],
        "webkit_bundle_version": webkit_version,
        "steps": [],
    }
    try:
        exercise(binary, args.out, result["steps"])
        if source_identity() != (commit, True):
            raise RuntimeError("source changed during the Launch Services check")
    except Exception as error:
        result.update(outcome="failed", error=str(error))
        raise
    else:
        result["outcome"] = "passed"
    finally:
        (args.out / "launchservices.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
