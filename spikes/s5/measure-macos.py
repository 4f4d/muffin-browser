"""Compare scenario-matched loopback loads in isolated macOS browser processes."""
import argparse
import ctypes
import hashlib
import http.server
import importlib.util
import json
import os
import plistlib
import signal
import subprocess
import tempfile
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import parse_qs, urlparse

FLOOR_SOURCE = Path(__file__).resolve().parent.parent / "engine-floor/macos/measure.py"
FLOOR_SPEC = importlib.util.spec_from_file_location("muffin_floor_measure", FLOOR_SOURCE)
FLOOR = importlib.util.module_from_spec(FLOOR_SPEC)
FLOOR_SPEC.loader.exec_module(FLOOR)

PAGE = """<!doctype html><title>Muffin performance fixture</title>
<h1>Muffin deterministic tab</h1><p>This page is served locally.</p>
<script>addEventListener('load', () => fetch('/ready' + location.search, {cache:'no-store'}));</script>"""
REFS = {
    "chrome": Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
    "firefox": Path("/Applications/Firefox.app/Contents/MacOS/firefox"),
}
PROC = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
PROC.proc_pid_rusage.argtypes = (ctypes.c_int, ctypes.c_int, ctypes.c_void_p)


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), Handler)
        self.ready = {}
        self.lock = threading.Lock()

    @property
    def origin(self):
        return f"http://127.0.0.1:{self.server_port}"


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        url = urlparse(self.path)
        slot = parse_qs(url.query).get("slot", [""])[0]
        if slot not in {str(i) for i in range(0, 11)} or url.path not in {"/tab", "/ready"}:
            self.send_error(404)
            return
        body = PAGE.encode() if url.path == "/tab" else b""
        if url.path == "/ready":
            with self.server.lock:
                self.server.ready.setdefault(slot, time.monotonic_ns())
        self.send_response(200 if body else 204)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def read_usage(pid):
    info = FLOOR.RusageV0()
    if PROC.proc_pid_rusage(pid, 0, ctypes.byref(info)):
        return {"usage_error": os.strerror(ctypes.get_errno())}
    return {"resident_bytes": info.resident_size, "physical_bytes": info.phys_footprint,
            "user_ns": info.user_time, "system_ns": info.system_time,
            "start_abstime": info.proc_start_abstime}


def role(name, root, pid):
    if pid == root:
        return "host"
    name = name.lower()
    if "webcontent" in name or "renderer" in name or "tab" in name:
        return "renderer"
    if "gpu" in name:
        return "gpu"
    if "network" in name or "socket" in name:
        return "network"
    arguments = subprocess.run(["ps", "-p", str(pid), "-o", "args="], capture_output=True, text=True).stdout
    if "network.mojom.NetworkService" in arguments or " socket " in arguments:
        return "network"
    if "--type=gpu-process" in arguments:
        return "gpu"
    if "--type=renderer" in arguments or arguments.rstrip().endswith(" tab"):
        return "renderer"
    return "other"


def snapshot(root, before, include_webkit):
    current = FLOOR.processes()
    tree = {root}
    while True:
        extra = {pid for pid, (parent, _) in current.items() if parent in tree} - tree
        if not extra:
            break
        tree |= extra
    associated = {
        pid for pid, (_, cmd) in current.items()
        if include_webkit and pid not in before and Path(cmd).name in FLOOR.HELPERS
    } - tree
    observed = []
    for pid in sorted((tree | associated) & current.keys()):
        parent, command = current[pid]
        name = Path(command).name
        observed.append({"pid": pid, "ppid": parent, "name": name, "role": role(name, root, pid),
                         "association": "new_webkit_helper" if pid in associated else "descendant_tree",
                         **read_usage(pid)})
    return {"at_monotonic_ns": time.monotonic_ns(), "processes": observed}


def command(suite, binary, profile, origin, tabs):
    if suite == "wry":
        return [binary, origin, str(tabs), "75"]
    urls = [f"{origin}/tab?slot={index}" for index in range(tabs)]
    if suite == "chrome":
        return [binary, f"--user-data-dir={profile}", "--no-first-run", "--no-default-browser-check",
                "--disable-background-networking", "--disable-sync", "--new-window", *urls]
    return [binary, "-no-remote", "-profile", profile, "-new-window", urls[0],
            *(arg for url in urls[1:] for arg in ("-new-tab", url))]


def shutdown(app):
    if app.poll() is not None:
        return
    # Only the new session created for this test process is signalled.
    try:
        os.killpg(app.pid, signal.SIGTERM)
    except ProcessLookupError:
        app.wait(timeout=3)
        return
    try:
        app.wait(timeout=3)
    except subprocess.TimeoutExpired:
        os.killpg(app.pid, signal.SIGKILL)
        app.wait(timeout=3)


def benchmark(server, suite, binary, profile, tabs, log_dir, repetition, cohort):
    with server.lock:
        server.ready.clear()
    baseline = FLOOR.processes()
    key = f"{suite}-{tabs}-{repetition}-{cohort}"
    log_path = log_dir / f"{key}.log"
    args = command(suite, binary, profile, server.origin, tabs)
    launch = time.monotonic_ns()
    with log_path.open("w") as log:
        app = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            deadline = time.monotonic() + 17
            ready = {}
            while app.poll() is None and time.monotonic() < deadline:
                with server.lock:
                    ready = dict(server.ready)
                if tabs == 0 and "shell_created_elapsed_ms=" in log_path.read_text(errors="replace"):
                    ready = {"shell": time.monotonic_ns()}
                    break
                if tabs and {str(i) for i in range(tabs)} <= ready.keys():
                    break
                time.sleep(0.025)
            successful = len(ready) >= max(1, tabs) and app.poll() is None
            samples = []
            if successful:
                time.sleep(1.5)
                samples.append(snapshot(app.pid, baseline, suite == "wry"))
                time.sleep(1.0)
                samples.append(snapshot(app.pid, baseline, suite == "wry"))
            elapsed = {slot: (stamp - launch) / 1e6 for slot, stamp in sorted(ready.items())}
            return {
                "suite": suite, "tabs": tabs, "pair": repetition, "cohort": cohort,
                "all_ready_observed": successful, "ready_ms_by_slot": elapsed,
                "process_exit_before_cleanup": app.poll(), "samples": samples, "source_log": key,
            }
        finally:
            shutdown(app)


def metadata(binary):
    bundle = binary.parents[2]
    with (bundle / "Contents/Info.plist").open("rb") as plist:
        info = plistlib.load(plist)
    return {"version": info.get("CFBundleShortVersionString"), "build": info.get("CFBundleVersion"),
            "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "binary_bytes": binary.stat().st_size,
            "binary_arch": FLOOR.output("lipo", "-archs", binary)}


def write_report(path, report):
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wry", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--pairs", type=int, default=2, choices=range(1, 4))
    parser.add_argument("--suites", nargs="+", choices=("wry", *REFS), default=["wry", *REFS])
    args = parser.parse_args()
    origin = args.work_dir.resolve()
    origin.mkdir(parents=True)
    binaries = {name: {"wry": args.wry.resolve(), **REFS}[name] for name in args.suites}
    data = {
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "source_commit": FLOOR.output("git", "rev-parse", "HEAD"),
        "os": FLOOR.output("sw_vers"),
        "machine": FLOOR.output("sysctl", "-n", "hw.model"),
        "host_architecture": FLOOR.output("uname", "-m"),
        "memory_bytes": int(FLOOR.output("sysctl", "-n", "hw.memsize")),
        "logical_cpus": int(FLOOR.output("sysctl", "-n", "hw.ncpu")),
        "cpu": FLOOR.output("sysctl", "-n", "machdep.cpu.brand_string"),
        "power_source": FLOOR.output("pmset", "-g", "batt").splitlines()[0],
        "webkit": FLOOR.output("defaults", "read", "/System/Library/Frameworks/WebKit.framework/Versions/A/Resources/Info", "CFBundleVersion"),
        "binaries": {name: metadata(binary) for name, binary in binaries.items()},
        "method": {
            "ready": "elapsed from new process launch to last local page load beacon; shell-only poll when no webview; not first paint",
            "profiles": "fresh disposable comparator profile, followed by process relaunch with reused profile; WRY dedicated persistent fixture store shared across its launches",
            "temperature": "OS caches not flushed; first and repeat denote process order only, not cold disk boots",
            "memory": "Darwin RUSAGE_INFO_V0 physical footprint and RSS, separate host tree and estimated launch-associated WebKit helpers",
            "idle": "one pair of samples, first 1.5 s after readiness; measure CPU counters for process identities present in both",
            "chrome_flags": "--no-first-run --no-default-browser-check --disable-background-networking --disable-sync; temporary separate user-data-dir",
            "firefox_prefs": "temporary profile; startup page pinned, default browser check/suggestions/telemetry disabled",
            "workload": "same small loopback page, no-store, no page scripts except the load beacon; all requested tabs must report",
        },
        "runs": [],
    }
    dest = origin / "benchmark.json"
    write_report(dest, data)
    with Server() as server:
        serving = threading.Thread(target=server.serve_forever, daemon=True)
        serving.start()
        try:
            for suite, binary in binaries.items():
                for tabs in ((0, 1, 5, 10) if suite == "wry" else (1, 5, 10)):
                    for pair in range(1, args.pairs + 1):
                        with tempfile.TemporaryDirectory(prefix="muffin-s5-profile-", dir=origin) as tmp:
                            profile = Path(tmp)
                            if suite == "firefox":
                                (profile / "user.js").write_text(
                                    'user_pref("browser.shell.checkDefaultBrowser", false);\n'
                                    'user_pref("browser.startup.page", 0);\n'
                                    'user_pref("browser.aboutwelcome.enabled", false);\n'
                                    'user_pref("datareporting.healthreport.uploadEnabled", false);\n',
                                    encoding="utf-8",
                                )
                            for cohort in ("first_process", "repeat_process"):
                                row = benchmark(server, suite, binary, profile, tabs, origin, pair, cohort)
                                data["runs"].append(row)
                                write_report(dest, data)
                                print(f"{suite} {tabs} {pair} {cohort} ready={row['all_ready_observed']}", flush=True)
        finally:
            server.shutdown()


if __name__ == "__main__":
    main()
