"""Send one native pointer click to the S2a fixture on a private Xvfb display."""
import argparse
import ctypes
import json
import re
import subprocess
import time
from pathlib import Path


def locator(events):
    for line in events.read_text().splitlines():
        record = json.loads(line)["event"]
        if record["kind"] == "script-button-center":
            return record["data"]
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("events", type=Path)
    args = parser.parse_args()
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        target = locator(args.events) if args.events.exists() else None
        if target is not None:
            break
        time.sleep(0.1)
    else:
        raise SystemExit("No fixture button geometry received")
    window_info = subprocess.check_output(
        ["xwininfo", "-name", "Muffin S2a popup fixture"], text=True, timeout=3
    )
    left = int(re.search(r"Absolute upper-left X:\s*(-?\d+)", window_info)[1])
    top = int(re.search(r"Absolute upper-left Y:\s*(-?\d+)", window_info)[1])
    x = left + round(target["x"] * target["dpr"])
    y = top + round(target["y"] * target["dpr"])
    print(f"xvfb_fixture_click x={x} y={y} dpr={target['dpr']}", flush=True)

    x11 = ctypes.CDLL("libX11.so.6")
    xtst = ctypes.CDLL("libXtst.so.6")
    display_type = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = display_type
    x11.XSync.argtypes = [display_type, ctypes.c_int]
    x11.XCloseDisplay.argtypes = [display_type]
    xtst.XTestFakeMotionEvent.argtypes = [display_type, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
    xtst.XTestFakeButtonEvent.argtypes = [display_type, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    display = x11.XOpenDisplay(None)
    if not display:
        raise SystemExit("Cannot open fixture X display")
    try:
        assert xtst.XTestFakeMotionEvent(display, 0, x, y, 0), "XTEST motion failed"
        assert xtst.XTestFakeButtonEvent(display, 1, True, 0), "XTEST press failed"
        assert xtst.XTestFakeButtonEvent(display, 1, False, 50), "XTEST release failed"
        x11.XSync(display, False)
    finally:
        x11.XCloseDisplay(display)


if __name__ == "__main__":
    main()
