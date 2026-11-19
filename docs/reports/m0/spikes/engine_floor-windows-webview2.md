---
spike: engine_floor
platform: windows
substrate: webview2
budget_minutes: 45
started_at: 2026-11-19T13:48:44Z
deadline_at: 2026-11-19T14:33:44Z
stopped_at: 2026-11-19T13:55:54Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: engine_floor on windows (webview2)

## Method

Built a native Win32 single-window WebView2 host with SDK 1.0.4314.35 and let its navigation-completed callback check `about:blank`. Ran on standard GitHub Actions `windows-2025` x64, Windows Server build 10.0.26100.0.

## Results

WebView2 Runtime 156.0.4314.58 reported a successful `about:blank` load. Binary size 189,952 bytes; the downloaded artifact's binary hash matches the runner's log. See `docs/reports/m0/evidence/engine-floor-windows.txt`, [runner](https://github.com/4f4d/muffin-browser/actions/runs/44537867658), and [binary artifact](https://github.com/4f4d/muffin-browser/actions/runs/44537867658/artifacts/14536933287). The first static link failed on Win32 imports; its log is preserved in `docs/reports/m0/evidence/engine-floor-windows-link-attempt.txt`.

## Findings and implications

A runner-observed engine launch is available. This is a Windows Server VM, so representative startup, process memory and installed runtime footprint remain unavailable. The executable size excludes Evergreen dependencies or an installer.

The final report was committed again after a same-second Git timestamp check. See docs/reports/m0/evidence/report-clock-check.txt.
