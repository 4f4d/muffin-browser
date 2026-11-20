# Muffin Browser — Codex starter package (v5)

**2026-11-20 update.** The M0 kickoff below records the original procedure. See the [owner implementation decision](docs/adr/0002-macos-first-implementation-and-deferred-qualification.md) for the current implementation and qualification gates.

1. Create the **public** Muffin Browser repository and open it.
2. Copy the contents of this package into the repository root, preserving `docs/`, `tools/`, and `.github/`.
3. If the repository already has an `AGENTS.md`, inspect and merge these project rules without deleting or weakening existing instructions.
4. Enable the local guard hook: `git config core.hooksPath tools/hooks`
5. Confirm both local guards work:
   - `python3 -m unittest discover -s tools/tests -v`
   - `python3 tools/gen_status.py --check` (should print OK).
6. Start Codex with the contents of `CODEX_KICKOFF.md`. Codex runs **M0a only** and then stops at a hard stop.
7. After M0a, you run **M0b** (`docs/M0B_MANUAL_TESTS.md`) on a Windows machine/VM and an Ubuntu Desktop VM (Wayland and X11), plus your Mac, and record results in `docs/reports/m0/results.json`.
8. You make the explicit go / beta / no-go decision for each platform after reviewing M0a and M0b. M1 starts only after you explicitly approve proceeding and record the decision.

Decisions that are yours, not Codex's: DEC-UA (user-agent policy), the sign-in gate, per-platform go/beta/no-go, LICENSE, name clearance. See `docs/DECISIONS_PENDING.md`.

This package is an execution plan, not proof that the architecture or features work. All assumptions must be verified against the exact dependency versions and operating systems chosen.

The status-guard workflow pins `actions/checkout` to the verified v7.0.1 commit SHA and checks out full history, because result records are validated against real commits. The validators cover M0b results, platform-level records, and M0a spike-report time budgets; see `AGENTS.md` and `docs/M0B_MANUAL_TESTS.md`.

Platform-level status artifacts must be stored under `docs/reports/ci/` or `docs/reports/platform/`; ordinary project files are not accepted as test artifacts. M0a spike reports require an initial pre-start Git commit and a final post-stop Git commit. Git timestamps are corroborating metadata, not trusted clocks.
