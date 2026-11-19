---
spike: S2a
platform: macos
substrate: wry-tao
budget_minutes: 60
started_at: 2026-11-19T15:54:13Z
deadline_at: 2026-11-19T16:54:13Z
stopped_at: 2026-11-19T16:41:20Z
stopped_reason: blocked
outcome: inconclusive
---

# S1 follow-up on macos

## Scope and method

One 60-minute cross-platform investigation. Pinned WRY 0.57.2 and TAO 0.37.2 use two ephemeral loopback origins, no privileged IPC, ten initially hidden native TAO window slots, related engine configuration, and guarded navigation in opener and popup. Automatic clicks use DOM events with no trusted gesture. The separate Xvfb click uses one native XTEST pointer event, with DOM activation recorded. No outside provider, protocol launcher, or real account participated. [Source review](../evidence/continuation-s2a-sources.txt), [environment](../evidence/continuation-environment.txt), [fixture](../../../../spikes/s1/followup/s2a_fixture.py)

## Popup capability matrix

| Environment | Observed |
|---|---|
| macOS 15.8.3 arm64 / WKWebView 20621.3.11.11.3 | Seven separately allocated native windows from programmatic cases. Same-origin script popup could read the opener, cross-origin script popup retained a handle with no document read, and implicit and explicit noopener anchors had no opener. Same/cross-origin redirects reached final pages and the synthetic auth chain returned its code to the original origin. Origin-constrained page messages reached the opener. Popup download completed to a scratch file with verified 27-byte payload. The opener observed the JS window as closed, but all seven host windows were still retained when bounded exit began. Native focus events were logged. There is no visual stacking or native destroy evidence. The earlier real desktop run reached its 420-second bound after the UI control channel expired before input. [Identity](../evidence/continuation-s2a-macos-identity.txt), [native](../evidence/continuation-s2a-macos-native.txt), [content](../evidence/continuation-s2a-macos-content.jsonl), [download](../evidence/continuation-s2a-macos-downloads.json), [interactive attempt](../evidence/continuation-s2a-macos-interactive.txt) |
| Windows Server 2025 x64 / WebView2 156.0.4314.58 | CI exercised the seven automatic cases and observed the same opener and redirect distinctions on the earlier fixture revision. The host denied the synthetic protocol popup. The runner did not exercise the later message, download and close cases, Windows 11 desktop focus or visual stacking. [Build identity](../evidence/continuation-s2a-windows-identity.txt), [native](../evidence/continuation-s2a-windows-native.txt), [content](../evidence/continuation-s2a-windows-content.jsonl) |
| X11 / WebKitGTK 2.52.6 | Automatic clicks reached content and no request reached the new-window callback. The XTEST alternative logged a trusted click, active user activation, a returned script handle, native popup focus and readable same-origin opener. This is one functional Xvfb case. GTK redirect, close, download and desktop window stacking remain open. [Automatic log](../evidence/continuation-s2a-linux-x11.txt), [XTEST log](../evidence/continuation-s2a-linux-x11-xtest.txt) |
| Wayland / WebKitGTK 2.52.6 | Headless Weston loaded the fixture and processed script clicks. No popup request reached the host callback. A legitimate interactive session remains unavailable for activation, focus, visual stacking and lifecycle. [Log](../evidence/continuation-s2a-linux-wayland.txt) |

CI retained independent engine-floor and shell outcomes. On this commit Windows passed engine-floor, S1 runner and the S2a fixture step. Ubuntu passed both engine floors and artifact upload. Its S1 and S2a steps failed, and the bounded cached Linux container results above use the corrected code and separate environments. [Run metadata](../evidence/continuation-s2a-ci.json), [initial GTK build failure](../evidence/continuation-s2a-linux-build-attempt.txt)

## Implications

Use engine-related popup creation when opener semantics are required, and apply policy again when the target navigates or redirects. WRY's common callback does not provide user-activation or origin-frame metadata. Windows exposes gesture state through the underlying API. WebKitGTK's trusted and untrusted input observations differ in the tested runs. A product-managed host must tie the renderer close notification to native window destruction and test focus and stacking on a real desktop. External protocols and both synthetic origins were explicitly controlled. All popup-ready events sampled above carried no fixture IPC global. Live providers, real cookies, broader frame policies and a functional cross-platform popup release remain open. [Pinned source evidence](../evidence/continuation-s2a-sources.txt)
