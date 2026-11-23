"""Measure the Apple Silicon debug shell against fixed loopback pages."""

import argparse
import ctypes
from hashlib import file_digest
import json
from pathlib import Path
import platform
import plistlib
import re
import subprocess
from tempfile import TemporaryDirectory
import time

from fixture import browser_fixture
from run_launchservices_smoke import ROOT, source_identity
from run_smoke import observation, send_command, until


TRIALS = 5
IDLE_SECONDS = 2
TAB_COUNTS = (1, 5, 10)


class RusageInfoV0(ctypes.Structure):
    # macOS SDK sys/resource.h, struct rusage_info_v0.
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [
        (field, ctypes.c_uint64) for field in (
            "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups",
            "pageins", "wired_size", "resident_size", "phys_footprint",
            "proc_start_abstime", "proc_exit_abstime",
        )
    ]


def shell_sampler():
    libproc = ctypes.CDLL("/usr/lib/libproc.dylib")
    query = libproc.proc_pid_rusage
    query.argtypes = (ctypes.c_int, ctypes.c_int, ctypes.POINTER(RusageInfoV0))
    query.restype = ctypes.c_int

    def sample(process):
        result = RusageInfoV0()
        if process.poll() is not None or query(process.pid, 0, ctypes.byref(result)) != 0:
            raise RuntimeError("could not query the dedicated debug shell's resource usage")
        return {
            "monotonic_ns": time.monotonic_ns(),
            "physical_footprint_bytes": result.phys_footprint,
            "user_cpu_ns": result.user_time,
            "system_cpu_ns": result.system_time,
        }

    return sample


def output(*command):
    return subprocess.check_output(command, cwd=ROOT, text=True, timeout=10).strip()


def machine():
    power = output("/usr/bin/pmset", "-g")
    low_power = re.search(r"^\s*lowpowermode\s+(\d+)\s*$", power, re.MULTILINE)
    return {
        "host_arch": platform.machine(),
        "model": output("/usr/sbin/sysctl", "-n", "hw.model"),
        "chip": output("/usr/sbin/sysctl", "-n", "machdep.cpu.brand_string"),
        "logical_cores": int(output("/usr/sbin/sysctl", "-n", "hw.logicalcpu")),
        "physical_memory_bytes": int(output("/usr/sbin/sysctl", "-n", "hw.memsize")),
        "macos_version": platform.mac_ver()[0],
        "os_build": output("/usr/bin/sw_vers", "-buildVersion"),
        "power_source": output("/usr/bin/pmset", "-g", "batt").splitlines()[0],
        "low_power_mode": None if not low_power else int(low_power.group(1)),
    }


def trial(binary, origin, log, sample, result):
    with TemporaryDirectory(prefix="muffin-shell-benchmark-", dir="/private/tmp") as temporary:
        control = Path(temporary)
        launch_start = time.monotonic_ns()
        process = subprocess.Popen([binary, "--test-control", control], stdout=log, stderr=log)
        try:
            def wait(label, matches):
                def read():
                    state = observation(control)
                    return state if state.get("chrome_ready") and matches(state) else None

                return until(process, label, read, timeout=12)

            wait("Start with chrome ready", lambda state: state["status"] == "home")
            result["startup_to_chrome_ready_ns"] = time.monotonic_ns() - launch_start
            selected = []
            for count in range(1, max(TAB_COUNTS) + 1):
                if count > 1:
                    send_command(control, "new_tab")
                    wait("new Start tab", lambda state: state["status"] == "home" and len(state["tabs"]) == count)
                url = f"{origin}/first?benchmark-tab={count}"
                start = sample(process)
                send_command(control, "navigate", value=url)
                state = wait("fixture page ready", lambda state: state["status"] == "ready" and state["url"] == url)
                end = sample(process)
                if state["selected_tab"] in selected or len(state["tabs"]) != count:
                    raise AssertionError("unexpected tab identity during benchmark")
                selected.append(state["selected_tab"])
                result["navigation"].append({"tabs": count, "start": start, "end": end})
                if count in TAB_COUNTS:
                    time.sleep(0.5)
                    start = sample(process)
                    time.sleep(IDLE_SECONDS)
                    end = sample(process)
                    result["idle"].append({"tabs": count, "start": start, "end": end})

            first_id = selected[0]
            start = sample(process)
            send_command(control, "discard_tab", id=first_id)
            wait("inactive view discarded", lambda state: any(
                row["id"] == first_id and row["discarded"] for row in state["tabs"]
            ))
            marked_discarded = sample(process)
            time.sleep(0.5)
            settled = sample(process)
            send_command(control, "select_tab", id=first_id)
            wait("discarded view restored", lambda state: (
                state["selected_tab"] == first_id and state["status"] == "ready"
                and state["url"] == f"{origin}/first?benchmark-tab=1"
                and any(row["id"] == first_id and not row["discarded"] for row in state["tabs"])
            ))
            result["discard_restore"] = {
                "start": start, "discarded": marked_discarded, "restore_start": settled, "restored": sample(process),
            }
            send_command(control, "quit_test")
            if process.wait(timeout=8) != 0:
                raise RuntimeError("benchmark shell failed to quit cleanly")
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/muffin")
    parser.add_argument("--verified-smoke", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True, help="new directory outside source control")
    args = parser.parse_args()
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        parser.error("use macOS Apple Silicon")
    commit, clean = source_identity()
    if not clean:
        parser.error("run from a clean benchmark checkout")
    binary = args.binary.resolve(strict=True)
    with binary.open("rb") as stream:
        binary_sha = file_digest(stream, "sha256").hexdigest()
    with args.verified_smoke.open("rb") as stream:
        smoke_sha = file_digest(stream, "sha256").hexdigest()
        stream.seek(0)
        smoke = json.load(stream)
    app_commit = smoke.get("source_commit", "")
    if (smoke.get("outcome") != "passed" or smoke.get("source_worktree_clean") is not True
            or smoke.get("binary_sha256") != binary_sha or not re.fullmatch("[a-f0-9]{40}", app_commit)):
        parser.error("use a debug binary with a matching clean native smoke receipt")
    output("git", "cat-file", "-e", app_commit + "^{commit}")
    environment = machine()
    with Path("/System/Library/Frameworks/WebKit.framework/Resources/Info.plist").open("rb") as stream:
        webkit = plistlib.load(stream)["CFBundleVersion"]
    if (environment["host_arch"], environment["macos_version"], webkit) != (
        smoke["host_arch"], smoke["macos_version"], smoke["webkit_bundle_version"]
    ):
        parser.error("benchmark on the verified host architecture, macOS and WebKit versions")
    sample = shell_sampler()
    args.out.mkdir()
    data = {
        "kind": "macos_debug_shell_benchmark",
        "process_scope": "Muffin shell PID only",
        "harness_commit": commit,
        "app_commit": app_commit,
        "binary_sha256": binary_sha,
        "native_debug_smoke_sha256": smoke_sha,
        "webkit_bundle_version": webkit,
        "environment": environment,
        "workload": {"trial_count": TRIALS, "idle_seconds": IDLE_SECONDS, "tab_counts": TAB_COUNTS},
        "trials": [],
    }
    try:
        with browser_fixture() as servers, (args.out / "browser.log").open("wb") as log:
            for _ in range(TRIALS):
                measurements = {"navigation": [], "idle": []}
                data["trials"].append(measurements)
                trial(binary, servers[0].origin, log, sample, measurements)
        if source_identity() != (commit, True):
            raise RuntimeError("benchmark source changed during measurement")
    except Exception as error:
        data.update(outcome="failed", error=str(error))
        raise
    else:
        data["outcome"] = "passed"
    finally:
        (args.out / "shell-benchmark.json").write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
