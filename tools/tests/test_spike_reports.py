"""Regression tests for M0a spike reports: Git lifecycle, budgets, outcomes, and kill rule."""
import datetime as dt
import importlib.util
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("muffin_gen_status_spikes", REPO_ROOT / "tools" / "gen_status.py")
gen_status = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = gen_status
spec.loader.exec_module(gen_status)


def header(**kw):
    h = {
        "spike": "S1", "platform": "macos", "substrate": "wry-tao", "budget_minutes": "90",
        "started_at": "2026-11-19T10:00:00Z", "deadline_at": "2026-11-19T11:30:00Z",
        "stopped_at": "2026-11-19T11:10:00Z", "stopped_reason": "completed", "outcome": "pass",
    }
    h.update(kw)
    return {k: v for k, v in h.items() if v is not None}


def render(h):
    return "---\n" + "".join(f"{k}: {v}\n" for k, v in h.items()) + "---\n\n# Report\n"


def to_datetime(value):
    return dt.datetime.fromisoformat(str(value).replace("Z", "+00:00"))


def git_init(root):
    subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)


def git_commit_file(root, path, when, message):
    rel = path.relative_to(root).as_posix()
    subprocess.run(["git", "-C", str(root), "add", "--", rel], check=True)
    env = dict(os.environ, GIT_AUTHOR_DATE=when, GIT_COMMITTER_DATE=when)
    subprocess.run(
        ["git", "-C", str(root), "-c", "user.name=test", "-c", "user.email=test@example.com",
         "commit", "-q", "-m", message],
        check=True, env=env, capture_output=True,
    )


def utc_stamp(value):
    return value.astimezone(dt.timezone.utc).isoformat()


class SpikeReportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.old_root = gen_status.ROOT
        gen_status.ROOT = self.root
        self.dir = self.root / "docs/reports/m0/spikes"
        self.dir.mkdir(parents=True)
        git_init(self.root)
        self.n = 0

    def tearDown(self):
        gen_status.ROOT = self.old_root
        self.temp.cleanup()

    def put(self, h, name=None):
        """Create a report with a pre-start header commit and post-stop final commit."""
        self.n += 1
        path = self.dir / (name or f"r{self.n}.md")
        defaults = header()
        start_text = h.get("started_at") or defaults["started_at"]
        stop_text = h.get("stopped_at") or defaults["stopped_at"]
        try:
            started = to_datetime(start_text)
        except (TypeError, ValueError):
            started = to_datetime(defaults["started_at"])
        try:
            stopped = to_datetime(stop_text)
        except (TypeError, ValueError):
            stopped = to_datetime(defaults["stopped_at"])

        draft_budget = int(h.get("budget_minutes", defaults["budget_minutes"]))
        original_deadline = started + dt.timedelta(minutes=draft_budget)
        draft = {
            "spike": h.get("spike", defaults["spike"]),
            "platform": h.get("platform", defaults["platform"]),
            "substrate": h.get("substrate", defaults["substrate"]),
            "budget_minutes": str(draft_budget),
            "started_at": start_text,
            "deadline_at": utc_stamp(original_deadline),
        }
        path.write_text(render(draft), encoding="utf-8")
        git_commit_file(self.root, path, utc_stamp(started - dt.timedelta(seconds=30)), "start spike: commit header")
        path.write_text(render(h), encoding="utf-8")
        git_commit_file(self.root, path, utc_stamp(stopped + dt.timedelta(seconds=30)), "finish spike: commit report")
        return path

    def bad(self, message):
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn(message, str(ctx.exception))

    def test_valid_report_is_accepted(self):
        self.put(header())
        out = gen_status.spike_status()
        self.assertEqual(out["reports"]["S1:macos:wry-tao"]["outcome"], "pass")
        self.assertFalse(out["kill_rule"]["triggered"])

    def test_authorized_followup_keeps_original_report_and_applies_kill_rule(self):
        self.put(header(outcome="inconclusive", stopped_reason="blocked"))
        self.put(header(spike="S1-follow-up", budget_minutes="90", outcome="fail"))
        out = gen_status.spike_status()
        self.assertEqual(out["reports"]["S1:macos:wry-tao"]["outcome"], "inconclusive")
        self.assertEqual(out["kill_rule"]["triggering_reports"], ["S1-follow-up:macos:wry-tao"])

    def test_followup_requires_matching_closed_baseline(self):
        self.put(header(spike="S1-follow-up"))
        self.bad("requires a closed original S1 report")

    def test_followup_is_restricted_to_authorized_substrate(self):
        self.put(header(spike="S1-follow-up", substrate="tauri"))
        self.bad("only WRY + TAO")

    def test_followup_has_its_own_platform_cap(self):
        self.put(header(platform="windows"))
        self.put(header(spike="S1-follow-up", platform="windows"))
        self.bad("exceeds the 45-minute cap")

    def test_template_and_readme_are_ignored(self):
        (self.dir / "_TEMPLATE.md").write_text("not a report")
        (self.dir / "README.md").write_text("not a report")
        self.assertEqual(gen_status.spike_status()["reports"], {})

    def test_each_required_field_is_enforced(self):
        for field in gen_status.SPIKE_REQUIRED:
            with self.subTest(field=field):
                for f in self.dir.glob("*.md"):
                    f.unlink()
                self.put(header(**{field: None}))
                self.bad(f"missing required '{field}'")

    def test_missing_front_matter_is_rejected(self):
        (self.dir / "x.md").write_text("# no header\n")
        self.bad("front-matter")

    def test_budget_over_per_spike_cap_is_rejected(self):
        self.put(header(budget_minutes="120", deadline_at="2026-11-19T12:00:00Z"))
        self.bad("exceeds the 90-minute cap")

    def test_unknown_spike_is_rejected(self):
        self.put(header(spike="S99"))
        self.bad("unknown spike")

    def test_deadline_must_match_budget(self):
        self.put(header(deadline_at="2026-11-19T15:00:00Z"))
        self.bad("deadline_at must equal")

    def test_timestamp_needs_timezone(self):
        self.put(header(started_at="2026-11-19T10:00:00"))
        self.bad("timezone")

    def test_stopping_after_deadline_is_rejected(self):
        self.put(header(stopped_at="2026-11-19T11:45:00Z"))
        self.bad("after deadline_at")

    def test_small_grace_after_deadline_is_tolerated(self):
        self.put(header(stopped_at="2026-11-19T11:31:30Z"))
        gen_status.spike_status()

    def test_deadline_stop_must_be_inconclusive(self):
        for bad_outcome in ("pass", "blocked", "not_run"):
            with self.subTest(outcome=bad_outcome):
                for f in self.dir.glob("*.md"):
                    f.unlink()
                self.put(header(stopped_reason="deadline", stopped_at="2026-11-19T11:30:00Z", outcome=bad_outcome))
                self.bad("must have outcome 'inconclusive'")

    def test_deadline_stop_inconclusive_is_accepted_and_flags_human_decision(self):
        self.put(header(stopped_reason="deadline", stopped_at="2026-11-19T11:30:00Z", outcome="inconclusive"))
        out = gen_status.spike_status()
        self.assertEqual(out["needs_human_decision"], ["S1:macos:wry-tao"])
        self.assertFalse(out["kill_rule"]["triggered"])

    def test_deadline_stop_claimed_but_stopped_early_is_rejected(self):
        self.put(header(stopped_reason="deadline", stopped_at="2026-11-19T10:20:00Z", outcome="inconclusive"))
        self.bad("well before deadline_at")

    def test_blocked_reason_only_accepts_blocked_or_inconclusive(self):
        for outcome in ("pass", "not_run"):
            with self.subTest(outcome=outcome):
                for f in self.dir.glob("*.md"):
                    f.unlink()
                self.put(header(stopped_reason="blocked", outcome=outcome))
                self.bad("requires outcome 'blocked' or 'inconclusive'")

    def test_blocked_outcome_requires_blocked_reason(self):
        self.put(header(stopped_reason="completed", outcome="blocked"))
        self.bad("outcome 'blocked' requires stopped_reason 'blocked'")

    def test_valid_blocked_stop_is_accepted_and_flags_human_decision(self):
        self.put(header(stopped_reason="blocked", outcome="blocked"))
        out = gen_status.spike_status()
        self.assertEqual(out["needs_human_decision"], ["S1:macos:wry-tao"])
        self.assertFalse(out["kill_rule"]["triggered"])

    def test_valid_inconclusive_blocked_stop_is_accepted(self):
        self.put(header(stopped_reason="blocked", outcome="inconclusive"))
        out = gen_status.spike_status()
        self.assertEqual(out["needs_human_decision"], ["S1:macos:wry-tao"])

    def test_blocked_s1_fail_cannot_evade_kill_rule(self):
        self.put(header(stopped_reason="blocked", outcome="fail"))
        self.bad("outcome 'fail' requires stopped_reason 'completed'")

    def test_fail_requires_completed_stop_reason(self):
        for reason in ("deadline", "blocked"):
            with self.subTest(reason=reason):
                for f in self.dir.glob("*.md"):
                    f.unlink()
                self.put(header(stopped_reason=reason, outcome="fail"))
                self.bad("outcome 'fail' requires stopped_reason 'completed'")

    def test_kill_rule_triggers_only_on_completed_fail_on_macos_or_windows(self):
        self.put(header(outcome="fail"))
        self.assertTrue(gen_status.spike_status()["kill_rule"]["triggered"])
        for f in self.dir.glob("*.md"):
            f.unlink()
        self.put(header(platform="windows", outcome="fail"))
        self.assertTrue(gen_status.spike_status()["kill_rule"]["triggered"])
        for f in self.dir.glob("*.md"):
            f.unlink()
        self.put(header(platform="linux_x11", outcome="fail"))
        self.assertFalse(gen_status.spike_status()["kill_rule"]["triggered"])

    def test_extension_requires_human_approval_and_reason(self):
        extended = header(budget_minutes="90", extension_minutes="30", deadline_at="2026-11-19T12:00:00Z",
                          stopped_at="2026-11-19T11:50:00Z")
        self.put(extended)
        self.bad("extension_approved_by")
        for f in self.dir.glob("*.md"):
            f.unlink()
        extended.update(extension_approved_by="om", extension_reason="Cold Rust build exceeded budget")
        self.put(extended)
        gen_status.spike_status()

    def test_duplicate_restart_is_rejected(self):
        self.put(header())
        self.put(header())
        self.bad("duplicate spike report")

    def test_aggregate_budget_cap_is_enforced(self):
        for i in range(9):
            self.put(header(substrate=f"sub{i}"))
        self.bad("aggregate budget")


class SpikeGitCrossCheckTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.old_root = gen_status.ROOT
        gen_status.ROOT = self.root
        self.dir = self.root / "docs/reports/m0/spikes"
        self.dir.mkdir(parents=True)
        git_init(self.root)

    def tearDown(self):
        gen_status.ROOT = self.old_root
        self.temp.cleanup()

    def initial_and_final_commits(self, *, first_offset_seconds=-30, final_offset_seconds=30, initial_overrides=None):
        path = self.dir / "s1.md"
        h = header()
        started = to_datetime(h["started_at"])
        stopped = to_datetime(h["stopped_at"])
        original_deadline = started + dt.timedelta(minutes=int(h["budget_minutes"]))
        draft = {
            "spike": h["spike"], "platform": h["platform"], "substrate": h["substrate"],
            "budget_minutes": h["budget_minutes"], "started_at": h["started_at"],
            "deadline_at": utc_stamp(original_deadline),
        }
        if initial_overrides:
            draft.update(initial_overrides)
        path.write_text(render(draft), encoding="utf-8")
        git_commit_file(self.root, path, utc_stamp(started + dt.timedelta(seconds=first_offset_seconds)), "start spike")
        path.write_text(render(h), encoding="utf-8")
        git_commit_file(self.root, path, utc_stamp(stopped + dt.timedelta(seconds=final_offset_seconds)), "finish spike")
        return path

    def test_consistent_two_commit_git_history_is_accepted(self):
        self.initial_and_final_commits()
        gen_status.spike_status()

    def test_initial_header_commit_can_be_verified_as_in_progress(self):
        path = self.dir / "active.md"
        h = header()
        started = to_datetime(h["started_at"])
        original_deadline = started + dt.timedelta(minutes=int(h["budget_minutes"]))
        draft = {
            "spike": h["spike"], "platform": h["platform"], "substrate": h["substrate"],
            "budget_minutes": h["budget_minutes"], "started_at": h["started_at"],
            "deadline_at": utc_stamp(original_deadline),
        }
        path.write_text(render(draft), encoding="utf-8")
        git_commit_file(self.root, path, utc_stamp(started - dt.timedelta(seconds=30)), "commit initial spike header")
        out = gen_status.spike_status()
        report = out["reports"]["S1:macos:wry-tao"]
        self.assertEqual(report["outcome"], "in_progress")
        self.assertEqual(report["stopped_reason"], "in_progress")
        self.assertEqual(out["needs_human_decision"], [])

    def test_untracked_report_is_rejected(self):
        path = self.dir / "untracked.md"
        path.write_text(render(header()), encoding="utf-8")
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("must be tracked in Git", str(ctx.exception))

    def test_single_commit_at_end_is_rejected(self):
        path = self.dir / "s1.md"
        path.write_text(render(header()), encoding="utf-8")
        started = to_datetime(header()["started_at"])
        git_commit_file(self.root, path, utc_stamp(started - dt.timedelta(seconds=30)), "only one spike report commit")
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("at least two distinct Git commits", str(ctx.exception))

    def test_initial_header_commit_must_be_before_and_close_to_start(self):
        self.initial_and_final_commits(first_offset_seconds=-600)
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("initial header commit time must correspond to started_at", str(ctx.exception))

    def test_initial_header_commit_after_start_is_rejected(self):
        self.initial_and_final_commits(first_offset_seconds=30)
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("initial header commit must be before started_at", str(ctx.exception))

    def test_initial_commit_must_contain_the_matching_header(self):
        self.initial_and_final_commits(initial_overrides={"spike": "S99"})
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("initial Git header field 'spike' does not match", str(ctx.exception))

    def test_final_report_commit_must_be_after_and_close_to_stop(self):
        self.initial_and_final_commits(final_offset_seconds=-600)
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("final report commit must be after stopped_at", str(ctx.exception))

    def test_final_report_commit_too_far_after_stop_is_rejected(self):
        self.initial_and_final_commits(final_offset_seconds=600)
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("final report commit time must correspond to stopped_at", str(ctx.exception))

    def test_uncommitted_final_report_changes_are_rejected(self):
        path = self.initial_and_final_commits()
        path.write_text(path.read_text(encoding="utf-8") + "Uncommitted edit.\n", encoding="utf-8")
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.spike_status()
        self.assertIn("uncommitted changes", str(ctx.exception))


if __name__ == "__main__":
    unittest.main(verbosity=2)
