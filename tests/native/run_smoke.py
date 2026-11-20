"""Exercise the Apple Silicon shell against a disposable loopback fixture."""

import argparse
from hashlib import file_digest
import json
from pathlib import Path
import platform
import plistlib
import subprocess
from tempfile import TemporaryDirectory
import time
import tomllib

from fixture import browser_fixture


def until(process, description, read, timeout=18):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"browser exited ({process.returncode}) waiting for {description}")
        value = read()
        if value:
            return value
        time.sleep(0.05)
    raise TimeoutError(f"waiting for {description}")


def exercise(process, control, first, second, steps, capture):
    state_path = control / "state.json"

    def command(action, **fields):
        with (control / "commands.jsonl").open("a", encoding="utf-8") as stream:
            stream.write(json.dumps({"version": 1, "action": action, **fields}) + "\n")

    def expect(**fields):
        def read():
            if state_path.is_file():
                state = json.loads(state_path.read_text(encoding="utf-8"))
                if all(state.get(key) == value for key, value in fields.items()):
                    return {key: state[key] for key in fields}

        state = until(process, repr(fields), read)
        steps.append(state)

    expect(version=1, status="home", chrome_ready=True)
    first_url = first.origin + "/first"
    second_url = second.origin + "/second"
    command("chrome_navigate", value=first_url)
    expect(url=first_url, status="ready", title="Muffin fixture | First page", secure=False)
    steps.append({"navigation_input": "trusted chrome address form"})
    manifest = Path(__file__).resolve().parents[2] / "crates/muffin/Cargo.toml"
    with manifest.open("rb") as stream:
        product_token = "Muffin/" + tomllib.load(stream)["package"]["version"]
    user_agent = first.user_agents.get("/first", "")
    if "AppleWebKit/" not in user_agent or product_token not in user_agent.split():
        raise AssertionError("WebKit request did not include the normal engine and Muffin product tokens")
    steps.append({"webkit_user_agent_token": True, "browser_product_token": product_token})
    command("navigate", value=first.origin + "/redirect")
    expect(url=second_url, status="ready", title="Muffin fixture | Second page", can_back=True)
    until(
        process,
        "page-owned IPC lookalike fixture finished",
        lambda: ("POST", "/remote-page-ready?chrome_state=false") in second.requests,
    )
    expect(url=second_url, status="ready")
    if ("POST", "/unexpected-native-read") in second.requests:
        raise AssertionError("the native shell consumed the page-owned action queue")
    steps.append({"remote_page_executed": True, "remote_chrome_state_received": False})
    if capture is not None:
        capture(process, control, command)

    command("back")
    expect(url=first_url, status="ready", can_forward=True)
    command("forward")
    expect(url=second_url, status="ready")

    command("navigate", value="file:///muffin-fixture-should-stay-inaccessible")
    expect(status="blocked", url=second_url)
    if ("GET", "/redirect") not in first.requests:
        raise AssertionError("the first origin did not serve the redirect")
    steps.append({"distinct_loopback_origins": True, "redirect_requested": True})

    command("navigate", value=first.origin + "/slow")
    until(process, "slow fixture response", lambda: ("GET", "/slow") in first.requests)
    expect(loading=True)
    command("stop")
    expect(loading=False, native_loading=False)
    first.release_slow_page.set()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/muffin"))
    parser.add_argument("--out", type=Path, required=True)
    visual = parser.add_mutually_exclusive_group()
    visual.add_argument("--capture-window", action="store_true", help="attempt a capture of the fixture window")
    visual.add_argument("--capture-chrome", action="store_true", help="capture the native trusted chrome view")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.out.mkdir(parents=True, exist_ok=True)
    with binary.open("rb") as stream:
        binary_sha = file_digest(stream, "sha256").hexdigest()
    with Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").open("rb") as stream:
        webkit_version = plistlib.load(stream)["CFBundleVersion"]
    result = {
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "source_worktree_clean": not subprocess.check_output(
            ["git", "status", "--porcelain"], text=True
        ).strip(),
        "binary_sha256": binary_sha,
        "host_arch": platform.machine(),
        "macos_version": platform.mac_ver()[0],
        "webkit_bundle_version": webkit_version,
        "kind": "development_native_debug_loopback",
        "steps": [],
    }
    capture = None
    if args.capture_window:
        def capture(_process, control, _command):
            observation = json.loads((control / "state.json").read_text(encoding="utf-8"))
            window_number = observation["window_number"]
            image = args.out / "muffin-window.png"
            if not isinstance(window_number, int) or window_number <= 0:
                result["window_capture"] = "unavailable_window_number"
                return
            capture_result = subprocess.run(
                ["/usr/sbin/screencapture", "-x", "-o", "-l", str(window_number), str(image)],
                timeout=12, capture_output=True,
            )
            if capture_result.returncode != 0 or not image.is_file() or not image.stat().st_size:
                result["window_capture"] = "capture_failed"
                result["window_capture_diagnostic"] = capture_result.stderr.decode(errors="replace").strip()
                return
            with image.open("rb") as stream:
                result["window_image_sha256"] = file_digest(stream, "sha256").hexdigest()
            result["window_capture"] = "captured"
    if args.capture_chrome:
        def capture(process, control, command):
            snapshot = control / "chrome.tiff"
            image = args.out / "native-chrome.png"
            command("capture_chrome")
            until(process, "native trusted chrome image", lambda: snapshot.is_file() and snapshot.stat().st_size)
            subprocess.run(
                ["/usr/bin/sips", "-s", "format", "png", str(snapshot), "--out", str(image)],
                check=True, capture_output=True, timeout=10,
            )
            with image.open("rb") as stream:
                result["chrome_image_sha256"] = file_digest(stream, "sha256").hexdigest()

    try:
        with TemporaryDirectory(prefix="muffin-smoke-") as temporary, browser_fixture() as servers:
            control = Path(temporary)
            with (args.out / "browser.log").open("wb") as log:
                process = subprocess.Popen([str(binary), "--test-control", str(control)], stdout=log, stderr=log)
                try:
                    exercise(process, control, *servers, result["steps"], capture)
                except Exception:
                    state_path = control / "state.json"
                    if state_path.is_file():
                        result["last_observation"] = json.loads(state_path.read_text(encoding="utf-8"))
                    raise
                finally:
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            process.kill()
                            process.wait(timeout=5)
    except Exception as exc:
        result.update(outcome="failed", error=str(exc))
        raise
    else:
        result["outcome"] = "passed"
    finally:
        (args.out / "smoke.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Native debug smoke passed ({args.out / 'smoke.json'})")


if __name__ == "__main__":
    main()
