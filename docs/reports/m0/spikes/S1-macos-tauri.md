---
spike: S1
platform: macos
substrate: tauri
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:33:47Z
stopped_reason: blocked
outcome: blocked
---

# Spike report: S1 on macos (tauri)

## Method

A delegated writer used the separate `m0a/s1-tauri-worker` worktree for macOS Tauri 2.12.7 with child WebViews enabled. The lockfile resolves tauri-runtime-wry 2.12.5. The writer stopped after resolver and compiler failures.

## Results

The first dependency selection needed tauri-build 2.7.6. The source progressed to compile errors in builder calls and resize argument types. The last build identified the LogicalRect versus runtime Rect mismatch. See `docs/reports/m0/evidence/s1-tauri-compiler-attempt.txt`, `s1-tauri-rect-icon-attempt.txt` and `s1-tauri-resize-type-attempt.txt` in the same directory. No macOS Tauri binary or runtime evidence was obtained.

## Findings and implications

The diagnostics concern the unfinished fixture. Tauri's macOS behavior remains unverified. The separate Windows/Linux fixture is evaluated only on those platforms.
