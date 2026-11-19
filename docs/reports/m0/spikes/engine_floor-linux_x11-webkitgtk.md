---
spike: engine_floor
platform: linux_x11
substrate: webkitgtk
budget_minutes: 45
started_at: 2026-11-19T13:48:44Z
deadline_at: 2026-11-19T14:33:44Z
stopped_at: 2026-11-19T13:55:54Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: engine_floor on linux_x11 (webkitgtk)

## Method

Built a single GTK3 window with an ephemeral WebKitGTK context, forced the X11 backend through Xvfb and waited for WebKit's load-finished event.

## Results

Ubuntu 24.04.5 x86_64, GTK 3.24.41 and WebKitGTK 2.52.6 loaded `about:blank` with `GdkX11Display`. The host binary is 17,376 bytes, and the downloaded binary hash matches the runner's log. See `docs/reports/m0/evidence/engine-floor-linux-x11.txt`, [runner](https://github.com/4f4d/muffin-browser/actions/runs/44537537215), and [binary artifact](https://github.com/4f4d/muffin-browser/actions/runs/44537537215/artifacts/14535838414).

## Findings and implications

The runner log establishes an X11 engine launch and records DRI3 and accessibility-bus warnings. Native GPU rendering, representative startup/memory and installed runtime footprint remain unavailable on this VM. The executable size excludes system packages or an installer.

The final report was committed again after a same-second Git timestamp check. See docs/reports/m0/evidence/report-clock-check.txt.
