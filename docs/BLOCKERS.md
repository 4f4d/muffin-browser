# Muffin Browser — Blockers

Active attempts are recorded below.

## Entry format
For each blocker record: date, milestone, acceptance criterion, affected platform, classification (product / test / environment / platform limitation / external dependency), evidence (artifact paths), impact, safe workaround, owner, next decision.

## Attempt log (write the hypothesis BEFORE each attempt)
| # | Hypothesis (one line, written before running) | What was run | Outcome | Artifact |
|---|---|---|---|---|
| 1 | M0a Windows engine floor: linking Advapi32 will resolve the static WebView2 loader ETW and registry symbols. | Add Advapi32.lib to the native link. | Resolved. WebView2 loaded about:blank. | docs/reports/m0/evidence/engine-floor-windows-link-attempt.txt; docs/reports/m0/evidence/engine-floor-windows.txt |
| 1 | Report timestamp: recording the reviewed final files in a fresh commit yields a real committer time strictly after the observed stop. | Recommit final report bodies and regenerate status. | Resolved; generator passed | docs/reports/m0/evidence/report-clock-check.txt |
| 1 | Wayland floor: running Weston after Xvfb in the same Ubuntu job will reuse a successful dependency installation and avoid a second independent apt wait. | Share one install and give apt bounded network retries. | Original run completed. The revised job hit its timeout while fetching recommended extras | https://github.com/4f4d/muffin-browser/actions/runs/44537537215 |
| 1 | S1 WRY dependency fetch: the configured elevation path provides external DNS access to Cargo. | Rerun the same build outside the filesystem/network sandbox. | Resolved; pinned deps built outside sandbox | docs/reports/m0/evidence/s1-wry-sandbox-build.txt |
| 1 | S1 Tauri build: pinning its declared tauri-build minimum 2.7.6 resolves the rejected 2.5.3 dependency selection. | Change Tauri build dependency to 2.7.6. | Dependency resolution passed; compiler diagnostics followed | Pending Tauri writer build log |
| 1 | S1 screenshot tool: directing the Swift module cache to the approved temporary directory resolves the compiler filesystem error. | Compile window-scoped capture with an explicit module cache path. | Resolved; capture tool compiled | docs/reports/m0/evidence/s1-macos-swift-cache.txt |
| 1 | S1 Tauri macOS compiles after adapting calls to the resolved Tauri 2.12.7 and runtime-wry 2.12.5 APIs. | Correct builder and event APIs against compiler output. | Reduced to Rect constructor and icon validation errors | docs/reports/m0/evidence/s1-tauri-compiler-attempt.txt |
| 2 | S1 Tauri macOS builds with Rect literals and a configured minimal PNG icon. | Use a LogicalRect literal and minimal platform metadata. | Icon error resolved; type mismatch remains | docs/reports/m0/evidence/s1-tauri-rect-icon-attempt.txt |
| 1 | macOS native screenshot: using the owned process ID rather than window title addresses unavailable Quartz window names. | Capture the layer-0 window owned by the exact fixture process. | No on-screen layer-0 window found | docs/reports/m0/evidence/s1-wry-macos-initial.txt |
| 1 | macOS view focus: activating the fixture app at the AppKit application level before view focus checks will make document focus observable. | Activate the exact test process before the focus step. | Activation returned success; DOM focus remained false | docs/reports/m0/evidence/s1-wry-macos-initial.txt |
| — | Tauri worker stopped after three failed build attempts across resolver and compiler phases. | Preserved a draft type correction without another build. | Blocked; runtime and lifecycle remain unverified | docs/reports/m0/evidence/s1-tauri-resize-type-attempt.txt |
| 2 | Linux runner setup: omitting recommended packages avoids spending most of the job installing unrelated media and desktop extras while retaining explicit GTK and capture dependencies. | Add --no-install-recommends to the runner apt install. | Pending | docs/reports/m0/evidence/linux-ci-apt-timeout.txt |
| 2 | macOS GUI evidence: LaunchServices will give a bundled fixture app a regular foreground window where direct terminal execution did not. | Build a minimal app bundle, start with open, and capture by its own PID. | No matching WindowServer entry. Both focus readings false. | docs/reports/m0/evidence/s1-wry-macos-direct.txt |
| 1 | Ubuntu server setup: an existing rootless runtime or available user namespace can provide a temporary package namespace without interactive system installation. | Inspect available container/VM commands and test an ordinary user namespace. | Docker server accessible; raw user namespace rejected | docs/reports/m0/evidence/ssh-linux-package-attempt.txt |
| 1 | Windows runner capture: an owned visible window can be identified by the fixture PID when title lookup fails. | Enumerate visible windows by the launched fixture PID and capture its rectangle. | Pending | https://github.com/4f4d/muffin-browser/actions/runs/44542235415 |
| 2 | Ubuntu server setup: Docker available on the server can install dependencies only in an isolated Ubuntu image and capture the WRY fixture on Xvfb. | Build the pinned spike in a fixture-only Docker context; run under an unprivileged user. | Pending | docs/reports/m0/evidence/ssh-linux-package-attempt.txt |
| 3 | macOS GUI evidence: reading the owned NSWindow number directly can reach a capturable window even if WindowServer enumeration hides it. | Log AppKit visibility/key status and pass the fixture window number to screencapture. | Pending | docs/reports/m0/evidence/s1-wry-macos-bundle.txt |

An attempt counts as different only if its hypothesis differs from the earlier ones. After three logged attempts with distinct hypotheses, stop retrying that criterion. Do not fabricate a pass or weaken the criterion to hide it.
