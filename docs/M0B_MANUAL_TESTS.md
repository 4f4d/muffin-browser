# M0b — Manual Validation Script (human-run)
Codex cannot run these: they need real accounts, hardware authenticators, paid subscriptions, or a desktop session on Windows and Linux. You run them and record results in `docs/reports/m0/results.json`. Anything you do not run stays `not_run`. Codex must not fill these in.
## Environments
| Environment | Use for |
|---|---|
| Mac (bare metal) | macOS column; passkeys, DRM, HiDPI |
| Windows 11 (VM is fine for function) | Windows column; use bare metal for PK, ME-02, HD-01 |
| Ubuntu Desktop VM, GNOME, **Wayland session** | `linux_wayland` column |
| Ubuntu Desktop VM, **X11 session** | `linux_x11` column |
For each result record the exact environment (`bare_metal`, `vm`, or `ci_runner`), OS version, engine/runtime version, host architecture, and binary architecture. Tests marked **VM valid: no** are bare-metal-only: the generator rejects both `vm` and `ci_runner` records for those tests, regardless of the outcome. Do not relabel a VM or CI run as bare metal. An unrun test should remain absent from `results` (or use the minimal `not_run` record shown below).
## Getting exact versions
The validator requires dotted numeric versions, so a product name or a label such as "24H2" is rejected.
- **Windows OS:** PowerShell `[System.Environment]::OSVersion.Version` (for example `10.0.26100.1742`). **WebView2 runtime:** `(Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}').pv` or the Edge/WebView2 About page.
- **macOS:** `sw_vers -productVersion` and `sw_vers -buildVersion`; WebKit version from the app's recorded `navigator.userAgent` (`AppleWebKit/<version>`).
- **Linux:** `lsb_release -rs` or `/etc/os-release`; WebKitGTK via `dpkg -s libwebkit2gtk-4.1-0 | grep Version` (or your distro's package query); session type via `echo $XDG_SESSION_TYPE`.
- **Binary hash:** `shasum -a 256 <binary>` (macOS/Linux) or `Get-FileHash -Algorithm SHA256 <binary>` (Windows); **app commit:** `git rev-parse HEAD` in the checkout you built from.

## Public-repo hygiene
The repository is public. Use throwaway accounts. Keep raw screenshots in `docs/reports/m0/evidence-private/` (gitignored). Commit only redacted copies (no emails, names, tokens, cookies, account IDs, or recovery details) under `docs/reports/m0/evidence/`, and cite their repository-relative paths in `evidence`. The validator rejects missing, empty, out-of-repository, symlink-escaped, or `evidence-private/` evidence paths.
## Recording a result
Append a result to the relevant test's `results` array. For every `pass`, `fail`, `partial`, or `inconclusive` result, all fields below are mandatory:

- `app_commit`: full 40-character lowercase Git commit SHA for the source used to build the application. The commit must exist in the current repository's Git object database; use `git cat-file -e <FULL_SHA>^{commit}` to check it. Ensure the tested source commit has been fetched into this checkout before recording the result.
- `binary_sha256`: full 64-character lowercase SHA-256 of the exact binary you tested. On macOS/Linux use `shasum -a 256 <binary>`; on Windows use `Get-FileHash -Algorithm SHA256 <binary>`.
- `host_architecture` and `binary_architecture`: for example `arm64`, `aarch64`, or `x86_64`; record both even when they match.
- `os_version` and `engine_version`: exact OS build and WebView/runtime version, not just a product family.
- `tester` and `date`: human handle and real `YYYY-MM-DD` date.
- `evidence`: path to a non-empty, redacted file inside this repository and outside `evidence-private/`.

Example:
```json
{
  "platform": "windows",
  "environment": "bare_metal",
  "result": "pass",
  "date": "REPLACE_WITH_TEST_DATE_YYYY_MM_DD",
  "tester": "REPLACE_WITH_YOUR_HANDLE",
  "app_commit": "REPLACE_WITH_FULL_40_CHARACTER_APP_COMMIT_SHA",
  "binary_sha256": "REPLACE_WITH_64_CHARACTER_BINARY_SHA256",
  "host_architecture": "x86_64",
  "binary_architecture": "x86_64",
  "os_version": "REPLACE_WITH_EXACT_OS_VERSION_AND_BUILD",
  "engine_version": "REPLACE_WITH_EXACT_WEBVIEW_OR_ENGINE_VERSION",
  "notes": "",
  "evidence": "docs/reports/m0/evidence/REPLACE_WITH_REDACTED_EVIDENCE_FILE"
}
```

A `not_run` record may contain only `platform`, `environment`, and `result`; no evidence is expected. Prefer leaving unrun result arrays empty. Replace every `REPLACE_WITH_*` example value before recording a result. Placeholder values, partial hashes, invalid dates, missing/empty evidence, and evidence paths escaping the repository or resolving into `evidence-private/` are rejected.

Then run `python3 tools/gen_status.py`, `python3 -m unittest discover -s tools/tests -v`, and `python3 tools/gen_status.py --check`; commit the redacted evidence and updated result record together.
## Validation rules

The status generator enforces this runbook. A result of `pass`, `fail`, `partial`, or `inconclusive` is not accepted without the full metadata and a real evidence file. Tests marked **VM valid: no** reject both VM and CI-runner environments, for every result label. `not_run` is the only outcome that does not require build identity or evidence.

## Tests
| ID | Test | VM valid | Platforms |
|---|---|---|---|
| SI-01 | Direct Google login: sign in at accounts.google.com, then open Gmail | yes | macos, windows, linux_x11, linux_wayland |
| SI-02 | 'Sign in with Google' on three third-party sites | yes | macos, windows, linux_x11, linux_wayland |
| SI-03 | Microsoft account login plus one SSO redirect | yes | macos, windows, linux_x11, linux_wayland |
| SI-04 | GitHub login including 2FA | yes | macos, windows, linux_x11, linux_wayland |
| SI-05 | Informational: UA-detection diagnostic (never shipped) | yes | macos, windows, linux_x11, linux_wayland |
| PK-01 | Passkey create and sign-in on a public WebAuthn demo | **no** | macos, windows, linux_x11, linux_wayland |
| PK-02 | Hardware security key (USB) sign-in | **no** | macos, windows, linux_x11, linux_wayland |
| ME-01 | Streaming video: HD playback, seek, fullscreen enter/exit | yes | macos, windows, linux_x11, linux_wayland |
| ME-02 | DRM playback on one real subscription service plus a public DRM test page | **no** | macos, windows, linux_x11, linux_wayland |
| ME-03 | Codec fixtures: H.264/AAC, VP9, AV1, Opus audio | yes | macos, windows, linux_x11, linux_wayland |
| ME-04 | Tab audio indicator and mute | yes | macos, windows, linux_x11, linux_wayland |
| DL-01 | Downloads: large file, authenticated download, and quarantine / Mark-of-the-Web | yes | macos, windows, linux_x11, linux_wayland |
| UP-01 | File upload through the native dialog | yes | macos, windows, linux_x11, linux_wayland |
| PD-01 | PDF: open, scroll, print, download | yes | macos, windows, linux_x11, linux_wayland |
| PO-01 | Popups and window.opener flows on live providers | yes | macos, windows, linux_x11, linux_wayland |
| LX-01 | Wayland session: layout, resize, focus, keyboard shortcuts with multiple child views | yes | linux_wayland |
| LX-02 | Fractional scaling (125% and 150%) rendering and hit-testing | **no** | linux_x11, linux_wayland |
| LX-03 | IME input (for example ibus or fcitx) in chrome and content | yes | linux_x11, linux_wayland |
| LX-04 | GPU path vs software rendering behaviour | **no** | linux_x11, linux_wayland |
| HD-01 | HiDPI and mixed-DPI monitors | **no** | macos, windows |

## Test details
### SI-01 — Direct Google login: sign in at accounts.google.com, then open Gmail
- **How:** Use a throwaway Google account. Sign in directly in Muffin, including any 2FA prompt, then open Gmail and reload.
- **Criteria:** Pass: login completes, Gmail loads, session survives restart. Fail/partial: record the exact error page text (for example a blocked-browser message) and a redacted screenshot.
- **VM valid:** yes

### SI-02 — 'Sign in with Google' on three third-party sites
- **How:** Pick three unrelated sites that offer Google sign-in; record which ones in the notes. Complete the popup/redirect flow.
- **Criteria:** Pass: all three complete and return to the site signed in. Partial: record which failed and how.
- **VM valid:** yes

### SI-03 — Microsoft account login plus one SSO redirect
- **How:** Sign in at login.live.com or a Microsoft 365 page, then follow one SSO redirect to a third-party service.
- **Criteria:** Pass: login and redirect succeed, session persists.
- **VM valid:** yes

### SI-04 — GitHub login including 2FA
- **How:** Sign in to github.com with a throwaway account using a TOTP code.
- **Criteria:** Pass: login succeeds, session persists after restart.
- **VM valid:** yes

### SI-05 — Informational: UA-detection diagnostic (never shipped)
- **How:** Only if SI-01 or SI-02 failed. Human-run, local-only experiment to learn whether blocking depends on the user-agent string. Record the observation. Do not commit UA-changing code; this informs DEC-UA, which you decide.
- **Criteria:** Informational: record observed difference or none.
- **VM valid:** yes

### PK-01 — Passkey create and sign-in on a public WebAuthn demo
- **How:** Use a public WebAuthn demo site. Create a passkey with the platform authenticator, then sign in with it.
- **Criteria:** Pass: both steps work. Fail: record the error. Must be bare metal: VMs lack the platform authenticator.
- **VM valid:** no (bare metal only)

### PK-02 — Hardware security key (USB) sign-in
- **How:** Register and use a USB security key on the same demo site.
- **Criteria:** Pass: registration and sign-in succeed. Bare metal only.
- **VM valid:** no (bare metal only)

### ME-01 — Streaming video: HD playback, seek, fullscreen enter/exit
- **How:** Play an HD video on a mainstream video site, seek, enter and leave fullscreen via the page and Esc.
- **Criteria:** Pass: smooth playback, fullscreen works and exits cleanly, no layout corruption.
- **VM valid:** yes

### ME-02 — DRM playback on one real subscription service plus a public DRM test page
- **How:** Try a real DRM service you subscribe to, and a public DRM demo page. Record service names and the visible result.
- **Criteria:** Pass: protected content plays. Bare metal only: VM results are not valid for DRM.
- **VM valid:** no (bare metal only)

### ME-03 — Codec fixtures: H.264/AAC, VP9, AV1, Opus audio
- **How:** Open the codec fixtures produced by Codex (docs/reports/m0 or the spikes folder) and play each.
- **Criteria:** Pass per codec; record which codecs fail. On Linux also record installed GStreamer packages.
- **VM valid:** yes

### ME-04 — Tab audio indicator and mute
- **How:** Play audio in one tab, switch away, check whether Muffin spike can detect audio and mute it.
- **Criteria:** Pass: detection and mute both work. Partial: record which is missing.
- **VM valid:** yes

### DL-01 — Downloads: large file, authenticated download, and quarantine / Mark-of-the-Web
- **How:** Download a large file and a file behind a login. Then inspect the file: macOS quarantine attribute, Windows Zone.Identifier stream; Linux: record that none applies.
- **Criteria:** Pass: downloads complete, attribute present where the OS defines one. Record exact inspection commands used.
- **VM valid:** yes

### UP-01 — File upload through the native dialog
- **How:** Upload a file to a fixture page and to a real site.
- **Criteria:** Pass: dialog opens, file arrives, cancel works.
- **VM valid:** yes

### PD-01 — PDF: open, scroll, print, download
- **How:** Open a PDF fixture inline. Record whether the engine provides a viewer.
- **Criteria:** Pass: usable inline viewer with print and download. Fail: record that a bundled viewer would be needed.
- **VM valid:** yes

### PO-01 — Popups and window.opener flows on live providers
- **How:** Repeat a Google/GitHub popup-based sign-in and a payment-style popup (test mode) and watch for opener breakage.
- **Criteria:** Pass: popup closes and the parent page receives the result.
- **VM valid:** yes

### LX-01 — Wayland session: layout, resize, focus, keyboard shortcuts with multiple child views
- **How:** In a GNOME Wayland session, run the S1 spike: resize, tab to content and back, shortcuts while content has focus.
- **Criteria:** Pass: bounds follow resize, focus moves correctly, shortcuts work.
- **VM valid:** yes

### LX-02 — Fractional scaling (125% and 150%) rendering and hit-testing
- **How:** Set fractional scaling, run the spike, click targets near edges.
- **Criteria:** Pass: crisp rendering, clicks land where expected. Bare metal only.
- **VM valid:** no (bare metal only)

### LX-03 — IME input (for example ibus or fcitx) in chrome and content
- **How:** Type CJK text in the address bar and in a content page.
- **Criteria:** Pass: composition and commit work in both.
- **VM valid:** yes

### LX-04 — GPU path vs software rendering behaviour
- **How:** Record the active renderer and compare smoothness with the compositing workarounds off/on.
- **Criteria:** Informational. Bare metal only: VM graphics are not representative.
- **VM valid:** no (bare metal only)

### HD-01 — HiDPI and mixed-DPI monitors
- **How:** Drag the window between displays with different scale factors.
- **Criteria:** Pass: no blur, tearing, or offset content views. Bare metal only.
- **VM valid:** no (bare metal only)

## Sign-in gate (product decision)
The gate is SI-01 to SI-04 passing on the platforms you intend to ship. If any fail, record SI-05 information, then decide DEC-UA and whether the platform is go, beta, or no-go. Codex presents the evidence; you decide.
