"""Verify a cold HTTP Open URL event through macOS Launch Services."""

import argparse
from hashlib import file_digest, sha256
import json
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
from tempfile import mkdtemp
import time
from uuid import uuid4

from fixture import browser_fixture
from run_launchservices_smoke import ROOT, run, source_identity
from run_smoke import observation, send_command, until


FIXTURE_APPLICATION = """
ObjC.import('AppKit');
function run(argv) {
    const [bundleID, bundlePath, executablePath, action] = argv;
    const apps = $.NSRunningApplication.runningApplicationsWithBundleIdentifier(bundleID);
    const count = Number(apps.count);
    if (count === 0) return 'closed';
    if (count !== 1) throw Error('ambiguous fixture identity');
    const app = apps.objectAtIndex(0);
    if (ObjC.unwrap(app.bundleURL.path) !== bundlePath ||
        ObjC.unwrap(app.executableURL.path) !== executablePath) {
        throw Error('unexpected fixture application paths');
    }
    if (action === 'stop' && String(app.forceTerminate) !== 'true') {
        throw Error('fixture termination failed');
    }
    return 'running';
}
"""


def fixture_running(bundle_id, bundle, *, stop=False):
    result = subprocess.run([
        "/usr/bin/osascript", "-l", "JavaScript", "-e", FIXTURE_APPLICATION,
        bundle_id, str(bundle), str(bundle / "Contents/MacOS/muffin"),
        "stop" if stop else "check",
    ], capture_output=True, text=True, timeout=6)
    if result.returncode:
        raise RuntimeError(f"fixture application lookup failed: {result.stderr.strip()}")
    state = result.stdout.strip()
    if state not in {"closed", "running"}:
        raise RuntimeError("AppKit returned an unexpected fixture state")
    return state == "running"


def clean_up(launch, control, bundle_id, bundle):
    if launch.poll() is None:
        send_command(control, "quit_test")
        try:
            launch.wait(timeout=4)
        except subprocess.TimeoutExpired:
            pass
    if fixture_running(bundle_id, bundle, stop=True):
        deadline = time.monotonic() + 4
        while fixture_running(bundle_id, bundle):
            if time.monotonic() >= deadline:
                raise RuntimeError("fixture still running; retained its isolated control directory")
            time.sleep(0.1)
    if launch.poll() is None:
        launch.kill()
        launch.wait(timeout=4)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/muffin")
    parser.add_argument("--verified-smoke", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        parser.error("use macOS Apple Silicon")
    commit, clean = source_identity()
    if not clean:
        parser.error("run from a clean harness checkout")
    binary = args.binary.resolve(strict=True)
    with binary.open("rb") as stream:
        binary_sha = file_digest(stream, "sha256").hexdigest()
    raw_smoke = args.verified_smoke.read_bytes()
    smoke = json.loads(raw_smoke)
    app_commit = smoke.get("source_commit", "")
    with Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").open("rb") as stream:
        webkit = plistlib.load(stream)["CFBundleVersion"]
    if (smoke.get("outcome") != "passed" or smoke.get("source_worktree_clean") is not True
            or smoke.get("binary_sha256") != binary_sha
            or smoke.get("host_arch") != platform.machine()
            or smoke.get("macos_version") != platform.mac_ver()[0]
            or smoke.get("webkit_bundle_version") != webkit
            or not re.fullmatch("[a-f0-9]{40}", app_commit)):
        parser.error("verified smoke must identify this exact debug binary and host")
    run("git", "cat-file", "-e", app_commit + "^{commit}")
    bundle_id = f"local.muffin.browser.coldfixture-{uuid4().hex}"
    args.out.mkdir()
    receipt = {
        "kind": "macos_debug_cold_launchservices_open_url",
        "harness_commit": commit,
        "harness_worktree_clean": True,
        "app_commit": app_commit,
        "binary_sha256": binary_sha,
        "native_debug_smoke_sha256": sha256(raw_smoke).hexdigest(),
        "host_arch": platform.machine(),
        "macos_version": platform.mac_ver()[0],
        "webkit_bundle_version": webkit,
        "steps": [],
    }
    scratch = Path(mkdtemp(prefix="muffin-cold-launch-", dir="/private/tmp"))
    bundle = scratch / "Muffin Cold Launch Fixture.app"
    fixture_closed = True
    try:
        with browser_fixture() as (server, _):
            control = scratch / "control"
            control.mkdir(mode=0o700)
            log_path = args.out / "browser.log"
            command = [
                sys.executable, "tools/bundle_macos.py", "--binary", str(binary),
                "--out", str(bundle), "--bundle-id", bundle_id,
                "--name", "Muffin Cold Launch Fixture", "--web-handler",
            ]
            run(*command)
            with (bundle / "Contents/Info.plist").open("rb") as stream:
                plist = plistlib.load(stream)
            if plist.get("CFBundleIdentifier") != bundle_id:
                raise AssertionError("unexpected temporary bundle identifier")

            url = server.origin + "/first?cold-launch=1"
            with log_path.open("wb") as log:
                launch = subprocess.Popen(
                    ["/usr/bin/open", "-n", "-W", "-a", str(bundle), url, "--args",
                     "--test-control", str(control)],
                    stdout=log, stderr=log,
                )
                fixture_closed = False
                receipt["steps"].append("open -n -W sent a cold HTTP URL event to a unique debug bundle")
                try:
                    def ready():
                        state = observation(control)
                        return state if (
                            state.get("chrome_ready")
                            and state.get("window_count") == 1
                            and state.get("status") == "ready"
                            and state.get("url") == url
                            and state.get("title") == "Muffin fixture | First page"
                            and state.get("private_window") is False
                        ) else None

                    state = until(launch, "cold Launch Services URL opened in one regular test window", ready)
                    if ("GET", "/first?cold-launch=1") not in server.requests:
                        raise AssertionError("the loopback fixture did not receive the cold-open HTTP GET")
                    if not fixture_running(bundle_id, bundle):
                        raise RuntimeError("AppKit did not identify the launched fixture")
                    receipt["steps"].append({
                        "cold_url": state["url"],
                        "title": state["title"],
                        "window_count": state["window_count"],
                        "fixture_get_observed": True,
                    })
                    # The debug control channel is used only to close this disposable app.
                    send_command(control, "quit_test")
                    launch.wait(timeout=12)
                    if launch.returncode:
                        raise RuntimeError(f"open reported {launch.returncode}")
                    receipt["steps"].append("the disposable browser closed and open -W returned")
                finally:
                    clean_up(launch, control, bundle_id, bundle)
                    fixture_closed = True
        if source_identity() != (commit, clean):
            raise RuntimeError("the harness source changed during the check")
    except Exception as error:
        receipt.update(outcome="failed", error=str(error))
        raise
    else:
        receipt["outcome"] = "passed"
    finally:
        (args.out / "cold-launch.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        if fixture_closed:
            shutil.rmtree(scratch)


if __name__ == "__main__":
    main()
