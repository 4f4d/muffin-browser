# M6 — macOS visual and accessibility checkpoint

The reviewed candidate is macOS Apple Silicon development source `60772c5ca3c85fd5328f412301b7a9fa49369059`. A preview of this exact binary is open at Start with a disposable profile. The app’s accessibility tree shows the Open tabs group, the selected page’s pressed state and separate Close and Unload controls. [Binary and verifier receipt](../platform/m6/macos-arm64-m6-verifier.json)

## Native evidence

- [Light toolbar](../platform/m6/macos-arm64-chrome-toolbar-light.png), [dark toolbar](../platform/m6/macos-arm64-chrome-toolbar-dark.png), [640-point tabs](../platform/m6/macos-arm64-chrome-compact.png), [640-point More actions](../platform/m6/macos-arm64-chrome-compact-actions.png)
- [Private Start](../platform/m6/macos-arm64-private-start.png), [secure connection error](../platform/m6/macos-arm64-secure-connection-error.png)
- The combined native run exercised all eight compact toolbar actions fitting in the available row, an adjacent New Tab button, selected-tab visibility under overflow, two palettes and application-local AppKit shortcuts. The privacy fixture verified that the new private window focuses the address field and that the TLS error moves focus to its heading before Back to Start returns focus to the address field. Settings tests confirm that the same button retains focus when the clear-data confirmation opens. [Native capture receipt](../platform/m6/macos-arm64-m6-capture.json), [component tests](../../../ui/tests/app.test.ts)
- The complete local verifier passed 83 tooling tests, one native fixture unit test, 27 UI tests and 45 Rust tests. It also ran formatting, Clippy, audits, debug and release builds, the release debug-control exclusion, and the native smoke. The independent reviewer found no material issue preventing this owner checkpoint. [Full verifier receipt](../platform/m6/macos-arm64-m6-verifier.json), [native smoke](../platform/m6/macos-arm64-m6-native-smoke.json)

## Accessibility audit

Axe 4.13.0 initially detected an invalid tablist child relationship for the independent Close and Unload buttons. The strip now exposes a labeled group with pressed selection buttons. The follow-up Chromium audit reported zero violations across 20 checks covering five surfaces, two palettes and two widths. Axe flagged some contrast and ARIA measurements for manual inspection. Pa11y with HTML_CodeSniffer flagged the lack of a visible form submit button in the omnibox. Actual Chromium keyboard input confirmed Enter produces the intended navigation action. Check how VoiceOver announces this interaction during the owner review. [First audit](../platform/m6/first-chrome-accessibility-audit.json), [follow-up audit](../platform/m6/chrome-accessibility-audit.json)

## What to review

Muffin uses a warm accent for the New Tab control and meaningful states. The chrome groups navigation, address entry and downloads in the first row. The second row keeps tabs, the adjacent New Tab button, search and page tools within reach. At narrow widths the secondary actions use compact labels. The active tab keeps its tonal fill and shows a focus ring during keyboard use. The effect is implemented in the app’s chrome, with a solid fallback for increased contrast and reduced transparency. [Chrome style](../../../ui/src/style.css), [local Lucide attribution](../../../ui/src/icons/NOTICE.md)

- Inspect the current light and dark chrome at regular width. At 640 points, switch to an overflowed tab and open More. Check that the active title, Close, New Tab and all eight toolbar actions are usable and that the page or Start surface remains the focal area.
- From content, open a new tab with Command–T and a private window with Shift–Command–N. In tab search, use Arrow Down to reach a match and Enter to select it. With VoiceOver, check the controls in the Open tabs group, how the selected page is announced, and the distinctive labels for Close, Unload, More, Page tools and Downloads.
- Review a trusted connection-error surface. Check that the heading receives focus and that Tab reaches Back to Start. Return to Start and confirm that focus reaches the address field. In Settings, check that the website-data confirmation keeps focus on Cancel.
- Review the compact chrome under larger text or 200% zoom, increased contrast, Reduce Transparency, and Reduce Motion. Record the actual macOS and VoiceOver versions and any contrast, focus, hit-area, clipping or announcement issues.

## Acceptance boundary

The human visual and accessibility checkpoint is open. Automated browser and native checks help locate regressions. The owner records the accessibility and visual acceptance here or supplies findings for another iteration. M0b, platform and release qualification use their existing evidence and decision process. [Milestone gates](../../PLAN.md#gates)
