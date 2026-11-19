---
spike: S4
platform: windows
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T16:48:50Z
deadline_at: 2026-11-19T18:18:50Z
stopped_at: 2026-11-19T17:07:04Z
stopped_reason: completed
outcome: inconclusive
---

# S1 follow-up on windows

## Method and observed evidence

Standard Windows Server 2025 x64 runner with WebView2 156.0.4314.58. The committed WRY fixture observed two byte-exact cookie-gated downloads, reported Zone.Identifier presence on both outputs, denied the canary attachment and confirmed that an incognito view could not read the normal marker or send its cookie. WRY reported completed navigation to the PDF response. Actual PDF pixels and native resource blocking were not exercised. This is runner functional evidence. [Build identity](../evidence/continuation-s4-windows-identity.txt), [native](../evidence/continuation-s4-windows-native.txt), [HTTP](../evidence/continuation-s4-windows-content.jsonl), [outputs](../evidence/continuation-s4-windows-downloads.json), [workflow](../evidence/continuation-s4-windows-ci.json)

## Open criteria

Windows 11 native desktop testing, download protection enforcement, progress/cancellation during transfer, WebResourceRequested behavior, PDF display/print/file access, private teardown and crash behavior remain open. The pinned WRY default includes a feature-disable argument concerning SmartScreen which this fixture overrides. [Feasibility matrix](../S4_EVERYDAY_MATRIX.md), [pinned source](../evidence/continuation-s4-sources.txt)
