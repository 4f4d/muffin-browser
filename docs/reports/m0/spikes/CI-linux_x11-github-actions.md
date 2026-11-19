---
spike: CI
platform: linux_x11
substrate: github-actions
budget_minutes: 60
started_at: 2026-11-19T13:48:44Z
deadline_at: 2026-11-19T14:48:44Z
stopped_at: 2026-11-19T14:04:47Z
stopped_reason: completed
outcome: pass
---

# Spike report: CI on linux_x11 (github-actions)

## Method

Set up a standard Ubuntu 24.04 x64 runner workflow to build the GTK3/WebKitGTK host, exercise native X11 and Wayland display backends in a VM, and upload source-identified artifacts.

## Results

The initial [workflow run](https://github.com/4f4d/muffin-browser/actions/runs/44537537215) completed both backend jobs and uploaded separate [X11](https://github.com/4f4d/muffin-browser/actions/runs/44537537215/artifacts/14535838414) and [Wayland](https://github.com/4f4d/muffin-browser/actions/runs/44537537215/artifacts/14538495289) binaries and logs. Logs are preserved in `docs/reports/m0/evidence/engine-floor-linux-x11.txt` and `docs/reports/m0/evidence/engine-floor-linux-wayland.txt`.

## Findings and implications

Minimum native Linux workflow and artifact transport worked. The original Wayland job recovered after a prolonged apt step. A [follow-up workflow run](https://github.com/4f4d/muffin-browser/actions/runs/44539079446) with shared dependencies was still running when this minimum gate closed. Candidate shell jobs and desktop rendering are tracked by S1.
