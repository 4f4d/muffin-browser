"""Measure a bare WKWebView on a local macOS desktop. Requires Python 3."""
import argparse
import ctypes
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import selectors
import subprocess
import time

HELPERS = {
    "com.apple.WebKit.WebContent",
    "com.apple.WebKit.Networking",
    "com.apple.WebKit.GPU",
}


def output(*command):
    return subprocess.check_output(command, text=True).strip()


def processes():
    rows = output("ps", "-A", "-o", "pid=,ppid=,comm=").splitlines()
    return {int(pid): (int(parent), command) for pid, parent, command in
            (row.strip().split(maxsplit=2) for row in rows)}


class RusageV0(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [
        (name, ctypes.c_uint64) for name in (
            "user_time", "system_time", "pkg_idle_wakeups", "interrupt_wakeups",
            "pageins", "wired_size", "resident_size", "phys_footprint",
            "proc_start_abstime", "proc_exit_abstime",
        )
    ]


def memory(pid):
    proc = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
    proc.proc_pid_rusage.argtypes = (ctypes.c_int, ctypes.c_int, ctypes.c_void_p)
    info = RusageV0()
    if proc.proc_pid_rusage(pid, 0, ctypes.byref(info)) != 0:
        return {"error": os.strerror(ctypes.get_errno())}
    return {"resident_bytes": info.resident_size, "physical_footprint_bytes": info.phys_footprint}


def measure(binary):
    before = processes()
    start = time.monotonic_ns()
    with subprocess.Popen([binary], stdout=subprocess.PIPE, text=True) as app:
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(app.stdout, selectors.EVENT_READ)
                if not selector.select(20):
                    raise RuntimeError("WKWebView readiness timed out")
                page = json.loads(app.stdout.readline())
            elapsed = (time.monotonic_ns() - start) / 1_000_000
            time.sleep(2)
            after = processes()
            tree = {app.pid}
            while True:
                children = {pid for pid, (parent, _) in after.items() if parent in tree}
                if children.issubset(tree):
                    break
                tree.update(children)
            new_helpers = {
                pid: (parent, Path(command).name) for pid, (parent, command) in after.items()
                if pid not in before and Path(command).name in HELPERS
            }
            return {
                "navigation_ready_ms": round(elapsed, 3), "page": page,
                "parent_tree": [{"pid": pid, "process": Path(after[pid][1]).name, **memory(pid)}
                                for pid in sorted(tree)],
                "new_webkit_helpers": [{"pid": pid, "parent_pid": parent, "process": name, **memory(pid)}
                                       for pid, (parent, name) in sorted(new_helpers.items())],
            }
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    app.kill()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    binary = args.bundle.resolve() / "Contents/MacOS/muffin-floor"
    files = [path for path in args.bundle.rglob("*") if path.is_file()]
    metadata = {
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "source_commit": output("git", "rev-parse", "HEAD"),
        "binary_architecture": output("lipo", "-archs", str(binary)),
        "binary_sha256": output("shasum", "-a", "256", str(binary)).split()[0],
        "os_version": output("sw_vers", "-productVersion"),
        "os_build": output("sw_vers", "-buildVersion"),
        "machine": output("sysctl", "-n", "hw.model"),
        "memory_bytes": int(output("sysctl", "-n", "hw.memsize")),
        "cpu_count": int(output("sysctl", "-n", "hw.ncpu")),
        "power": output("pmset", "-g", "batt").splitlines()[0],
        "power_configuration": output("pmset", "-g", "custom"),
        "webkit_framework_bundle_version": output(
            "defaults", "read", "/System/Library/Frameworks/WebKit.framework/Versions/A/Resources/Info",
            "CFBundleVersion",
        ),
        "runtime_distribution": "OS-provided Apple WebKit",
        "binary_bytes": binary.stat().st_size,
        "bundle_file_bytes": sum(path.stat().st_size for path in files),
        "bundle_disk_kib": int(output("du", "-sk", str(args.bundle)).split()[0]),
        "zip_bytes": args.archive.stat().st_size,
        "method": {
            "startup": "new host process to navigation finished and JavaScript response; not first paint",
            "profile": "nonpersistent WKWebsiteDataStore",
            "sampling": "five sequential launches; sample process memory two seconds after navigation, then terminate",
            "memory": "Darwin proc_pid_rusage RUSAGE_INFO_V0; parent tree and new system WebKit helpers reported separately",
            "attribution": "new WebKit helper PIDs measured by prelaunch versus post-navigation process snapshots; timing association",
        },
        "runs": [measure(binary) for _ in range(5)],
    }
    print(json.dumps(metadata, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
