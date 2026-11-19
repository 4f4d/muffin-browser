# Muffin Browser — Blockers

Active attempts are recorded below.

## Entry format
For each blocker record: date, milestone, acceptance criterion, affected platform, classification (product / test / environment / platform limitation / external dependency), evidence (artifact paths), impact, safe workaround, owner, next decision.

## Attempt log (write the hypothesis BEFORE each attempt)
| # | Hypothesis (one line, written before running) | What was run | Outcome | Artifact |
|---|---|---|---|---|
| 1 | M0a Windows engine floor: linking Advapi32 will resolve the static WebView2 loader ETW and registry symbols. | Add Advapi32.lib to the native link. | Resolved. WebView2 loaded about:blank. | docs/reports/m0/evidence/engine-floor-windows-link-attempt.txt; docs/reports/m0/evidence/engine-floor-windows.txt |
| 1 | Report timestamp: recording the reviewed final files in a fresh commit yields a real committer time strictly after the observed stop. | Recommit final report bodies and regenerate status. | Resolved; generator passed | docs/reports/m0/evidence/report-clock-check.txt |
| 1 | Wayland floor: running Weston after Xvfb in the same Ubuntu job will reuse a successful dependency installation and avoid a second independent apt wait. | Share one install and give apt bounded network retries. | Original run eventually completed; revised run tracked separately | https://github.com/4f4d/muffin-browser/actions/runs/44537537215 |
| 1 | S1 WRY dependency fetch: the configured elevation path provides external DNS access to Cargo. | Rerun the same build outside the filesystem/network sandbox. | Resolved; pinned deps built outside sandbox | docs/reports/m0/evidence/s1-wry-sandbox-build.txt |
| 1 | S1 Tauri build: pinning its declared tauri-build minimum 2.7.6 resolves the rejected 2.5.3 dependency selection. | Change Tauri build dependency to 2.7.6. | Pending | Pending Tauri writer build log |
| 1 | S1 screenshot tool: directing the Swift module cache to the approved temporary directory resolves the compiler filesystem error. | Compile window-scoped capture with an explicit module cache path. | Pending | docs/reports/m0/evidence/s1-macos-swift-cache.txt |

An attempt counts as different only if its hypothesis differs from the earlier ones. After three logged attempts with distinct hypotheses, stop retrying that criterion. Do not fabricate a pass or weaken the criterion to hide it.
