---
spike: S1
platform: linux_x11
substrate: tauri
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:44:06Z
stopped_reason: completed
outcome: fail
---

# Spike report: S1 on linux_x11 (tauri)

## Method

Ran Tauri 2.12.7 on Ubuntu 24.04.5 Xvfb, using the stock Linux child creation path. Compared DOM viewports against the same requested bounds as WRY and captured the actual app window.

## Results

Chrome and content started at heights 207 and 207 despite the planned 72 and 548. After resize, the panes were 234 and 233 pixels tall. The overlay filled the third vertical pane with an engine error surface. The snapshot `docs/reports/m0/evidence/s1-x11-tauri.png` shows the stacking. `docs/reports/m0/evidence/s1-x11-tauri.txt` includes binary identity, runtime observations, loopback navigation, focus results and all four groups. See [runner output](https://github.com/4f4d/muffin-browser/actions/runs/44544220053/artifacts/14540046912).

## Findings and implications

The stock Tauri child layout decisively failed the required positioned-bounds and overlay criterion in the Xvfb environment. This is consistent with the exact runtime source using a GTK vertical box, recorded in `docs/reports/m0/evidence/source-checks.tsv`. The overlay asset load also failed, a separate observation. Native accelerators and web-process recovery were not exercised. The platform product decision remains open.
