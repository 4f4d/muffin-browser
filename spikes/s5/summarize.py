"""Derive the S5 comparison table from retained macOS raw samples."""
import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path
from statistics import median

MIB = 1024**2


def row_stats(row):
    if not row["all_ready_observed"] or len(row["samples"]) != 2:
        return None
    initial, final = row["samples"]
    before = {(p["pid"], p.get("start_abstime")): p for p in initial["processes"]}
    after = {(p["pid"], p.get("start_abstime")): p for p in final["processes"]}
    roles = defaultdict(float)
    for process in after.values():
        if "physical_bytes" not in process:
            return None
        roles[process["role"]] += process["physical_bytes"] / MIB
    if len([p for p in after.values() if p["role"] == "host"]) != 1:
        return None
    stable = before.keys() & after.keys()
    elapsed = final["at_monotonic_ns"] - initial["at_monotonic_ns"]
    if elapsed <= 0:
        return None
    cpu_delta = sum(
        after[key]["user_ns"] + after[key]["system_ns"] - before[key]["user_ns"] - before[key]["system_ns"]
        for key in stable if "user_ns" in before[key] and "user_ns" in after[key]
    )
    ready = row["ready_ms_by_slot"].values()
    return {"first_ready_ms": min(ready), "last_ready_ms": max(ready),
            "host_mib": roles["host"], "observed_total_mib": sum(roles.values()),
            "renderer_mib": roles["renderer"], "gpu_mib": roles["gpu"], "network_mib": roles["network"],
            "other_mib": roles["other"], "process_count": len(after),
            "stable_process_count": len(stable), "idle_cpu_percent_of_one_core": cpu_delta * 100 / elapsed}


def summarize(data):
    if not data.get("mach_timebase"):
        raise ValueError("CPU samples require the corrected recorder with a Mach timebase")
    groups = defaultdict(list)
    attempts = Counter()
    for row in data["runs"]:
        key = (row["suite"], row["tabs"], row["cohort"])
        attempts[key] += 1
        stats = row_stats(row)
        if stats is not None:
            groups[key].append(stats)
    result = []
    for key in sorted(attempts):
        suite, tabs, cohort = key
        rows = groups[key]
        medians = {name: round(median(row[name] for row in rows), 3) for name in rows[0]} if rows else {}
        result.append({"suite": suite, "tabs": tabs, "cohort": cohort,
                       "attempted": attempts[key], "used": len(rows), "medians": medians})
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    args = parser.parse_args()
    print(json.dumps(summarize(json.loads(args.raw.read_text())), indent=2, sort_keys=True))
