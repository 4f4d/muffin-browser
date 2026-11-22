# M4 macOS native interaction receipt

- **Date** 2026-11-22
- **Tester** Codex using Computer Use with owner approval
- **App** Muffin M4 Fixture (`org.muffin.m4.fixture`)
- **Source** `8cc3708c38b7216144d4dbe4897f432da954b625`, clean detached worktree
- **Debug binary SHA-256** `03d64af552f4740e115e165f96185080318db6d14be2efaac9d371473153d22d`
- **Environment** Apple Silicon arm64, macOS 15.8.3, WebKit `20621.3.11.11.3`

The local loopback fixture used a temporary test-control directory and a generated file. These observations cover the named actions on the verified debug binary. [Raw server receipt](macos-arm64-native-interactions-upload.json), [matching build verification](macos-arm64-print-verifier.json)

| Check | Outcome | Observed behavior |
|---|---|---|
| Native picker selection | Pass | On `/upload`, Choose File opened the macOS picker. Accessibility state identified `fixture-upload.txt` as selected and exposed `Open` (`OKButton`) and `Cancel` (`CancelButton`). Selecting Open returned to the upload page with `fixture-upload.txt` in the file control. The loopback server received exactly 35 bytes with SHA-256 `5f9c410a623100abafe744086873b9be0c473be2eb186de1a8357843380a6ad3`, matching the generated file. |
| Native picker cancellation | Pass | Reopening Choose File exposed the native Cancel button. Clicking it returned to the same page with the previous selection. The fixture was still running through the remaining checks and its final receipt recorded exactly one upload. |
| Native system print panel | Pass | Page tools → Print page opened an AppKit sheet with Copies, Pages, PDF, and Cancel. The print preview displayed the fixture page and `Page 1 of 1`. Clicking Cancel dismissed the sheet and restored the upload page. The Print button was disabled with no printer selected. No print job was submitted. |
| Native page context menu | Pass | Right-clicking `Download fixture alpha` on `/second` exposed WebKit menu identifiers for Open Link, Open Link in New Window, Download Linked File, Copy Link, and Share. Escape removed the menu. The same page then accepted `m4-menu-ok` in the fixture text field. |

The picker and print sheet were observed in actual-app screenshots in the test session. The picker image contains local sidebar entries and stays out of the public repository. The app-window screenshot omits the floating context menu, so the menu result relies on its native accessibility state and the subsequent input check. Individual menu actions and physical print output are outside this receipt. The earlier denied Computer Use attempt is retained separately. [Earlier attempt](macos-arm64-fullscreen-inline-pdf.json)
