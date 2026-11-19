"""Check scoped Muffin native fixture results and workflow evidence from the repository."""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = ROOT / "docs/reports/m0/evidence"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(name):
    return (EVIDENCE / name).read_text(encoding="utf-8")


def events(raw):
    rows = [json.loads(line) for line in raw.splitlines()]
    return [row["event"] for row in rows if "event" in row]


def verify_mac_boundary():
    script = ROOT / "spikes/s1/followup/verify_boundary.py"
    for suffix in ("", "-bundle"):
        base = "s1-followup-macos" + suffix
        subprocess.run(
            ["python3", script, EVIDENCE / (base + "-native.txt"), EVIDENCE / (base + "-content.jsonl")],
            check=True,
        )
    log = read("s1-followup-macos-native.txt")
    for marker in ("appkit_shortcut=Chrome", "appkit_shortcut=Content", "appkit_test_key=a",
                   "appkit_test_key=b", "appkit_test_key=c", "dom_before_explicit_focus target=Chrome",
                   "dom_before_explicit_focus target=Content"):
        require(marker in log, f"Mac native input marker missing: {marker}")


def verify_popups():
    for target in ("macos", "windows"):
        data = events(read(f"continuation-s2a-{target}-content.jsonl"))
        ready = [event["data"] for event in data if event["kind"] == "popup-ready"]
        require(len(ready) == 7, f"{target}: missing seven observed popup pages")
        require(all(r["ipc"] == "undefined" for r in ready), f"{target}: fixture IPC exposed")
        cross = next(r for r in ready if r["path"] == "/popup/cross-origin")
        require(cross["opener"] and not cross["opener_read"], f"{target}: cross-origin opener state differs")
        blank = [r for r in ready if r["path"] in ("/popup/target-blank", "/popup/noopener")]
        require(len(blank) == 2 and all(not r["opener"] for r in blank), f"{target}: noopener evidence differs")
        require(any(e["kind"] == "auth-callback" for e in data), f"{target}: synthetic return missing")
    x11 = read("continuation-s2a-linux-x11-xtest.txt")
    require('"trusted":true' in x11 and '"active":true,"has_handle":true' in x11,
            "X11 trusted-pointer path is missing")
    wayland = read("continuation-s2a-linux-wayland.txt")
    require("popups=0" in wayland and "popup_request" not in wayland, "Wayland evidence no longer matches its limited outcome")
    print("popup_state=verified_fixture_subset")


def verify_downloads(name, native, content, result):
    expected = [("1", True), ("2", True), ("private", False)]
    requests = [(e["copy"], e["authorized"]) for e in content if e["kind"] == "download_request"]
    require(requests == expected, f"{name}: synthetic cookie gating differs")
    files = result["download_results"]
    require(result["core_observed"] and len(files) == 2, f"{name}: core outcome missing")
    require({f["name"] for f in files} == {"download-01.bin", "download-02.bin"},
            f"{name}: the collision check differs")
    require(all(f["bytes"] == 786432 and f["matches_expected"] for f in files),
            f"{name}: exact fixture payload absent")
    require("private_storage_isolated=true" in native and "reason=fixture-denied" in native,
            f"{name}: missing isolation or download denial")


def verify_everyday():
    for target in ("macos", "windows"):
        prefix = f"continuation-s4-{target}"
        verify_downloads(target, read(prefix + "-native.txt"), events(read(prefix + "-content.jsonl")),
                         json.loads(read(prefix + "-downloads.json")))
    for backend in ("x11", "wayland"):
        raw = read(f"continuation-s4-linux-{backend}.txt")
        native, rest = raw.split("----- content-events.jsonl\n", 1)
        body, tail = rest.split("----- launcher-summary.json\n", 1)
        summary = tail.split("fixture_runner_exit=", 1)[0]
        verify_downloads(backend, native, events(body), json.loads(summary))
        token = read(f"s1-wry-server-{backend}-recovery.txt")
        terminated = token.index("content_process_terminated=TerminatedByApi")
        require("inspection=recovered" in token[terminated:], f"{backend}: deliberate reload observation missing")
    for target in ("macos", "x11", "wayland"):
        picture = EVIDENCE / f"continuation-s4-pdf-{target}.png"
        digest = hashlib.sha256(picture.read_bytes()).hexdigest()
        require(digest in read(f"continuation-s4-engine-{target}.txt"), f"{target}: sample PDF capture hash differs")
    print("downloads_storage_and_controlled_recovery=verified_fixture_subset")


def check_overlay(magick, file, boundary, jpeg, expected):
    width, height = map(int, subprocess.check_output(
        [magick, "identify", "-format", "%w %h", file], text=True).split())
    rgb = subprocess.check_output([magick, file, "-alpha", "off", "-depth", "8", "rgb:-"])
    require(len(rgb) == width * height * 3, f"{file.name}: decoded size differs")
    found = []
    for offset in range(0, len(rgb), 3):
        r, g, b = rgb[offset:offset+3]
        red = (r > 120 and r > 2 * g and r > 1.5 * b) if jpeg else (r, g, b) == (179, 0, 54)
        if red:
            row, col = divmod(offset // 3, width)
            found.append((col, row))
    if expected is None:
        require(not found, f"{file.name}: negative overlay image contains fixture red")
        print(f"overlay_capture={file.name} red_pixels=0 negative_control=true")
        return
    require(bool(found), f"{file.name}: overlay colour absent")
    xs, ys = zip(*found)
    bbox = (min(xs), min(ys), max(xs), max(ys))
    tolerance = 2 if jpeg else 0
    require(all(abs(a - b) <= tolerance for a, b in zip(bbox, expected)), f"{file.name}: envelope differs {bbox}")
    require(min(ys) < boundary < max(ys), f"{file.name}: overlay does not cross both panes")
    print(f"overlay_capture={file.name} dimensions={width}x{height} red_pixels={len(found)} bbox={bbox} crosses_y={boundary}")


def verify_workflows():
    for path in sorted((ROOT / ".github/workflows").glob("*.yml")):
        data = path.read_text()
        refs = re.findall(r"^\s*-\s+uses:\s+(\S+)", data, re.MULTILINE)
        require(refs and all(re.fullmatch(r"[^@]+@[0-9a-f]{40}", rule) for rule in refs), f"{path.name}: action pin drift")
        require("pull_request_target:" not in data and "persist-credentials: false" in data,
                f"{path.name}: workflow trust settings differ")
        print(f"workflow={path.name} immutable_actions={len(refs)}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--magick", default="magick", help="local ImageMagick executable for retained capture checks")
    args = parser.parse_args()
    verify_mac_boundary()
    verify_popups()
    verify_everyday()
    verify_workflows()
    for name, boundary, jpeg, expected in (
        ("s1-followup-macos-overlay.jpg", 200, True, (420, 144, 939, 395)),
        ("s1-wry-linux-server-x11.png", 72, False, (190, 42, 459, 185)),
        ("s1-windows-wry-tao.png", 102, False, (190, 72, 459, 215)),
        ("s1-x11-tauri.png", 72, False, None),
    ):
        check_overlay(args.magick, EVIDENCE / name, boundary, jpeg, expected)
    print("S7 evidence subset verified")


if __name__ == "__main__":
    main()
