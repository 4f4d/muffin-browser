---
spike: S8
platform: macos
substrate: wry-tao-x64
budget_minutes: 45
started_at: 2026-11-19T17:33:19Z
deadline_at: 2026-11-19T18:18:19Z
stopped_at: 2026-11-19T17:38:25Z
stopped_reason: completed
outcome: inconclusive
---

# S1 follow-up on macos

## Method and result

Cross-built the locked optimized x86-64 macOS fixture and verified its architecture, archive integrity and presence in the universal file. The 1,719,280-byte EXE payload produced a 695,150-byte ZIP and 1,684 KiB allocated bundle. Its unpacked binary passed the synthetic S4 workflow under already installed Rosetta on the Apple Silicon host. Native Intel hardware coverage and actual minimum macOS support remain unverified. [Sizing](../evidence/continuation-s8-macos-sizing.txt), [translated smoke](../evidence/continuation-s8-macos-x64-replay.txt), [package matrix](../S8_PACKAGE_RUNTIME.md)
