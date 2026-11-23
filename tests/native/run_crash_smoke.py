"""Check host-process crash recovery with an isolated debug profile."""

import argparse
from hashlib import file_digest
import json
from pathlib import Path
import platform
import plistlib
import re
import signal
import subprocess
import time
from tempfile import TemporaryDirectory

from fixture import browser_fixture
from run_smoke import observation, send_command, until


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/muffin"))
    parser.add_argument("--binary-source-commit", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.binary_source_commit):
        parser.error("--binary-source-commit must be a full 40-character lowercase SHA")
    binary = args.binary.resolve(strict=True)
    subprocess.run(
        ["git", "cat-file", "-e", f"{args.binary_source_commit}^{{commit}}"],
        check=True, stdout=subprocess.DEVNULL,
    )
    args.out.mkdir(parents=True, exist_ok=True)
    with binary.open("rb") as stream:
        binary_sha = file_digest(stream, "sha256").hexdigest()
    with Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").open("rb") as stream:
        webkit_version = plistlib.load(stream)["CFBundleVersion"]
    result = {
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "binary_source_commit": args.binary_source_commit,
        "source_worktree_clean": not subprocess.check_output(["git", "status", "--porcelain"], text=True).strip(),
        "binary_sha256": binary_sha,
        "host_arch": platform.machine(),
        "macos_version": platform.mac_ver()[0],
        "webkit_bundle_version": webkit_version,
        "kind": "development_native_debug_host_process_crash",
        "steps": [],
    }
    try:
        with TemporaryDirectory(prefix="muffin-crash-smoke-", dir="/private/tmp") as temporary, browser_fixture() as servers:
            control = Path(temporary)
            session_path = control / "session.json"
            launch = [str(binary), "--test-control", str(control), "--test-session", str(session_path)]
            log_path = args.out / "crash-smoke.log"
            with log_path.open("wb") as log:
                process = subprocess.Popen(launch, stdout=log, stderr=log)
                try:
                    until(process, "native chrome ready", lambda: (
                        state if (state := observation(control)).get("chrome_ready") else None
                    ))
                    regular_url = servers[0].origin + "/first?crash-recovery=regular"
                    send_command(control, "navigate", value=regular_url)
                    until(process, "regular fixture tab committed", lambda: (
                        state if (state := observation(control)).get("url") == regular_url
                        and state.get("status") == "ready" else None
                    ))
                    send_command(control, "new_private_window")
                    until(process, "private window opened", lambda: (
                        state if (state := observation(control)).get("private_window") is True
                        and state.get("window_count") == 2 else None
                    ))
                    private_url = servers[1].origin + "/second?probe=private-crash-only"
                    send_command(control, "navigate", value=private_url)
                    until(process, "private fixture tab committed", lambda: (
                        state if (state := observation(control)).get("url") == private_url
                        and state.get("status") == "ready" else None
                    ))
                    deadline = time.monotonic() + 10
                    while time.monotonic() < deadline:
                        if session_path.is_file() and session_path.stat().st_size:
                            session_text = session_path.read_text(encoding="utf-8")
                            session = json.loads(session_text)
                            if any(
                                tab.get("url") == regular_url
                                for window in session.get("windows", [])
                                for tab in window.get("tabs", [])
                            ):
                                break
                        time.sleep(0.05)
                    else:
                        raise TimeoutError("regular session was not saved before the crash")
                    if "private-crash-only" in session_text or len(session["windows"]) != 1:
                        raise AssertionError("private window data reached the persisted regular session")
                    result["steps"].append({
                        "regular_session_saved_before_crash": True,
                        "private_window_excluded_before_crash": True,
                        "crashed_while_private_window_selected": True,
                    })
                    process.send_signal(signal.SIGKILL)
                    process.wait(timeout=8)
                    if process.returncode != -signal.SIGKILL:
                        raise AssertionError(f"host did not exit from SIGKILL: {process.returncode}")

                    (control / "commands.jsonl").unlink(missing_ok=True)
                    (control / "state.json").unlink(missing_ok=True)
                    process = subprocess.Popen(launch, stdout=log, stderr=log)
                    restored = until(process, "regular session restored after host crash", lambda: (
                        state if (state := observation(control)).get("chrome_ready")
                        and state.get("window_count") == 1
                        and state.get("private_window") is False
                        and state.get("url") == regular_url
                        and state.get("status") == "ready" else None
                    ))
                    forwarded_url = servers[0].origin + "/first?crash-recovery=forwarded"
                    forwarded_process = subprocess.run(
                        [*launch, forwarded_url], capture_output=True, timeout=8,
                    )
                    if forwarded_process.returncode != 0:
                        raise AssertionError("second launch did not exit after forwarding its URL")
                    forwarded = until(process, "second launch URL forwarded to recovered host", lambda: (
                        state if (state := observation(control)).get("url") == forwarded_url
                        and state.get("status") == "ready"
                        and state.get("window_count") == 1
                        and state.get("private_window") is False else None
                    ))
                    result["steps"].append({
                        "restored_regular_url": restored["url"],
                        "forwarded_url_after_recovery": forwarded["url"],
                        "window_count": restored["window_count"],
                        "private_window_restored": False,
                        "single_instance_lock_recovered": True,
                    })
                except Exception:
                    state_path = control / "state.json"
                    if state_path.is_file():
                        result["last_observation"] = json.loads(state_path.read_text(encoding="utf-8"))
                    raise
                finally:
                    if process.poll() is None:
                        send_command(control, "quit_test")
                        try:
                            process.wait(timeout=8)
                        except subprocess.TimeoutExpired:
                            process.terminate()
                            try:
                                process.wait(timeout=4)
                            except subprocess.TimeoutExpired:
                                process.kill()
                                process.wait(timeout=5)
    except Exception as exc:
        result.update(outcome="failed", error=str(exc))
        raise
    else:
        result["outcome"] = "passed"
    finally:
        (args.out / "crash-smoke.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
