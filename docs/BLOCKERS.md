# Muffin Browser — Blockers

Active attempts are recorded below.

## Entry format
For each blocker record: date, milestone, acceptance criterion, affected platform, classification (product / test / environment / platform limitation / external dependency), evidence (artifact paths), impact, safe workaround, owner, next decision.

## Attempt log (write the hypothesis BEFORE each attempt)
| # | Hypothesis (one line, written before running) | What was run | Outcome | Artifact |
|---|---|---|---|---|

| 1 | M0a Windows engine floor: linking Advapi32 will resolve the static WebView2 loader ETW and registry symbols. | Add Advapi32.lib to the native link. | Pending | https://github.com/4f4d/muffin-browser/actions/runs/44537537356 |
An attempt counts as different only if its hypothesis differs from the earlier ones. After three logged attempts with distinct hypotheses, stop retrying that criterion. Do not fabricate a pass or weaken the criterion to hide it.
