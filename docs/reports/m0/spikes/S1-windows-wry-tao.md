---
spike: S1
platform: windows
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:44:06Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: S1 on windows (wry-tao)

## Method

Ran the pinned WRY/TAO shell under WebView2 on the Windows Server 2025 x64 runner. Exercised local navigation, viewport bounds/resize, groups of 1/5/10/20 views, programmatic focus and native overlap capture.

## Results

`docs/reports/m0/evidence/s1-windows-wry-tao.txt` records source 8c8d36f, WebView2 156.0.4314.58 and the verified binary SHA-256. Chrome/content queried viewports matched 900×72/900×548 and 1000×72/1000×628 after resize. The loopback document reported neither named bridge global. Both target documents reported focus and all four load/drop groups completed. The [runner archive](https://github.com/4f4d/muffin-browser/actions/runs/44544220054/artifacts/14539896675) includes the raw screenshot. The crop `docs/reports/m0/evidence/s1-windows-wry-tao.png` shows the red overlay over both regions.

## Findings and implications

Runner observations support bounds, navigation, focus and overlap for this fixture. Native accelerator routing and web-process crash recovery lack evidence. Windows 11 desktop, mixed DPI and real provider behavior remain unverified.
