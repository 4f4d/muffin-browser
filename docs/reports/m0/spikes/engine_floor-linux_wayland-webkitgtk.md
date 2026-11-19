---
spike: engine_floor
platform: linux_wayland
substrate: webkitgtk
budget_minutes: 45
started_at: 2026-11-19T13:48:44Z
deadline_at: 2026-11-19T14:33:44Z
stopped_at: 2026-11-19T14:04:47Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: engine_floor on linux_wayland (webkitgtk)

## Method

Built the same bare GTK3/WebKitGTK host as X11. Connected GDK to a headless Weston 13 Wayland compositor using the Pixman path, unset X11 DISPLAY, and waited for WebKit load-finished on `about:blank`.

## Results

Ubuntu 24.04.5 x86_64, GTK 3.24.41 and WebKitGTK 2.52.6 loaded `about:blank` with `GdkWaylandDisplay`. Downloaded binary 17,376 bytes, hash verified against the runner log. See `docs/reports/m0/evidence/engine-floor-linux-wayland.txt`, [runner](https://github.com/4f4d/muffin-browser/actions/runs/44537537215), and [artifact](https://github.com/4f4d/muffin-browser/actions/runs/44537537215/artifacts/14538495289).

## Findings and implications

The runner proves a bare WebKitGTK load over the headless Wayland backend. Cursor, GDK seat and Mesa/EGL warnings are in the artifact. No GNOME desktop session, hardware rendering, representative startup/memory, or installed footprint was measured.
