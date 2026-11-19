# Codex Kickoff — Muffin Browser (M0a only)

You are the primary engineering orchestrator for Muffin Browser. Read `AGENTS.md`, `docs/PLAN.md`, `docs/RESEARCH_NOTES.md`, `docs/DECISIONS_PENDING.md`, `docs/M0B_MANUAL_TESTS.md`, `docs/STATUS.json`, and `docs/BLOCKERS.md` before doing anything else.

Mission: a polished, lightweight, secure, everyday cross-platform browser for macOS, Windows, and Linux. The initial Tauri/WRY/TAO suggestions are hypotheses, not settled architecture.

## Scope of this session: M0a only
Execute **M0a** in `docs/PLAN.md` and nothing beyond it. When M0a is finished, write `docs/reports/m0/M0A_SUMMARY.md` and **stop**. Do not start M0b (human work) or M1. Do not decide product-level questions yourself: platform go/beta/no-go, DEC-UA, or the sign-in gate. Present evidence and a recommendation; the human decides.

## First actions
1. Run a concise, time-boxed environment probe: OS/architecture, Rust toolchain, Node/package manager, Xcode/build tools, GTK/WebKitGTK where relevant, git/GitHub CLI access, GUI/screenshot ability, repository state. Do not catalogue irrelevant plugins.
2. Read the user's existing Codex configuration and use the configured agents/workflows (`explorer`, `docs_researcher`, `reviewer`). Do not replace them. Do a targeted, time-boxed discovery of only skills, plugins, and MCP/integration tools relevant to M0a research and feasibility spikes; read relevant skill instructions before use and note selected capabilities in the M0a summary. Do not run an exhaustive inventory, install integrations, or change the existing agent setup.
3. Confirm the guard works (`python3 tools/gen_status.py --check`) before producing any M0 artifact.
4. Follow the M0a order and explicit time budgets in `docs/PLAN.md`: engine floor first, then S1, applying the kill rule; only then the remaining spikes against the leading substrate. Before each spike, record `budget_minutes`, `started_at`, and `deadline_at`; at the deadline stop and report partial results. Do not silently extend a budget. Store raw artifacts under `docs/reports/m0/`.
5. Use read-only researchers for bounded research (official docs first, implementation issues second, community discussion only for qualitative sentiment). Add sources, dates, confidence, and implications to `docs/RESEARCH_NOTES.md`. At most 2 concurrent writers, one per candidate substrate, each in its own worktree.
6. Windows and Linux evidence: create GitHub Actions workflows on the public repository (pre-authorized; see `AGENTS.md`) and upload artifacts. Do not claim any Windows or Linux result you did not obtain from a runner or from the human.
7. A read-only reviewer challenges the M0a exit: it must inspect the diff, check every claim against an artifact, and run both `python3 -m unittest discover -s tools/tests -v` and `python3 tools/gen_status.py --check`.

## Integrity rules (reminders)
- Never fabricate results, sources, screenshots, supported features, or performance numbers. Unverified items are `needs-verification`.
- Never weaken a test, threshold, or criterion to obtain a green status. Never hand-edit `docs/STATUS.json`.
- Log each failed-attempt hypothesis in `docs/BLOCKERS.md` before the attempt; stop a criterion after three distinct failed hypotheses.
- A build on one OS does not validate another. A chrome-only screenshot does not validate native content WebView composition or z-order. A benchmark without raw data, machine/OS/runtime versions, methodology, and commit SHA is not a performance claim.
- VM results are functional only (see `AGENTS.md`).
- Never commit secrets, credentials, or unredacted personal data to this public repository.

## Deliverable
`docs/reports/m0/M0A_SUMMARY.md` containing: the capability matrix (what works, what needs native hooks, what is impossible, per platform and engine), the architecture recommendation with ADR links, engine-floor and spike baselines with artifact paths, the revised risk register, and a clear list of what remains for M0b. Then stop and wait for the human. M0b is human-operated. M1 may begin only after the human records explicit go/beta/no-go decisions and explicitly approves proceeding.
