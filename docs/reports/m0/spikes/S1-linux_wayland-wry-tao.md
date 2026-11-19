---
spike: S1
platform: linux_wayland
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:58:52Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: S1 on linux_wayland (wry-tao)

## Method

Ran native WRY 0.57.2 + TAO 0.37.2 with `gtk::Fixed` under Ubuntu 24.04.5 headless Weston. Verified GDK Wayland and attempted a second run on an isolated container on a physical x86_64 server, using a stable allocation window and a controlled WebKitGTK renderer termination/reload check.

## Results

Both runs recorded initial document widths of 900 pixels against a reported 952-pixel parent, with a 548-pixel content height against the expected 637. The fixture kept the failure for this initial-size criterion. The resized widths matched 1052 and content matched 717. Local navigation and all four view batches completed. The server run recorded `TerminatedByApi` and recovered the configured URL and title, with 900×548 reported after recovery. Both focus queries returned false and the compositor logs include absent-seat and EGL/Mesa warnings. See `docs/reports/m0/evidence/s1-wayland-wry-tao.txt` and `s1-wry-server-wayland-recovery.txt`.

## Findings and implications

Waiting for allocation did not close the initial geometry discrepancy in the headless environment. The GTK Wayland path demonstrates local navigation and controlled termination/reload. Interactive desktop focus, visual overlay stacking, retained layout during recovery, native shortcuts, IME, scaling and GPU rendering remain unverified. This is an inconclusive platform S1 rather than a shipping support decision.
