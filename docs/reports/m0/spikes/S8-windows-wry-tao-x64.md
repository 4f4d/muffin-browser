---
spike: S8
platform: windows
substrate: wry-tao-x64
budget_minutes: 45
started_at: 2026-11-19T17:33:19Z
deadline_at: 2026-11-19T18:18:19Z
stopped_at: 2026-11-19T17:38:25Z
stopped_reason: completed
outcome: inconclusive
---

# S1 follow-up on windows

## Method and result

Verified the successful Windows Server CI x64 test executable hash and inspected the PE architecture and imports. The debug fixture executable is 2,500,096 bytes and the verified EXE-only ZIP is 850,683 bytes. The runner used an external WebView2 Runtime 156.0.4314.58. No installer or installed-footprint measurement ran. Windows 11 smoke, a clean runtime deployment, missing-runtime handling, offline scenarios and non-admin installation remain open. [PE and ZIP evidence](../evidence/continuation-s8-windows-x64.txt), [runner](../evidence/continuation-s4-windows-ci.json), [package matrix](../S8_PACKAGE_RUNTIME.md)
