"""Check the retained native and HTTP event logs from one S1 trust-boundary run."""
import argparse
import json
from pathlib import Path


def verify(native: str, http_events: list[dict]) -> None:
    events = {event["name"]: event["data"] for event in http_events}
    probes = events["fetch-probes"]
    expected = (
        (probes["positive_control"] == {"status": 200, "expected": True}, "public control"),
        ("error" in probes["private_file"], "synthetic local file read"),
        ("error" in probes["chrome_uri"], "private chrome document read"),
        (events["command-attempt"]["scheme_link"] == "activated", "command URL attempt"),
        (events["inert-postmessage"]["observed"] is True, "page-local postMessage"),
        (
            {event["data"]["target"] for event in http_events if event["name"] == "navigation-attempt"}
            == {"file", "chrome"},
            "both local navigation attempts",
        ),
    )
    for okay, name in expected:
        if not okay:
            raise ValueError(f"Missing expected boundary evidence for {name}")

    lines = native.splitlines()
    commands = [line for line in lines if line.startswith("native_command_count=")]
    if commands != ["native_command_count=1", "native_command_count=2"]:
        raise ValueError("The fixture must record exactly two authorized canary actions")
    before = lines.index(commands[0])
    after = lines.index(commands[1])
    required = {
        "navigation_denied view=content scheme=muffin-followup",
        "navigation_denied view=content scheme=muffin-chrome",
    }
    if not required.issubset(lines[before + 1:after]):
        raise ValueError("The untrusted command and chrome navigation denials must be between controls")
    if lines.count("chrome_ipc_received allowed=true") != 2 or "fixture_end commands=2" not in lines:
        raise ValueError("Native accepted-call and shutdown counters disagree")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("native", type=Path)
    parser.add_argument("http_events", type=Path)
    args = parser.parse_args()
    verify(args.native.read_text(), [json.loads(line) for line in args.http_events.read_text().splitlines()])
    print("S1 fixture boundary evidence matches the positive and negative checks.")
