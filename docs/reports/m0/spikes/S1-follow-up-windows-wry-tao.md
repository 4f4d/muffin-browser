---
spike: S1-follow-up
platform: windows
substrate: wry-tao
budget_minutes: 45
started_at: 2026-11-19T15:27:33Z
deadline_at: 2026-11-19T16:12:33Z
stopped_at: 2026-11-19T15:35:57Z
stopped_reason: blocked
outcome: blocked
---

# S1 follow-up on windows

## Attempt and result

Probed the local VM inventory and available consoles. UTM and VMware Fusion were present, but no Windows 11 guest or other available interactive Windows 11 desktop was identified. The earlier multi-WebView navigation, geometry, visual capture and focus-query run used Windows Server 2025. [Guest inventory and bounded media probe](../evidence/s1-followup-guest-inventory.txt), [original Server run](S1-windows-wry-tao.md)

## Remaining evidence

Native WebView2 accelerator routing, shortcuts while content is focused, both focus directions with keyboard input, unexpected renderer termination and recovery, and multi-monitor and DPI behaviour are untested on Windows 11. Keep WRY + TAO under evaluation pending an interactive Windows 11 run.
