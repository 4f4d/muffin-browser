---
spike: S5
platform: macos
substrate: wry-tao
budget_minutes: 60
started_at: 2026-11-19T17:08:01Z
deadline_at: 2026-11-19T18:08:01Z
stopped_at: 2026-11-19T17:21:32Z
stopped_reason: completed
outcome: inconclusive
---

# S1 follow-up on macos

## Method and evidence

Collected a full 40-launch scenario dataset on real Apple M4 macOS with optimized WRY + TAO and isolated Chrome and Firefox comparators. The scenario spans a native window without WebViews and 1/5/10 fixed loopback WebViews, using two first and repeat launches for each workload. All calibrated launches reached readiness and had both usage samples. Verified the Darwin ABI and measured CPU counter conversion. The raw pilot is preserved separately, including failed direct Firefox launches and the pre-calibration units. Safari remote automation was unavailable. [Raw](../evidence/continuation-s5-macos-raw.json), [medians](../evidence/continuation-s5-macos-derived.json), [ABI](../evidence/continuation-s5-rusage-layout.txt), [alternative routes](../evidence/continuation-s5-browser-alternatives.txt), [footprint](../evidence/continuation-s5-macos-footprint.txt)

## Implications and limits

Host memory remained about 14–21 MiB. The ten-tab repeat workload reached a 460.8 MiB sum including associated WebKit helpers. Cold boot, first paint, sustained idle, and attribution of every system WebKit process remain open. This is a small, early post-launch measurement with a working desktop, recorded power source and fixed loopback pages. It does not generalize to everyday pages or release performance. [Comparative report, all cohorts and original engine floor comparison](../S5_COMPARATIVE_BASELINES.md)
