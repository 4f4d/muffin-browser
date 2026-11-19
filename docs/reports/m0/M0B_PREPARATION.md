# M0b validation build and testing guide

The M0b engineering build lives in [`spikes/m0b-validation`](../../../spikes/m0b-validation/). It uses the provisionally authorized WRY 0.57.2 and TAO 0.37.2 with system WebViews. It has an address and search bar, native windows, lazy content tabs, separate private tabs, related popup windows and a Test tools panel. DuckDuckGo is the search endpoint for this temporary build. External schemes and top-level file URLs are denied. The engine user agent and certificate behavior are left at their defaults. [Validation source](../../../spikes/m0b-validation/src/main.rs), [security assessment](M0B_VALIDATION_SECURITY.md), [owner decisions](../../DECISIONS_PENDING.md)

## Select a build

Use the platform-specific evidence and artifacts in the [build manifest](evidence/m0b-validation/builds.json). Match the source SHA and binary SHA to the package metadata and the launch receipt before any manual test. A ZIP or tarball hash verifies the download. The executable hash identifies the actual application for the result record. Hosted builds are engineering artifacts with limited retention. [Artifact workflow](../../../.github/workflows/m0b-validation.yml), [manual runbook](../../M0B_MANUAL_TESTS.md)

| Build | Install and launch | Environment for M0b |
|---|---|---|
| macOS arm64 | Unzip and run `Muffin.app` from Finder or invoke its executable in Terminal. The package uses system AppKit and WKWebView. Engineering packages carry no distribution signature or notarization. The tested OS is in the manifest. | Apple Silicon Mac. Bare metal for PK-01, PK-02, ME-02 and HD-01. |
| Windows x64 | Unzip and run `muffin-validation.exe` with the x64 WebView2 Evergreen Runtime installed. The adapter requires InPrivate profile reporting, the external-scheme cancel event and native download interfaces. If initialization fails, check that the installed Evergreen runtime is current. | Windows 11 desktop for functional cases. A suitable VM can cover the cases marked VM-valid. Bare metal for the hardware cases. Server 2025 CI is synthetic evidence. |
| Ubuntu 24.04 x64 | Extract the tarball and run `muffin-validation`. Install GTK 3 and WebKitGTK 4.1 from the distribution. Record the installed GStreamer stack before codec and playback tests. | Use an actual GNOME session of the required type. Confirm `XDG_SESSION_TYPE` before testing. Headless Xvfb and Weston are engineering tests. Bare metal is required for LX-02 and LX-04. |

```sh
# macOS, inside the extracted package directory
./Muffin.app/Contents/MacOS/Muffin --identity
./Muffin.app/Contents/MacOS/Muffin --diagnostics

# Ubuntu 24.04, inside the extracted package directory
sudo apt install libgtk-3-0t64 libwebkit2gtk-4.1-0
printf '%s\n' "$XDG_SESSION_TYPE"
GDK_BACKEND=wayland ./muffin-validation --diagnostics  # in a GNOME Wayland session
GDK_BACKEND=x11 ./muffin-validation --diagnostics      # in a GNOME X11 session
```

```powershell
.\muffin-validation.exe --identity
.\muffin-validation.exe --diagnostics
Get-FileHash -Algorithm SHA256 .\muffin-validation.exe
```

To reproduce a package locally, check out the manifest's full source commit in a clean worktree, install the [workflow's platform dependencies](../../../.github/workflows/m0b-validation.yml), and set `MUFFIN_SOURCE_COMMIT` to that checkout's exact HEAD. Run `cargo build --release --locked --manifest-path spikes/m0b-validation/Cargo.toml`, then call [`scripts/package.py`](../../../spikes/m0b-validation/scripts/package.py) with `--binary`, `--target`, `--source-commit` and `--out-dir`. The app reports its own SHA on every launch. Capture a fresh receipt on the machine that runs the test because OS and runtime values can differ from the packaging host.

## Test workflow

1. Start with the loopback fixtures and a disposable app data directory. Create the optional media directory with the existing FFmpeg recipe, then launch the server. It assigns two local origins and exits after its time limit.
2. Enter the first origin in the browser. The page links to a second-origin popup, the simulated sign-in cookie gate and attachment, an 8 MiB throttled attachment, an upload form, PDF, media and storage. For the local upload, use a small synthetic file named `synthetic.txt`. The server records a receipt and byte count.
3. Use **Test tools** for print, mute where supported, download folder, cancel where supported, and inactive view release. Downloads are written to distinct `muffin-*` subdirectories of the selected folder. The default is the validation data directory's `downloads/` folder. An intentionally downloaded file can remain after the private tab closes. Use the greenfield profile for synthetic runs and a separate data directory for manual provider cases on Windows and Linux. macOS normal WebKit storage is scoped to the engineering app identity.
4. Use Cmd on Mac and Ctrl on Windows/Linux with L for the address input, T for a new tab, Shift+N for a new private tab, W to close, Tab to switch, R to reload, brackets for history navigation, period to stop, Shift+B to focus the page and P to print. Exercise native shortcuts with the content page focused. For resize and focus, use the regular native window controls and the address and Page focus controls. Take an app-window-only screenshot if the case is visual.
5. Follow the existing [manual runbook](../../M0B_MANUAL_TESTS.md) for the exact pass criteria, test environments and result fields. For every attempt record a fresh `--identity` receipt, environment type, hardware or account prerequisites, expected and observed behavior and the redacted artifact. Keep raw screenshots, account details and evidence of browser state under the ignored `evidence-private/` directory. Enter the full source SHA and executable hash in the human result record with the OS and WebView version on the test machine. Protect account identifiers, credential prompts, cookies, session URLs and recovery details in any shared artifact.

```sh
sh spikes/m0b-prep/make_media.sh <new-media-directory>
python3 spikes/m0b-validation/fixtures/server.py \
  --ready-file <scratch>/ready.json --log-file <scratch>/routes.jsonl \
  --media-dir <new-media-directory> --max-seconds 1800
```

The standalone native regression runner takes the executable and an output directory. Run it in a desktop environment or use `--x11-synthetic-input` in a disposable Xvfb session. The test-only `--core-only` profile records unavailable popup coverage as inconclusive. The runner keeps profile data temporary and outputs source and binary identities, case-level results, fixed route markers and verification of downloaded payload hashes. [Runner](../../../spikes/m0b-validation/fixtures/run_native.py)

## Manual case mapping

In this table **All** means the macOS arm64, Windows x64 and Ubuntu x64 validation builds, with Linux tested in separate X11 and Wayland sessions. Apply the environments and exact outcome rules in the original runbook. App receipt and redacted evidence are required for each executed cell.

| ID | Build and setup | Use and evidence | Limitation |
|---|---|---|---|
| SI-01 | All, VM-valid. Throwaway Google account and its second factor. | Address bar to Google Accounts, then Gmail. Reload and relaunch in the same normal profile. Capture the redacted completion or error. | Google policy remains a human gate. |
| SI-02 | All, VM-valid. Three unrelated sites and a throwaway Google account. | Follow each site's sign-in button, capture the popup or redirect and the resulting signed-in state. | Local opener tests cover two synthetic origins. |
| SI-03 | All, VM-valid. Microsoft account and SSO target. | Follow the sign-in and third-party redirect, relaunch and record session persistence. | Tenant and provider policies may vary. |
| SI-04 | All, VM-valid. Throwaway GitHub account and TOTP. | Complete sign-in and 2FA, relaunch and record persistence without the account name or prompt contents. | Synthetic cookie tests cannot establish live acceptance. |
| SI-05 | All, local human diagnostic after applicable sign-in failure. | Record the observed original UA and the owner-directed diagnostic finding. | No UA override is provided in the validation app. DEC-UA remains open. |
| PK-01 | All, bare metal, platform authenticator and WebAuthn test account as needed. | Open the runbook's WebAuthn demo and record passkey registration and subsequent sign-in. | Engine, authenticator and provider integration require the actual host. |
| PK-02 | All, bare metal, connected USB security key and demo. | Register and sign in, capture only a redacted result. | Hardware interaction remains human-operated. |
| ME-01 | All, VM-valid, HD stream and eligible network. | Play, seek, enter fullscreen and exit with Esc. Capture viewport and retained controls before and after. | Codec and fullscreen behavior vary by engine and session. |
| ME-02 | All, bare metal, existing subscription and public DRM fixture. | Record service and redacted playback or exact error for each. | No bundled DRM module or protected-content test result. |
| ME-03 | All, VM-valid, generated media assets. | Play all four assets from the loopback Media page. Record results by codec and Linux dependency versions. | FFprobe establishes encoding. Each engine must still play it. |
| ME-04 | All, VM-valid, at least two tabs and audio stimulus. | Play audio, switch tabs, check the playing marker and mute action. Record each observation. | WebView2 and WebKitGTK have native audio APIs. The macOS adapter leaves tab audio/mute unavailable. |
| DL-01 | All, VM-valid, synthetic slow file plus suitable signed-in download site. | Use Download folder, record progress/cancel, repeat the same basename and verify file hashes. Inspect `xattr -p com.apple.quarantine` or PowerShell `Get-Item -Stream Zone.Identifier` where applicable. | macOS has completion and file-size progress when the engine exposes the destination early. The common WKDownload adapter has no in-progress cancel. Loopback metadata is a separate observation from remote policy. |
| UP-01 | All, VM-valid, eligible site and disposable `synthetic.txt`. | Open the main-tab file chooser, cancel, retry, submit and save the redacted receipt. | Automated probes use synthetic file input data. The native picker remains a human check. |
| PD-01 | All, VM-valid, inline fixture PDF and available print dialog. | Open, scroll, print or cancel and download. Screenshot the actual viewer/dialog and record the file outcome. | A PDF navigation or attachment result alone cannot establish a usable viewer or print path. |
| PO-01 | All, VM-valid, authorized throwaway accounts and test-mode payment popup. | Start with a trusted click on the loopback popup, then live provider cases. Record redirect, opener return, resulting state and dismissal. | Nested popups are denied. macOS related popup chrome only implements close delegation. Live workflows remain a human gate. |
| LX-01 | Ubuntu x64, GNOME Wayland desktop, VM-valid for function. | Open several active and lazy tabs, toggle Test tools, resize and move keyboard focus between chrome and the page. Capture the viewport and shortcut receipt. | Headless smoke has no interactive Wayland seat. |
| LX-02 | Ubuntu x64, bare-metal GNOME X11/Wayland, 125% and 150% scaling. | Repeat clicks near edges while resizing and check rendered bounds, text and hit locations. Record desktop settings. | No input or scaling evidence from CI. |
| LX-03 | Ubuntu x64, GNOME X11/Wayland, VM-valid, installed IME. | Compose and commit CJK text in chrome and in a page text field. Record the IME and expected resulting text. | Preedit display and input depend on the native GTK/WebKit configuration. |
| LX-04 | Ubuntu x64 bare metal with a usable GPU, both sessions where applicable. | Record compositor/renderer, compare acceleration and software behavior with native window captures. | Headless renderer availability and physical-server CPU identity cannot establish GPU composition. |
| HD-01 | macOS arm64 and Windows x64 bare metal with displays at different scaling factors. | Move the same window between displays during focus, input and resizing. Record scale factors and viewport capture. | Multi-monitor and mixed-DPI validation remain open. |

## Verification and readiness

The packages in the manifest share application source commit `3f883b353fce9e3a2c47844d18fb93afcd01a90c`. All three archive hashes and enclosed native binary hashes match their metadata. macOS and Windows packages were downloaded from the exact GitHub Actions run. The Ubuntu package was built in an isolated Ubuntu 24.04 container on the authorized headless host and extracted under the test user before both backend runs. [Build manifest](evidence/m0b-validation/builds.json), [archive verification](evidence/m0b-validation/archive-verification.txt), [hosted jobs](https://github.com/4f4d/muffin-browser/actions/runs/44591002246)

| Platform | Exact-package observation | Handoff |
|---|---|---|
| macOS arm64 | Hosted macOS 15.8.2 with WKWebView 20621.3.11.11.3 passed the required native run and relaunch, including related opener and close, two authenticated files, the full 8 MiB file and normal/private storage. The local macOS 15.8.3 build from the preceding commit also passed its synthetic run. The native application and controls were observed on the active Mac. Its window-scoped screenshot refused capture. | Use the hosted package for the supported human cases. Tab audio/mute and in-progress cancellation remain unavailable in this adapter. [Hosted smoke](evidence/m0b-validation/mac-ci-3f883-summary.json), [local smoke](evidence/m0b-validation/mac-local-ed918-summary.json), [desktop observation](evidence/m0b-validation/mac-interactive-observation.txt) |
| Windows x64 | Server 2025 with WebView2 156.0.4314.58 passed the required native run and relaunch. Both authenticated downloads matched the reference hash. The slow attachment reported native progress and the live operation was interrupted. | Use the hosted package on Windows 11 for the manual cases. Windows 11 shortcuts, interactive focus, renderer recovery, DPI and multiple monitors remain open. [Hosted smoke](evidence/m0b-validation/windows-ci-3f883-summary.json) |
| Ubuntu 24.04 x64 | The same ELF ran under Xvfb and headless Weston with WebKitGTK 2.52.6. Navigation, trusted chrome, related opener and close, synthetic upload submission and private/normal relaunch checks ran. Both backends failed the authenticated download, repeat download, progress and cancellation checks. The engine reported destination errors. | Use the local **partial** package for the supported navigation, storage, popup and other manual cases. Hold Linux DL-01 until the destination adapter is fixed and verified. Actual GNOME Wayland and X11 desktops remain required. [Xvfb](evidence/m0b-validation/linux-host-3f883-x11-summary.json), [Weston](evidence/m0b-validation/linux-host-3f883-wayland-summary.json), [destination diagnostics](evidence/m0b-validation/linux-host-3f883-x11-first.log) |

The synthetic suite uses actual system WebViews. The popup action was JavaScript-generated on macOS, Windows and the local Linux runs, so those logs establish opener and close behavior for the tested action. The first hosted Xvfb run used a targeted XTEST click and reached opener testing before its idle timeout. The intermediate hosted run recorded engine completion events for both attachments but found zero matching payloads in the expected directory. Its opener check passed and its native close case failed. Synthetic upload submission uses a fixture-selected file object. Native file chooser, print UI, full codec playback, real provider acceptance and unexpected renderer termination remain open. Test tools expands the chrome region. Overlapping native composition remains an S1 investigation. The address display can lag same-document routing. [CI history](evidence/m0b-validation/builds.json), [intermediate Xvfb results](evidence/m0b-validation/linux-ci-ed918-summary.json), [test source](../../../spikes/m0b-validation/src/smoke.rs), [desktop observation](evidence/m0b-validation/mac-interactive-observation.txt)

The repository's 82 checks, six S7 checks, S7 artifact verifier, five validation Rust checks, seven fixture checks and status generator passed. macOS Intel and Windows ARM64 type-check from the Mac host. Their native packages and runtime checks remain unverified. The previous M0a records and human M0b results are preserved. [Automated verification](evidence/m0b-validation/automated-verification.txt), [architecture checks](evidence/m0b-validation/architecture-checks.txt), [independent review](M0B_VALIDATION_REVIEW.md)

## Evidence boundaries

The prior M0a feasibility kit and its [identity manifest](evidence/continuation-m0b-prep-manifest.json) remain available as historical evidence. The current package and smoke receipts describe a distinct browsing build. No manual case is filled by packaging, a fixture run, a headless window or a compile. The S1 architecture decision, interactive Linux and Windows gaps, unexpected WebContent recovery, store/renderer behavior under crash and product security enforcement remain open. Revisit the S5 memory comparison with a representative browsing workload before making a memory claim. [M0a continuation handoff](M0A_CONTINUATION_SUMMARY.md), [S5 baseline](S5_COMPARATIVE_BASELINES.md), [generated status](../../STATUS.json)
