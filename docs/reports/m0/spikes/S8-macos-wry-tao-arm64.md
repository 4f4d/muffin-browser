---
spike: S8
platform: macos
substrate: wry-tao-arm64
budget_minutes: 45
started_at: 2026-11-19T17:33:19Z
deadline_at: 2026-11-19T18:18:19Z
stopped_at: 2026-11-19T17:38:25Z
stopped_reason: completed
outcome: pass
---

# S1 follow-up on macos

## Method and result

Built the locked optimized S4 fixture for aarch64-apple-darwin from the named commit under Xcode SDK 26.2. Measured a 1,709,776-byte executable, 672,701-byte ZIP and 1,676 KiB extracted allocated bundle. The unpacked fixture matched the built binary and passed the bounded real macOS arm64 S4 synthetic workflow. lipo also produced a verified 1,366,894-byte ZIP with both target slices. macOS supplies WebKit and system frameworks. Signing, distribution and install/uninstall were not exercised. [Sizing and dependencies](../evidence/continuation-s8-macos-sizing.txt), [unpacked native smoke](../evidence/continuation-s8-macos-arm64-replay.txt), [package matrix](../S8_PACKAGE_RUNTIME.md)
