---
spike: S1-follow-up
platform: macos
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T15:16:55Z
deadline_at: 2026-11-19T16:46:55Z
stopped_at: 2026-11-19T15:32:54Z
stopped_reason: blocked
outcome: inconclusive
---

# S1 follow-up on macos

## Method

Ran the pinned WRY 0.57.2 and TAO 0.37.2 fixture on the interactive macOS 15.8.3 arm64 desktop in a visible AppKit window. Recorded the focused native responder, its view ancestry, the key-window and active-app flags, and separate DOM focus queries. Sent fixture-only keyboard events through the desktop automation surface. Tested real app-local NSEvent Cmd+1 and Cmd+2 handling. The positive and negative security cases occupied under 15 minutes of the shared 90-minute window. [Original macOS S1](S1-macos-wry-tao.md), [run identity](../evidence/s1-followup-macos-environment.txt)

## Results

- Clicking content and sending `a` and Return produced a real AppKit key event, the text `a` and a page Return report. Cmd+1 moved the native first responder to the chrome WKWebView. The later `b` appeared in chrome.
- Cmd+2 moved the native first responder to content. The later `c` appeared in content, resulting in `ac`. The native makeFirstResponder calls reported true and the responder ancestry matched both destinations while the window was key. Both already-targeted documents returned false from the pre-explicit-focus query. The fixture then requested DOM input focus. Later observations showed true focus in the selected document. This records the sequence without determining whether DOM focus would eventually have settled on its own.
- The interactive window capture shows the red overlay across the blue chrome and white content. RGB decoding of the original JPEG locates its red envelope at (420,144)–(939,395), crossing the y=200 boundary. The alternate command-line WindowServer capture failed. [Window capture](../evidence/s1-followup-macos-overlay.jpg), [native and DOM log](../evidence/s1-followup-macos-native.txt), [pixel and environment evidence](../evidence/s1-followup-macos-environment.txt)
- Two positive trusted chrome canary calls bracketed untrusted command and URI attempts without an additional native effect. The HTTP control succeeded, while the bundled-file and private-chrome fetches rejected. [Security assessment](../S1_FOLLOW_UP_SECURITY.md), [bundled-file identity](../evidence/s1-followup-macos-bundle-environment.txt)

## Limits

The fixture registered the public WKWebView process-termination callback and a checked fixture reload path. No unexpected WebContent termination occurred during either run. A public, safely scoped way to assign an OS WebContent process exclusively to this fixture was unavailable, so an external termination was not induced. The Apple callback reports termination for any reason. Unexpected termination, state retention and recovery remain untested. [Apple WKNavigationDelegate](https://developer.apple.com/documentation/webkit/wknavigationdelegate/webviewwebcontentprocessdidterminate(_:))

## Recommendation

Continue WRY + TAO for macOS. Interactive input, bidirectional native routing and captured composition now have evidence alongside original geometry and lifecycle results. Full macOS S1 still requires unexpected renderer recovery evidence.
