#!/usr/bin/env python3
"""Run the actual validation executable against synthetic loopback origins."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

import server


def identity(log: str) -> dict:
    receipts = [line.removeprefix("identity ") for line in log.splitlines() if line.startswith("identity ")]
    if len(receipts) != 1:
        raise RuntimeError("expected exactly one native identity receipt")
    report = json.loads(receipts[0])
    if any(not report.get(key) for key in (
        "app_commit", "binary_sha256", "os_version", "host_architecture",
        "binary_architecture", "engine_version",
    )):
        raise RuntimeError("native identity fields incomplete")
    return report


def platform_mark(path: Path) -> bool | None:
    if os.name == "nt":
        return Path(str(path) + ":Zone.Identifier").is_file()
    if sys.platform == "darwin":
        result = subprocess.run(
            ["xattr", "-p", "com.apple.quarantine", str(path)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
        )
        return result.returncode == 0
    return None


def verify_downloads(data: Path) -> dict:
    expected = hashlib.sha256(server.AUTH_DOWNLOAD).hexdigest()
    large_payload = server.LARGE_BLOCK * server.LARGE_BLOCKS
    large_hash = hashlib.sha256(large_payload).hexdigest()
    sizes = []
    marks = []
    large = []
    for path in (data / "downloads").glob("muffin-*/*"):
        if not path.is_file():
            continue
        length = path.stat().st_size
        if length not in (len(server.AUTH_DOWNLOAD), len(large_payload)):
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest == expected:
            sizes.append(length)
            marks.append(platform_mark(path))
        elif digest == large_hash:
            large.append(length)
    return {
        "count": len(sizes), "expected_sha256": expected, "bytes": sizes,
        "platform_marks_present": marks, "large_download_bytes": large,
        "large_download_expected_sha256": large_hash,
    }


def post_x11_click(pid: int, x: str, y: str) -> bool:
    search = subprocess.run(
        ["xdotool", "search", "--onlyvisible", "--all", "--pid", str(pid),
         "--name", "^Muffin · M0b validation$"],
        capture_output=True, text=True, timeout=4, check=False,
    )
    windows = search.stdout.split()
    if len(windows) != 1 or not windows[0].isdecimal():
        return False
    click = subprocess.run(
        ["xdotool", "mousemove", "--window", windows[0], x, y, "click", "1"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=4, check=False,
    )
    return click.returncode == 0


def run(binary: Path, out: Path, data: Path, origins: dict, mode: str, profile: str) -> dict:
    arguments = [
        str(binary), "--data-dir", str(data), "--fixture", origins["origin"],
        origins["peer"], "--smoke", mode,
    ]
    if profile == "x11":
        arguments.append("--smoke-x11-gesture")
    elif profile == "core-only":
        arguments.append("--smoke-core-only")
    log_file = out / f"{mode}.log"
    gesture = None
    with log_file.open("wb") as stream, subprocess.Popen(
        arguments, stdout=stream, stderr=subprocess.STDOUT,
    ) as child:
        deadline = time.monotonic() + 115
        while child.poll() is None:
            if time.monotonic() > deadline:
                child.kill()
                raise RuntimeError("native smoke exceeded its 115-second bound")
            if profile == "x11" and mode == "first" and gesture is None:
                trace = log_file.read_text(encoding="utf-8", errors="replace")
                marker = re.search(r"^fixture_input=x11_popup x=(\d+) y=(\d+)$", trace, re.MULTILINE)
                if marker:
                    gesture = post_x11_click(child.pid, *marker.groups())
            time.sleep(0.08)
        returncode = child.returncode
    log = log_file.read_text(encoding="utf-8", errors="replace")
    receipt = identity(log)
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if receipt["binary_sha256"] != digest:
        raise RuntimeError("native binary hash did not match the executable")
    cases = {}
    for line in log.splitlines():
        match = re.fullmatch(r"smoke_case=(\w+) result=(pass|fail|inconclusive)", line)
        if match:
            cases[match[1]] = match[2]
    if mode == "first":
        popup = [cases.get(name) for name in (
            "popup_opener_and_cross_origin_message", "popup_preserved_opener", "popup_webview_close",
        )]
        popup_status = "pass" if popup == ["pass"] * 3 else "inconclusive" if profile == "core-only" else "fail"
    else:
        popup_status = "not_applicable"
    return {
        "exit": returncode,
        "required_cases_ok": "fail" not in cases.values() and "smoke_complete required_cases_ok=true" in log,
        "popup": popup_status,
        "native_x11_gesture_posted": gesture,
        "identity": receipt,
        "cases": cases,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--media-dir", type=Path)
    profile = parser.add_mutually_exclusive_group()
    profile.add_argument("--x11-synthetic-input", action="store_true", help="send XTEST only inside a disposable Xvfb display")
    profile.add_argument("--core-only", action="store_true", help="record lack of popup gesture capability explicitly")
    args = parser.parse_args()
    profile = "x11" if args.x11_synthetic_input else "core-only" if args.core_only else "desktop"
    binary = args.binary.resolve(strict=True)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="muffin-m0b-native-") as temporary:
        root = Path(temporary)
        ready = root / "ready.json"
        route_log = out / "fixture-routes.jsonl"
        route_log.write_text("", encoding="utf-8")
        owners = server.serve(ready, route_log, args.media_dir)
        try:
            origins = json.loads(ready.read_text(encoding="utf-8"))
            first = run(binary, out, root / "app-data", origins, "first", profile)
            downloads = verify_downloads(root / "app-data")
            if sys.platform == "win32":
                # Give WebView2's browser process a bounded flush interval
                # after the first host process and its controllers have closed.
                time.sleep(2)
            relaunch = run(binary, out, root / "app-data", origins, "relaunch", profile)
            result = {"first": first, "downloads": downloads, "relaunch": relaunch}
            (out / "summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            okay = first["exit"] == relaunch["exit"] == 0 and first["required_cases_ok"] and relaunch["required_cases_ok"]
            okay = okay and downloads["count"] == 2 and first["identity"] == relaunch["identity"]
            okay = okay and first["popup"] != "fail"
            if first["identity"]["os"] == "macos":
                okay = okay and downloads["large_download_bytes"] == [len(server.LARGE_BLOCK) * server.LARGE_BLOCKS]
            if profile == "x11":
                okay = okay and first["native_x11_gesture_posted"] is True
            print(f"required native smoke {'passed' if okay else 'failed'}; desktop popup {first['popup']}")
            return 0 if okay else 1
        finally:
            for owner in owners:
                owner.shutdown()
                owner.server_close()


if __name__ == "__main__":
    raise SystemExit(main())
