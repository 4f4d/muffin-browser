---
spike: engine_floor
platform: macos
substrate: wkwebview
budget_minutes: 45
started_at: 2026-11-19T13:44:59Z
deadline_at: 2026-11-19T14:29:59Z
stopped_at: 2026-11-19T13:47:04Z
stopped_reason: completed
outcome: pass
---

# Spike report: engine_floor on macos (wkwebview)

## Method

Built the bare, single-window AppKit/WKWebView host from source commit `fe451df41ec15258119f2e2259e9f5b1a9141f22`. Five separate `about:blank` launches on macOS 15.8.3 (24H241), Apple M4 arm64, on battery. Readiness was a finished navigation plus JavaScript result. Process memory was sampled two seconds later with Darwin `proc_pid_rusage`.

## Results

`docs/reports/m0/evidence/engine-floor-macos.json` contains the tested binary SHA-256, exact WebKit bundle version, hardware and power configuration, every sample, and reproduction methodology. Navigation readiness was 337–1,809 ms (median 376 ms). The app process used 18.1–18.7 MiB physical footprint. The sum including the three newly observed WebKit helpers was approximately 55.6–56.6 MiB. The helpers were parented by launchd and attributed by their appearance after each launch. All five trials listed the same three helper roles. The test bundle was 83,743 bytes by file sizes, 88 KiB by allocated blocks, and 15,978 bytes as a ZIP. The runtime is supplied by macOS and excluded from those bundle sizes.

## Findings and implications

The host supplies a WKWebView readiness and footprint reference for this machine and power configuration. The first launch was substantially slower than the next four. There is no first-paint metric and no instrumented proof of helper ownership or apportioned shared pages. The observed sum is an estimate of the app plus associated helpers. The launch-time distribution is retained without dropping the slow sample.

## What remains

Reproduce on other host/power conditions before generalizing, and use a matched scenario for substrate overhead.
