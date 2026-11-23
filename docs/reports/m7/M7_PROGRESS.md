# M7 progress and qualification handoff

The owner deferred human testing on 2026-11-23 and authorized continued engineering. The target for local qualification is Apple Silicon macOS. M6 visual and accessibility acceptance, M0b qualification and release approval remain open. [Human testing checklist](HUMAN_TESTING_CHECKLIST.md), [owner decision](../../adr/0007-defer-human-qualification-during-m7.md)

## Candidate verification

From a clean Apple Silicon checkout, create the ignored parent directory with `mkdir -p target` and run `python3 tools/package_macos_qualification.py --out target/qualification`. Use a new output directory under `target/` or outside the checkout. The command runs the engineering verifier and builds a local ZIP containing the release executable. It records source, binary and archive identity, checks the archive round trip, and tests that the extracted executable rejects debug control. Run `python3 tests/native/run_launchservices_smoke.py --verified-smoke target/qualification/native/smoke.json --out target/qualification/launchservices` to exercise macOS URL delivery with the verified debug binary and loopback URLs. The test leaves the system default browser preference alone.

### 2026-11-23 local results

Clean source `bce769de8892e78cca94d6299e265fd4d3fef578` ran on macOS 15.8.3 build 24H241, arm64, with WebKit 20621.3.11.11.3.

- The qualification ZIP is **2,157,148 bytes**. Its application payload is 5,163,730 bytes. Extracting the archive preserved the executable and plist hashes, launch permission and arm64 architecture. The bundle declares HTTP and HTTPS. The extracted release executable rejected debug control. [Archive and binary receipt](../platform/m7/macos-arm64-qualification.json)
- The complete local verifier passed 83 tooling tests, one fixture test, 27 UI tests, 45 Rust tests, the generated status check, Svelte checks and builds, formatting, Clippy, dependency audits, both Rust builds and 59 native smoke steps. npm audit reported zero vulnerabilities. Cargo audit succeeded with two allowed warnings for unmaintained `proc-macro-error` and unsound `glib 0.18.5`. An inverse dependency tree put both in the Linux GTK graph and neither in the Apple Silicon graph. [Native verifier receipt](../platform/m7/macos-arm64-native-smoke.json), [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370), [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429)
- The expanded debug smoke passed 75 steps including compact layout, both palettes, native shortcut routing and download folder persistence. [Expanded native receipt](../platform/m7/macos-arm64-native-expanded.json)
- macOS Launch Services delivered two explicitly targeted HTTP fixture URLs to the running debug bundle. The regular window handled the second URL while a private window was open and focused before dispatch. [Launch Services receipt](../platform/m7/macos-arm64-launchservices.json)
- A controlled crash of the disposable debug host preserved the saved regular tab and excluded the private window. The restarted app accepted a URL forwarded by a second process, exercising stale endpoint recovery. Unexpected WebContent termination remains open. [Host crash receipt](../platform/m7/macos-arm64-host-crash.json)

The independent reviewer ran the portable verifier on the exact source, extracted the archive separately to check the payload hashes, and matched all four native receipts to the debug executable built by the full verifier. The reviewer cleared the artifact preparation work. The hosted verifier also passed on the same source on macOS 15.8.2 arm64, including 59 debug native smoke steps. The generated status includes the workflow's build record. This package is for local qualification. Distribution signing, installation and Finder launch, update and rollback, and production acceptance remain open. [Hosted verifier](https://github.com/4f4d/muffin-browser/actions/runs/45112178186), [workflow build receipt](../ci/macos-arm64-build-00000000045112178186-0001.json)

Run the crash fixture from the same clean checkout with `python3 tests/native/run_crash_smoke.py --binary-source-commit "$(git rev-parse HEAD)" --out <new-directory>`. Match its binary hash to the full verifier's debug smoke receipt before using the result as build evidence.

## macOS packaging policy and performance follow-up

### Package execution policy

The local archive retains its verified SHA-256. Its extracted executable has an arm64 linker-generated ad hoc signature. Strict bundle verification and `spctl --assess --type execute` both exited 1 with the missing-resources diagnostic. The test used the current enabled assessment settings and the archive and app as extracted, both with no recorded quarantine attribute. This is a packaged execution-policy failure. Finder and quarantine-triggered launch behavior remain open. The Mach-O deployment target is 11.0. The qualification run remains on macOS 15.8.3. [Exact package policy receipt](../platform/m7/macos-arm64-package-policy.json)

For direct macOS distribution, the remaining sequence is an owner-approved Developer ID signing and notarization policy, a sealed final bundle, and a new archive assessment with its own hashes. Recheck a quarantined copy in a disposable macOS session. [Apple distribution guidance](https://developer.apple.com/documentation/technologyoverviews/distribution), [signing decision](../../DECISIONS_PENDING.md)

### Cold link delivery

Launch Services cold-started a uniquely identified debug bundle and delivered the loopback HTTP URL outside its command-line arguments. The fixture received the targeted GET. The initial regular window showed the fixture URL and title, then exited through the isolated debug channel. A separate injected shutdown timeout exercised the fallback. AppKit matched the fixture bundle ID, bundle path and executable path, stopped that exact app, and confirmed it had exited before the isolated directory was removed. Both checks used the debug build from `bce769d` with the committed harness at `207e224`. Release-bundle cold launch and system default-browser adoption remain open. [Cold URL receipt](../platform/m7/macos-arm64-cold-launch.json), [timeout cleanup receipt](../platform/m7/macos-arm64-cold-cleanup.json)

### Physical Mac shell benchmark

Five fresh debug app processes ran on a 10-core Apple M4 with 24 GiB, macOS 15.8.3 and WebKit 20621.3.11.3, on battery with Low Power Mode enabled. Each trial loaded ten copies of the fixed loopback page into separate tabs. It sampled the main process’s physical footprint and CPU at one, five and ten loaded tabs after a 0.5-second settle and a two-second idle window. The shell PID uses macOS `proc_pid_rusage`. [Raw per-trial benchmark](../platform/m7/macos-arm64-shell-benchmark.json), [Apple SDK API](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h)

| Loaded tabs | Median shell footprint | Footprint range | Median shell CPU during idle |
|---|---:|---:|---:|
| 1 | 19.63 MiB | 19.52–19.81 MiB | 0.07% of one core |
| 5 | 20.27 MiB | 19.86–20.52 MiB | 0.09% of one core |
| 10 | 20.97 MiB | 20.73–21.45 MiB | 0.10% of one core |

Median time from process spawn to the fixture's chrome-ready observation was 2.44 seconds (2.43–2.61 seconds). The first loopback navigation reached fixture ready in a median of 216 ms. Observed discard and restore took 108 ms and 220 ms respectively. The commands and 50 ms polling belong to these wall-clock observations. The median footprint change 0.5 seconds after discard was +0.02 MiB at the host, leaving memory reclamation unverified. Scope is the debug shell binary from `bce769d`. Release startup, first paint, Safari comparison and attributed WebKit process memory and CPU remain open. [Benchmark harness](../../../tests/native/run_shell_benchmark.py), [raw timings and footprint](../platform/m7/macos-arm64-shell-benchmark.json)

Run either follow-up harness from a clean checkout with `--binary <debug-executable> --verified-smoke <matching-native/smoke.json> --out <new-ignored-directory>`. The receipts record separate harness and application source commits. Keep the Mac idle during the benchmark and record its power configuration. [Cold launch harness](../../../tests/native/run_cold_launch_smoke.py), [benchmark harness](../../../tests/native/run_shell_benchmark.py)

### Follow-up verification

The full local verifier passed on clean `207e224` with 83 tooling tests, one fixture test, 27 UI tests, 45 Rust tests and 59 native smoke steps. The independent reviewer checked the benchmark, independently matched the debug executable and smoke hashes, and cleared the cold-launch harness after reviewing its normal and forced-cleanup receipts. The package execution-policy failure remains open. [Full verifier receipt](../platform/m7/macos-arm64-follow-up-verifier.json)

### WebContent recovery

The public navigation delegate reports termination against a `WKWebView`. The available public interfaces did not establish which live WebContent process belonged to the isolated fixture, and WebKit can reuse processes. Actual WebContent loss and recovery remain unverified. Earlier host-crash evidence covers the separate Muffin shell process. [Apple termination callback](https://developer.apple.com/documentation/webkit/wknavigationdelegate/webviewwebcontentprocessdidterminate(_:)), [Apple process pool documentation](https://developer.apple.com/documentation/webkit/wkwebviewconfiguration/processpool), [host crash receipt](../platform/m7/macos-arm64-host-crash.json)

## Security review

Assets in scope include browser authority, regular and private profile state, downloaded files and distribution integrity. The principal inputs are remote pages and redirects, OS link events, second-instance requests, downloaded data, and the packaged executable.

| Boundary | Control and evidence | Residual qualification |
|---|---|---|
| Remote content and native browser commands | The native shell polls a private chrome WebView that loads bundled HTML with a hash-based content security policy. Content views have their own configuration and no privileged command handler. The native fixture checks that an inert page-owned action queue triggers neither the monitored native-read canary nor an unsolicited popup request. [Chrome policy](../../../ui/scripts/inline-bundle.mjs), [WebView construction](../../../crates/muffin/src/main.rs#L622), [native fixture](../../../tests/native/run_smoke.py#L51) | Extend production boundary coverage to cross-origin subframes, popup lifecycles, and synthetic app-file read denial. |
| External URLs and instance forwarding | Native routing permits bounded HTTP(S) addresses without embedded credentials. A private Unix socket accepts bounded requests from the same local user. [URL policy](../../../crates/muffin/src/main.rs#L2818), [instance protocol](../../../crates/muffin/src/instance.rs) | Confirm cold and running app delivery through the macOS default-browser path. Same-user processes are trusted to submit web URLs. |
| Transfers and certificate errors | Native download approval gates file creation. Retry is bounded to an eligible bodyless HTTP GET and drops original headers and cookie handling. Native fixtures verify stored bytes, quarantine and TLS failure behavior with loopback content. [Download delegate](../../../crates/muffin/src/download_delegate.rs), [native suite](../../../tests/native/run_smoke.py) | Qualify authenticated downloads, macOS quarantine behavior and real origin and device permissions. |
| Private state and renderer lifetime | The core serializes regular session metadata only. Private windows use an ephemeral data store, and the native fixtures inspect profile and session state after restart. The content renderer callback checks generation and allows one recovery attempt on an eligible URL. [Privacy evidence](../m5/M5_PROGRESS.md), [renderer recovery code](../../../crates/muffin/src/main.rs#L2278) | Qualify unexpected WebContent loss and inspect OS and WebKit crash or cache artifacts on a disposable installation. |
| Distribution integrity | The package receipt binds the local archive and executable to a clean source commit. The extracted release executable rejects debug control. | Distribution signing, notarization, update trust and rollback need approved policy and distribution artifacts. |

## Open qualification

- Launch the distribution build from Finder and exercise OS default-browser selection, cold launch with a link, Gatekeeper, installation and removal in a disposable environment. The current archive fails the package execution-policy check.
- Measure release startup, first paint, CPU and physical memory across the application and its attributable WebKit processes against Safari in fixed scenarios. Record installer and runtime dependencies separately.
- Confirm renderer termination and recovery with attributable WebContent evidence.
- Record the distribution identity, signing, notarization, update verification and rollback policy before distribution testing. The decision register tracks approvals and the remaining launch gates. [Decision register](../../DECISIONS_PENDING.md)
- Qualify native accessibility and input with the owner on Mac. Windows 11, GNOME X11 and GNOME Wayland retain their prior evidence and platform status. [Generated status](../../STATUS.json)
