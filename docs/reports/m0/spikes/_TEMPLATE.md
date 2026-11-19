---
spike: S1
platform: macos
substrate: wry-tao
budget_minutes: 90
started_at: 2026-01-01T00:00:00Z
deadline_at: 2026-01-01T01:30:00Z
# Optional, only with explicit human approval:
# extension_minutes: 30
# extension_approved_by: <human handle>
# extension_reason: <why>
---

# Spike report: <spike> on <platform> (<substrate>)

Copy this file to `docs/reports/m0/spikes/<spike>-<platform>-<substrate>.md` and replace every value.

## Git commit lifecycle

1. **Before the spike starts**, commit an initial header containing `spike`, `platform`, `substrate`, `budget_minutes`, `started_at`, and `deadline_at`. It is acceptable for the initial committed version to omit `stopped_at`, `stopped_reason`, and `outcome`; fill those only when the spike stops.
2. At the end of the spike, fill `stopped_at`, `stopped_reason`, and `outcome`, complete the report body, and commit the final report.
3. The validator requires the tracked, clean final report to have at least two distinct commits. The initial header commit must be before and within five minutes of `started_at`; the final report commit must be after and within five minutes of `stopped_at`.
4. Git commit timestamps are corroborating metadata only, not a cryptographically trusted clock. A reviewer must inspect commit history and artifacts; timestamps alone do not prove actual elapsed time.

- `spike`: one of `engine_floor`, `S1`, `S2a`, `S3`, `S4`, `S5`, `S7`, `S8`, `CI`.
- `platform`: `macos`, `windows`, `linux_x11`, or `linux_wayland`.
- `stopped_reason`: `completed`, `deadline`, or `blocked`.
- `outcome`: `pass`, `fail`, `inconclusive`, `blocked`, or `not_run`.
- A deadline stop must have `outcome: inconclusive`.
- A blocked stop must have outcome `blocked` or `inconclusive`; `outcome: blocked` requires `stopped_reason: blocked`.
- `outcome: fail` requires `stopped_reason: completed`. Only a completed S1 failure on macOS or Windows triggers the kill rule.
- `deadline_at` must equal `started_at` + `budget_minutes` (+ any approved extension).

## Method
## Results (cite artifact paths)
## Findings and implications
## What remains
