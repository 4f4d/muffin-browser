One report per spike, platform, and substrate. Start from `_TEMPLATE.md`. Files beginning with `_` and this README are ignored by the validator.

## Required Git lifecycle

1. Before the timed work, create the report with only the initial fields (`spike`, `platform`, `substrate`, `budget_minutes`, `started_at`, `deadline_at`) and commit it. The generator accepts this committed header as `in_progress`.
2. After the initial report commit, run `python3 tools/gen_status.py`, then `python3 tools/gen_status.py --check`, and commit the refreshed `docs/STATUS.json` before starting the timed work.
3. Run the spike within its approved wall-clock budget.
4. After stopping, complete `stopped_at`, `stopped_reason`, `outcome`, and the report body, then commit the final report.
5. Regenerate `docs/STATUS.json`, run `python3 tools/gen_status.py --check`, and commit the updated status.

The pre-commit hook runs all regression tests but defers its generated-status check when a spike report is staged, because the report commit does not yet exist in Git while the hook runs. Always regenerate and validate status immediately after committing the report.

The validator requires a tracked, clean report. A completed report must have at least two distinct commits. The initial header commit must be before and within five minutes of `started_at`; the final report commit must be after and within five minutes of `stopped_at`. The initial commit snapshot must contain the matching start header. Git timestamps are corroborating metadata only, not a cryptographically trusted clock. Reviewers must inspect commit history and artifacts rather than treating timestamps as proof of elapsed wall-clock time.
