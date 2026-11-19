---
spike: S4
platform: macos
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T16:48:50Z
deadline_at: 2026-11-19T18:18:50Z
stopped_at: 2026-11-19T17:07:04Z
stopped_reason: completed
outcome: inconclusive
---

# S1 follow-up on macos

## Method and observed evidence

WRY + TAO on macOS 15.8.3 arm64, WKWebView 20621.3.11.11.3. The synthetic fixture verified two same-name cookie-gated downloads at separately selected scratch paths with matching bytes and quarantine xattrs, an attachment denied at the native callback, and same-origin cookie/localStorage separation from an incognito WebView. A separate native WKWebView control proved subresource suppression at the server and captured the inline PDF text. The WRY fixture completed PDF navigation. [Environment and tested binary](../evidence/continuation-s4-macos-identity.txt), [HTTP events](../evidence/continuation-s4-macos-content.jsonl), [native](../evidence/continuation-s4-macos-native.txt), [downloads](../evidence/continuation-s4-macos-downloads.json), [native filter and PDF control](../evidence/continuation-s4-engine-macos.txt), [PDF snapshot](../evidence/continuation-s4-pdf-macos.png)

## Open criteria

Progress and true mid-stream cancellation, large transfers, local files and printing, filter-list cost and compatibility, cross-private-view storage, cache/history, teardown and crash persistence remain untested. Native hooks and platform-specific risks are in the [feasibility matrix](../S4_EVERYDAY_MATRIX.md).
