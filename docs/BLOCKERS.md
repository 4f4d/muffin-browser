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

An attempt counts as different only if its hypothesis differs from the earlier ones. After three logged attempts with distinct hypotheses, stop retrying that criterion. Do not fabricate a pass or weaken the criterion to hide it.
