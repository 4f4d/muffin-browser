"""Regression tests for platform-level (build/smoke/functional/release) result records."""
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("muffin_gen_status_levels", REPO_ROOT / "tools" / "gen_status.py")
gen_status = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = gen_status
spec.loader.exec_module(gen_status)

RUN_URL = "https://github.com/example/muffin/actions/runs/123456789"


def git(root, *args, env=None):
    full_env = dict(os.environ, **(env or {}))
    return subprocess.run(
        ["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        check=True, capture_output=True, text=True, env=full_env,
    ).stdout.strip()


class LevelRecordTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.old_root = gen_status.ROOT
        gen_status.ROOT = self.root
        git(self.root, "init", "-q")
        (self.root / "seed.txt").write_text("seed\n")
        git(self.root, "add", ".")
        git(self.root, "commit", "-q", "-m", "seed")
        self.sha = git(self.root, "rev-parse", "HEAD")
        (self.root / "docs/reports/results").mkdir(parents=True)
        (self.root / "docs/reports/ci").mkdir(parents=True)
        (self.root / "docs/reports/platform").mkdir(parents=True)
        (self.root / "docs/reports/ci/smoke.log").write_text("smoke ok\n")
        (self.root / "docs/reports/platform/manual.log").write_text("manual smoke ok\n")
        (self.root / "README_FIRST.md").write_text("This is project documentation, not test evidence.\n")
        self.digest = hashlib.sha256(b"smoke ok\n").hexdigest()

    def tearDown(self):
        gen_status.ROOT = self.old_root
        self.temp.cleanup()

    def rec(self, **kw):
        r = {
            "id": "win-smoke-1", "platform": "windows", "level": "smoke", "result": "pass",
            "environment": "ci_runner", "date": "2026-11-18", "commit": self.sha,
            "artifact": "docs/reports/ci/smoke.log", "artifact_sha256": self.digest,
            "run_url": RUN_URL,
        }
        r.update(kw)
        return r

    def write(self, *records):
        (self.root / "docs/reports/results/r.json").write_text(json.dumps(list(records)))

    def bad(self, record, message):
        self.write(record)
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.level_status()
        self.assertIn(message, str(ctx.exception))

    def test_valid_ci_record_is_accepted_and_hash_is_reported(self):
        self.write(self.rec())
        entry = gen_status.level_status()["windows"]["smoke"]
        self.assertEqual(entry["result"], "pass")
        self.assertEqual(entry["artifact_sha256"], self.digest)
        self.assertEqual(entry["run_url"], RUN_URL)

    def test_valid_human_record_needs_tester_not_run_url(self):
        r = self.rec(environment="bare_metal", tester="om")
        r.pop("run_url")
        self.write(r)
        self.assertEqual(gen_status.level_status()["windows"]["smoke"]["result"], "pass")

    def test_unrelated_repository_file_is_not_platform_artifact_evidence(self):
        self.bad(self.rec(artifact="seed.txt"), "designated reports artifact directory")

    def test_directory_artifact_is_rejected(self):
        self.bad(self.rec(artifact="docs/reports/ci"), "non-empty file")

    def test_readme_is_rejected_even_when_its_hash_is_correct(self):
        data = (self.root / "README_FIRST.md").read_bytes()
        self.bad(
            self.rec(
                artifact="README_FIRST.md",
                artifact_sha256=hashlib.sha256(data).hexdigest(),
            ),
            "designated reports artifact directory",
        )

    def test_valid_human_artifact_in_platform_directory_is_accepted(self):
        data = b"manual smoke ok\n"
        r = self.rec(
            environment="bare_metal",
            artifact="docs/reports/platform/manual.log",
            artifact_sha256=hashlib.sha256(data).hexdigest(),
            tester="human-reviewer",
        )
        r.pop("run_url")
        self.write(r)
        self.assertEqual(gen_status.level_status()["windows"]["smoke"]["result"], "pass")

    def test_path_traversal_artifact_is_rejected(self):
        self.bad(self.rec(artifact="../outside.log"), "clean repository-relative path")

    def test_symlink_into_another_artifact_area_is_rejected(self):
        link = self.root / "docs/reports/ci/platform-link.log"
        try:
            link.symlink_to(self.root / "docs/reports/platform/manual.log")
        except OSError:
            self.skipTest("symlinks unavailable")
        self.bad(
            self.rec(artifact="docs/reports/ci/platform-link.log", artifact_sha256=hashlib.sha256(b"manual smoke ok\n").hexdigest()),
            "outside its designated reports artifact directory",
        )

    def test_empty_artifact_is_rejected(self):
        (self.root / "docs/reports/ci/empty.log").write_text("")
        self.bad(self.rec(artifact="docs/reports/ci/empty.log"), "non-empty file")

    def test_private_evidence_artifact_is_rejected(self):
        d = self.root / "docs/reports/m0/evidence-private"
        d.mkdir(parents=True)
        (d / "raw.txt").write_text("raw\n")
        self.bad(self.rec(artifact="docs/reports/m0/evidence-private/raw.txt"), "evidence-private")

    def test_hash_mismatch_is_rejected(self):
        self.bad(self.rec(artifact_sha256="0" * 64), "does not match")

    def test_missing_hash_is_rejected(self):
        r = self.rec()
        r.pop("artifact_sha256")
        self.bad(r, "artifact_sha256")

    def test_short_commit_is_rejected(self):
        self.bad(self.rec(commit="deadbee"), "full 40-character")

    def test_unknown_commit_is_rejected(self):
        self.bad(self.rec(commit="d" * 40), "not in this repository's history")

    def test_ci_record_without_run_url_is_rejected(self):
        r = self.rec()
        r.pop("run_url")
        self.bad(r, "run_url")

    def test_malformed_run_url_is_rejected(self):
        self.bad(self.rec(run_url="https://example.com/not-actions"), "run_url")

    def test_human_record_without_tester_is_rejected(self):
        r = self.rec(environment="bare_metal")
        r.pop("run_url")
        self.bad(r, "tester")

    def test_vm_cannot_satisfy_release(self):
        self.bad(self.rec(environment="vm", level="release", tester="om"), "release")

    def test_fail_records_need_the_same_evidence(self):
        self.bad(self.rec(result="fail", artifact_sha256="0" * 64), "does not match")

    def test_invalid_date_is_rejected(self):
        self.bad(self.rec(date="2026-13-40"), "ISO date")

    def test_duplicate_ids_are_rejected(self):
        self.write(self.rec(), self.rec(level="build"))
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.level_status()
        self.assertIn("duplicate record id", str(ctx.exception))

    def test_symlink_artifact_escaping_repo_is_rejected(self):
        with tempfile.TemporaryDirectory() as outside:
            target = pathlib.Path(outside) / "x.log"
            target.write_text("smoke ok\n")
            link = self.root / "docs/reports/ci/link.log"
            try:
                link.symlink_to(target)
            except OSError:
                self.skipTest("symlinks unavailable")
            self.bad(self.rec(artifact="docs/reports/ci/link.log"), "outside the repository")

    def test_not_run_needs_no_evidence(self):
        self.write({"id": "x", "platform": "macos", "level": "build", "result": "not_run",
                    "environment": "ci_runner", "date": "2026-11-18"})
        self.assertEqual(gen_status.level_status()["macos"]["build"]["result"], "not_run")


if __name__ == "__main__":
    unittest.main(verbosity=2)
