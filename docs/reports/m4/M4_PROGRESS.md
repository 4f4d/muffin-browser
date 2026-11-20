# M4 — macOS page tools and downloads

The download-consent slice passed independent review and local and hosted verification on `2038bff536c1342a724329b6281dacf7eb52ef5b`. The full M4 engineering gate remains open for native file selection and right-click menus. The target is still the macOS Apple Silicon development build. [Verifier receipt](../platform/m4/macos-arm64-consent-verifier.json), [hosted verifier](https://github.com/4f4d/muffin-browser/actions/runs/44769839442), [status guard](https://github.com/4f4d/muffin-browser/actions/runs/44769839334)

## Verified behavior

- An automatic page attachment appeared for approval. Cancelling it left the download directory empty. An approved page link, an address-bar attachment, and a cross-origin redirect created three files with distinct names and matching SHA-256 digests. The browser displayed the redirect's final response host. The fixture activates the link inside its WebKit page. Chrome component tests cover the Save and Cancel buttons and action contract. [Download decisions](../../adr/0004-macos-download-consent.md), [native receipt](../platform/m4/macos-arm64-consent-smoke.json), [Downloads capture](../platform/m4/macos-arm64-consent-downloads.png), [hosted fixture](../ci/m4/macos-arm64-44769839442-native.json)
- WebKit found the fixture phrase, rejected an absent phrase, read zoom back at 110% and 100%, and exported a complete PDF. A separate fixture navigation received an inline PDF and rendered its text in the selected content view. AppKit's window mask recorded fullscreen entry and exit. [Native receipt](../platform/m4/macos-arm64-consent-smoke.json), [AppKit receipt](../platform/m4/macos-arm64-fullscreen-inline-pdf.json), [inline PDF capture](../platform/m4/macos-arm64-inline-pdf.png)
- The local verifier passed 83 tool tests, one Python loopback fixture test, 17 UI tests and 32 Rust tests. It built debug and release binaries and confirmed the release binary rejects debug control. Cargo audit reported two allowed GTK-side warnings. The hosted job supplied the platform build record. [Verifier receipt](../platform/m4/macos-arm64-consent-verifier.json), [CI build receipt](../ci/macos-arm64-build-00000000044769839442-0001.json)

## Print verification rationale

The WebKit PDF export path and AppKit print path are separate. The print fixture uses the same window-based WebKit operation as the browser's Print control. It selects AppKit's Save Job disposition with a destination inside the disposable control directory, rechecks that disposition on the operation, and disables the print and progress panels. The test waits for a PDF trailer before examining the file. This covers actual local print output and leaves native print-panel interaction open. The two earlier print attempts produced an incomplete stream or timed out. [Print adapter](../../../crates/muffin/src/pdf.rs), [fixture check](../../../tests/native/run_smoke.py), [attempts](../../BLOCKERS.md#m4-integration-attempts), [Apple print operation](https://developer.apple.com/documentation/appkit/nsprintoperation)

## Open before the M4 exit review

- Computer Use denied access to the isolated fixture app. Native file selection requires choosing the generated test file and matching the fixture's received digest, then cancelling a second selection and observing that no additional upload arrives. Native print-panel display and dismissal and the engine's right-click page menu also need an observed interaction.
- macOS media permissions currently deny unsupported grants. Live oversized transfers can temporarily exceed the publication limit before the staging check runs. The store's final size rejection has automated coverage. [Download decisions](../../adr/0004-macos-download-consent.md), [store tests](../../../crates/muffin/src/downloads.rs)

The generated status remains at M4. [Project status](../../STATUS.json)
