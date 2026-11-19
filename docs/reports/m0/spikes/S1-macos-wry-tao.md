---
spike: S1
platform: macos
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:33:47Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: S1 on macos (wry-tao)

## Method

Built pinned WRY 0.57.2 and TAO 0.37.2, then launched native chrome, content and overlay children in an AppKit window. Used a bundled chrome fixture and isolated loopback content. Inspected DOM viewports, navigation and visible bridge globals. Created, navigated and dropped groups of 1, 5, 10 and 20 hidden test views. Attempted native capture and focus using three logged approaches.

## Results

`docs/reports/m0/evidence/s1-wry-macos-native-id.txt` records source 1e7caea, binary SHA-256, macOS 15.8.3 arm64 and WebKit 20621.3.11.3. The chrome viewport was 900×72 before resize and 1000×72 after. Content was 900×548 then 1000×628. The 270×144 overlay document loaded. Content navigated to /next and reported neither named bridge global. All four groups reached expected load callbacks before their handles were dropped.

Both chrome and content reported `document.hasFocus() == false` after the targeted focus calls even when AppKit reported a visible key window. WindowServer capture failed using the returned NSWindow number. The pilot logs and retries are in `docs/BLOCKERS.md`. Visual z-order, native shortcuts and web-process crash recovery lack evidence.

## Findings and implications

The negative programmatic focus result remains unresolved. The Mac run did not complete the visual and input/crash criteria. The fixture's success marker applies to the dimensions, local navigation and lifecycle subset. No complete S1 pass or completed S1 failure is assigned.
