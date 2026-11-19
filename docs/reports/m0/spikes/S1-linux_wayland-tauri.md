---
spike: S1
platform: linux_wayland
substrate: tauri
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:44:06Z
stopped_reason: completed
outcome: fail
---

# Spike report: S1 on linux_wayland (tauri)

## Method

Ran the same Tauri build under headless Weston on Ubuntu 24.04.5 and inspected actual child DOM viewport dimensions and local route results.

## Results

The logs at `docs/reports/m0/evidence/s1-wayland-tauri.txt` show 207/207-pixel chrome/content heights at startup, instead of 72/548, and 234/233-pixel heights after resize. The third child had the about:blank error document and a 206-pixel height, outside its configured overlay bounds. The loopback route and four close groups completed. Both focus readings were false in this environment, which did not supply an interactive desktop seat. See [runner output](https://github.com/4f4d/muffin-browser/actions/runs/44544220053/artifacts/14540046912).

## Findings and implications

The stock Tauri layout failed the requested DOM geometry criterion on this headless backend. This outcome applies to child allocation and does not establish graphical stacking, GPU behavior or desktop-Wayland focus. No native accelerator or process-crash recovery test ran.
