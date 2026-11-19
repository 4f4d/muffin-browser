# Independent M0a exit assessment

Reviewer: configured Codex reviewer (GPT-6 Luna)
Reviewed commit: `0d92691aa82ac96d62de9b532e32388039bcda3b`
Baseline: `016c915909b9a48d2fb223084f73088b11bd0650`

The reviewer accepted the accuracy of the blocked handoff and withheld gate advancement. The summary, ADR and status reflect mixed evidence and leave substrate and platform decisions with the human. [Handoff](M0A_SUMMARY.md)

## Verification performed independently

- `python3 -m unittest discover -s tools/tests -v` completed 78 tests in 8.291 seconds with `OK`.
- `python3 tools/gen_status.py --check` returned `OK`. The validator checked all 14 tracked reports, their commit histories, budgets and outcomes. The S1 start commit preceded the reported clock by 39 seconds. The last S1 report commit followed the reported stop by three seconds.
- `git diff --check 016c915909b9a48d2fb223084f73088b11bd0650 HEAD` passed. The reviewer checked the M0b result file against the baseline and found zero edits.
- The reviewer independently decoded the X11 WRY screenshots to RGB and verified the expected 270×144 red overlay. The physical-server capture has 37,943 exact fixture-red pixels with bounds (190,42)–(459,185) inclusive. This pixel result supersedes a preliminary visual-only misreading. [Pixel evidence](evidence/s1-image-check.txt)
- The 500 ms GTK allocation wait changed sampling time. The expected viewport bounds and pixel tolerance were preserved. The reviewer compared both renderer termination logs and confirmed the successful reloads, the viewport discrepancy after recovery and the different X11 and headless Wayland focus observations. [X11 log](evidence/s1-wry-server-x11-recovery.txt), [Wayland log](evidence/s1-wry-server-wayland-recovery.txt)

## Remaining limitations

The Linux Tauri reports use `fail/completed` because the decisive stock layout criterion was exercised and failed. Native shortcuts and process-crash recovery were untested in those reports. The kill rule applies to completed macOS or Windows S1 failures. [Linux X11](spikes/S1-linux_x11-tauri.md), [Linux Wayland](spikes/S1-linux_wayland-tauri.md), [generated status](../../STATUS.json)

The reviewer found no blocking factual mismatch. Full S1 still lacks Mac focus and visual capture, native accelerator routing, unplanned crash recovery, interactive Wayland graphics and input, and a negative test for privileged Tauri command reachability. Representative Linux desktop performance and package/runtime footprint remain unmeasured. [Substrate ADR](../../adr/0001-m0a-substrate-evidence.md)
