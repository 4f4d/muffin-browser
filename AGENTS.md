# Muffin Browser — Engineering Rules

These project instructions supplement the user's existing Codex instructions and agent setup. Do not replace, disable, or silently rewrite global instructions, configured agents, workflows, or model defaults. Inspect them first; propose a narrowly scoped change only when a demonstrated project need justifies it.

## Mission
Build Muffin as a lightweight, secure, polished, cross-platform desktop browser for macOS, Windows, and Linux. Prioritize reliable everyday browsing, a coherent premium interface, low shell overhead, and honest platform support. The small download target is not proof of low runtime memory.

## Decision discipline
- Research before architecture. Record material decisions as ADRs in `docs/adr/` with context, options, evidence, decision, trade-offs, and revisit triggers.
- Treat `docs/PLAN.md` as the project plan and `docs/STATUS.json` as generated verification state, not an assertion by the implementer.
- Do not assume Tauri child WebViews are stable or that WRY child-view behaviour is identical on X11 and Wayland. M0 decides the substrate.
- Do not claim uniform support where the underlying engines differ. Record status per feature and platform.
- Do not add accounts, sync servers, telemetry, AI features, a custom engine, bundled Chromium, or extensions to v1 without a separately reviewed ADR.
- If documentation, a user request, or an agent report conflicts with current official platform documentation, verify and record the discrepancy.

## Agent orchestration
- Reuse the existing configured `explorer`, `docs_researcher`, `reviewer`, workflows, and model routing where present. Do not create a duplicate agent for an existing capability.
- At the start of each milestone, perform targeted discovery of installed skills, plugins, and MCP/integration tools relevant to that milestone. Read the relevant skill instructions before use; use a capability only when it materially helps and is authorized. Record the few selected capabilities and their purpose in that milestone's handoff. Do not perform an exhaustive inventory, install integrations, or replace existing agent configuration.
- Prefer 1–2 child agents for ordinary work. Never run more than 2 concurrent writers. During M0a use read-only researchers plus at most one spike writer per candidate substrate. The test-harness writer starts in M1, not M0. Use the configured concurrency ceiling; do not maximize concurrency by default.
- Keep the primary orchestrator responsible for architecture, typed IPC contracts, integration, and milestone decisions.
- Use separate worktrees/branches for concurrent writers and assign exclusive directory ownership. Never have multiple writers edit the same files concurrently.
- The reviewer is read-only and adversarial. It must inspect the diff and run the relevant verifier; it cannot approve its own implementation.
- Use the lowest-cost capable model for bounded research and routine implementation. Escalate reasoning for architecture, security, native platform integration, and milestone exit reviews. Select only models actually available in the environment.
- Require agents to return concise findings, source links, confidence, assumptions, risks, changed files, and test evidence.

## Security boundaries
- Treat all remote web content as untrusted.
- The browser chrome may use a narrowly scoped native IPC contract. Remote content WebViews must receive no privileged Muffin IPC bridge or native capabilities.
- Keep browser profile data, SQLite, settings, and secrets in the native core. Never expose the database directly to a WebView.
- Validate URL schemes, origins, external protocol launches, download destinations, and all data crossing IPC.
- Use least privilege, strict CSP for bundled chrome assets, safe text rendering, secure OS credential storage, and explicit permission prompts.
- Never silently bypass certificate errors, OAuth restrictions, OS security controls, or platform entitlement requirements.
- Private browsing must use isolated temporary engine storage, not merely delete history after the fact. Define and test exactly what is and is not persisted.

## Testing integrity
- Every milestone has executable acceptance checks. `just verify` (or a documented equivalent) must run the required checks and exit non-zero on failure.
- A passing status must be linked to the current commit SHA, platform, environment, and actual command/artifact.
- Never weaken, skip, delete, quarantine, or loosen a test/threshold merely to obtain a green build. Changes to tests, fixtures, or thresholds require a written rationale/ADR and independent reviewer approval.
- Distinguish product failure, test defect, environment failure, unsupported platform capability, and flaky external service.
- Use deterministic local fixtures for blocking CI. Use real websites only for non-blocking canaries or explicitly recorded manual checks.
- Visual claims require screenshot artifacts of the actual application. Playwright screenshots of the chrome-only gallery do not prove native WebView composition or z-order.
- Performance claims require raw machine-readable benchmark output, commit SHA, machine identity/specification, OS, engine/runtime versions, and methodology.
- Never infer cross-platform runtime success from a successful build on another OS.

## Work cadence and blockers
- Execute one bounded milestone at a time. At each boundary, update the plan, generated status, ADRs, test evidence, and next actions.
- Continue automatically between milestones only when (a) all objective gates pass with reviewer sign-off and (b) the next boundary is not a hard stop. **Hard stops (always halt and wait for the human):** end of M0a, end of M0b, end of M6, and any publish/release. At a hard stop, write the handoff/summary file and stop; do not start the next milestone.
- The human, not the orchestrator, makes product-level decisions that M0 can force: dropping or downgrading a platform, choosing the substrate when evidence is mixed, and accepting a failed sign-in/passkey/DRM gate.
- Before each attempt on a failing criterion, append its hypothesis (one line) to `docs/BLOCKERS.md`. An attempt only counts as different if its hypothesis differs from earlier ones. After three logged attempts with distinct hypotheses, stop work on that criterion. Do not loop identical commands or conceal the blocker.
- Every M0a spike must use the explicit budget in `docs/PLAN.md` and have a report at `docs/reports/m0/spikes/<spike>-<platform>-<substrate>.md`. Before starting the spike, commit an initial header containing `spike`, `platform`, `substrate`, `budget_minutes`, `started_at`, and `deadline_at`; the stop fields are deliberately omitted until the spike ends. The generator accepts this committed state as `in_progress`. After the initial report commit, run `python3 tools/gen_status.py`, then `python3 tools/gen_status.py --check`, and commit the refreshed `docs/STATUS.json` before starting the timed work. After stopping, fill `stopped_at`, `stopped_reason`, and `outcome`, complete the report, and commit that final version. Then regenerate/check/commit `docs/STATUS.json` again. The report must be tracked, clean, and represented by at least two distinct commits when complete. The initial commit time must be before and within five minutes of `started_at`; the final commit time must be after and within five minutes of `stopped_at`. The generator enforces these conditions and the budget/outcome rules. The pre-commit hook defers only its `STATUS.json` check when a spike report is staged, because the report commit is not yet visible to Git; immediately after that commit, regenerate and validate `STATUS.json` as described above. **Git commit timestamps are corroborating metadata only, not a cryptographically trusted clock; the reviewer must inspect the commit history and artifacts.** Time spent on setup, dependency downloads, and failed attempts counts. At the deadline, stop the spike and report partial findings; do not extend or restart it silently. **A spike stopped by its deadline is recorded as `outcome: inconclusive` with `stopped_reason: deadline`, never `fail`, and never triggers the kill rule.** A blocked stop must have outcome `blocked` or `inconclusive`; `outcome: blocked` requires `stopped_reason: blocked`; `outcome: fail` requires `stopped_reason: completed`. Inconclusive and blocked spikes go to the human for a decision (the generator lists them under `needs_human_decision`). The kill rule fires only on an S1 spike that completed and failed on macOS or Windows. A new budget requires explicit human approval and a documented reason.
- Keep working on independent, safe tasks when one platform is blocked, but do not mark the blocked platform as verified.
- Do not publish, sign with private credentials, purchase services, upload releases, or change external accounts without explicit authorization.
- At every new session or context recovery, read `AGENTS.md`, `docs/PLAN.md`, `docs/STATUS.json`, `docs/BLOCKERS.md`, recent ADRs, and the latest verification artifacts before continuing.

## Definition of done
A feature is done only when its acceptance checks pass, platform limitations are documented, security implications are reviewed, the UI has been inspected where relevant, and user-facing documentation matches the implementation. A release is complete only for platforms that have the required build, smoke, functional, and packaging evidence. Label other platforms honestly as build-only, beta, or unverified.

## Generated status and evidence
- `docs/STATUS.json` is generated by `tools/gen_status.py` from result records under `docs/reports/`. Never hand-edit it. The pre-commit hook (`tools/hooks/pre-commit`) and CI (`.github/workflows/status-guard.yml`) run `python3 -m unittest discover -s tools/tests -v`. The hook runs `python3 tools/gen_status.py --check` except when an M0a spike report is staged; in that case it defers only the status check until after the report commit, because the new commit is not yet visible to Git. Immediately after that commit, regenerate/check/commit `docs/STATUS.json`. CI always runs the generator check.
- During M0a/M0b every claim in an ADR or report must cite an artifact path (raw log, JSON, or redacted screenshot). Claims without artifacts are `needs-verification`.
- Platform-level records (`docs/reports/results/*.json`; levels build/smoke/functional/release) get the same rigour for both `pass` and `fail`: a full 40-character commit SHA that exists in this repository's Git object database, a non-empty artifact file plus its `artifact_sha256`, a unique record `id`, and either a GitHub Actions `run_url` (`ci_runner`) or a named `tester` (`bare_metal`/`vm`). Platform-level artifacts must live under `docs/reports/ci/` or `docs/reports/platform/`; ordinary project files, directories, paths outside the repo, paths under `evidence-private/`, and symlinks escaping the selected artifact directory are rejected. A VM cannot satisfy `release`. The validator proves internal consistency, not truth: the reviewer must still open the artifact and the run. From M1 the CI workflow, not an agent, writes `ci_runner` records.
- Every M0b `pass`, `fail`, `partial`, or `inconclusive` record must identify the exact application source commit (`app_commit`, full 40-character SHA that exists in this repository's Git object database), tested binary (`binary_sha256`, full 64-character SHA-256), host and binary architectures, exact OS and engine/runtime versions, tester, ISO date, and a non-empty evidence file. Evidence must resolve inside the repository and outside `evidence-private/`; missing, empty, traversal, symlink-escaped, or private evidence paths are rejected.
- Tests marked `vm_valid: false` are bare-metal-only. The generator rejects both `vm` and `ci_runner` results for these tests, irrespective of outcome. `not_run` is the only outcome that does not require build identity or evidence. Run `python3 -m unittest discover -s tools/tests -v` as well as the status generator check.
- Results from virtual machines are valid for functional behaviour only. They are never valid evidence for GPU/Wayland rendering behaviour, DRM, passkeys with hardware authenticators, or performance.
- Cells that need a human (see `docs/M0B_MANUAL_TESTS.md`) stay `not_run` until the human records them. Never infer or pre-fill them.

## Pre-authorized actions
Allowed without asking, within the project repository the human designates:
- Create branches, commit, push, and open pull requests.
- Run GitHub Actions workflows on the repository.
- Install dependencies from standard package managers (cargo, npm, apt, brew, winget) in the dev environment and CI.
- Download public reference builds and runtimes for measurement (for example CEF builds for size/footprint numbers, reference browsers on CI runners).

Requires explicit human approval: paid services, signing keys or certificates, publishing or tagging releases, changing repository visibility/settings/secrets, creating or using accounts, and destructive operations.

## Public repository hygiene
The repository is public.
- Never commit secrets, credentials, tokens, personal information, test-account identifiers, or unredacted screenshots showing them. Raw private evidence goes in `docs/reports/m0/evidence-private/` (gitignored); only redacted copies are committed.
- Use the `pull_request` trigger for CI on fork contributions, never `pull_request_target` with checked-out untrusted code. No repository secrets are used in M0/M1 workflows.
- Pin every GitHub Action to a verified full commit SHA. The existing status-guard workflow already pins `actions/checkout` to v7.0.1; retain immutable pins and update them only through reviewed dependency changes.
- A LICENSE decision is pending (see `docs/DECISIONS_PENDING.md`). Until the human records one, do not add a license file or claim a license.

## Commands and layout (fill in at M1)
- Regression tests: `python3 -m unittest discover -s tools/tests -v`.
- Status verification: `python3 tools/gen_status.py --check` (or `just verify` after M1 creates it).
- Directory ownership (set in M1): `crates/` shell/platform writer, `ui/` chrome writer, `tests/` `tools/` `.github/` harness writer, `docs/` orchestrator.
