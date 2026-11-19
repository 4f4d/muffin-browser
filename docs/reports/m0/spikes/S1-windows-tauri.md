---
spike: S1
platform: windows
substrate: tauri
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:44:06Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: S1 on windows (tauri)

## Method

Ran Tauri 2.12.7 with the unstable child-WebView API and runtime-wry 2.12.5 in the same Windows x64 runner comparison. The host captured actual document viewports and the native window.

## Results

`docs/reports/m0/evidence/s1-windows-tauri.txt` records exact source, engine and binary identity. Chrome/content dimensions, resize and the 270×144 overlay matched the configured layout. The content reached /next, all four group sizes were exercised with close calls, and both programmatic focus queries returned true. The scoped image `docs/reports/m0/evidence/s1-windows-tauri.png` shows overlapping composition. The loopback page could see `window.ipc` and `window.__TAURI_INTERNALS__`. Fixture code registers no privileged command. Reachability of privileged app capabilities remains untested.

## Findings and implications

Stock child-window layout is feasible for this runner fixture. The visible bridge surface is a security design item. Native shortcut and crash recovery outcomes, Windows 11 desktop and mixed DPI remain unverified.
