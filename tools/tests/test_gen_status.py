"""Regression tests for M0b evidence and environment validation."""
import copy
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "tools" / "gen_status.py"
spec = importlib.util.spec_from_file_location("muffin_gen_status", MODULE_PATH)
gen_status = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = gen_status
spec.loader.exec_module(gen_status)


class M0BValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temp.name)
        self.old_root = gen_status.ROOT
        gen_status.ROOT = self.root
        (self.root / "docs/reports/m0/evidence").mkdir(parents=True)
        (self.root / "docs/reports/m0/evidence/PK-01.txt").write_text(
            "redacted test evidence\n", encoding="utf-8"
        )
        (self.root / "docs/reports/m0/evidence-private").mkdir(parents=True)
        (self.root / "docs/reports/m0/evidence-private/raw.txt").write_text(
            "private raw evidence\n", encoding="utf-8"
        )
        subprocess.run(["git", "-C", str(self.root), "init", "-q"], check=True)
        subprocess.run(["git", "-C", str(self.root), "add", "docs/reports/m0"], check=True)
        env = dict(os.environ, GIT_AUTHOR_DATE="2026-11-18T10:00:00+00:00", GIT_COMMITTER_DATE="2026-11-18T10:00:00+00:00")
        subprocess.run(
            ["git", "-C", str(self.root), "-c", "user.name=test", "-c", "user.email=test@example.com",
             "commit", "-q", "-m", "seed M0b test evidence"],
            check=True, env=env, capture_output=True,
        )
        self.app_commit = subprocess.run(
            ["git", "-C", str(self.root), "rev-parse", "HEAD"],
            check=True, capture_output=True, text=True,
        ).stdout.strip()
        self.catalog_path = self.root / "docs/reports/m0/results.json"
        self.test = {
            "id": "PK-01",
            "title": "Passkey create and sign-in",
            "vm_valid": False,
            "platforms": ["macos"],
            "results": [],
        }
        self.catalog = {"schema_version": 1, "tests": [self.test]}

    def tearDown(self):
        gen_status.ROOT = self.old_root
        self.temp.cleanup()

    def write_catalog(self, test=None):
        data = copy.deepcopy(self.catalog)
        if test is not None:
            data["tests"] = [copy.deepcopy(test)]
        self.catalog_path.write_text(json.dumps(data), encoding="utf-8")

    def valid_record(self, **overrides):
        record = {
            "platform": "macos",
            "environment": "bare_metal",
            "result": "pass",
            "date": "2026-11-18",
            "tester": "human-reviewer",
            "app_commit": self.app_commit,
            "binary_sha256": "b" * 64,
            "host_architecture": "arm64",
            "binary_architecture": "arm64",
            "os_version": "macOS 15.8.1 (24H2)",
            "engine_version": "WKWebView / WebKit 620.1.16",
            "evidence": "docs/reports/m0/evidence/PK-01.txt",
            "notes": "Passkey creation and sign-in completed.",
        }
        record.update(overrides)
        return record

    def assert_invalid(self, record, expected_message):
        test = copy.deepcopy(self.test)
        test["results"] = [record]
        self.write_catalog(test)
        with self.assertRaises(gen_status.Invalid) as ctx:
            gen_status.m0b_status()
        self.assertIn(expected_message, str(ctx.exception))

    def test_exact_ci_bypass_pk01_pass_without_evidence_is_rejected(self):
        # Regression for the v2 bypass: vm_valid=false, result=pass,
        # environment=ci_runner, and no evidence path.
        record = self.valid_record(environment="ci_runner")
        record.pop("evidence")
        self.assert_invalid(record, "bare-metal-only test rejects environment 'ci_runner'")

    def test_bare_metal_only_pk01_rejects_vm_and_ci_for_every_outcome(self):
        for environment in ("vm", "ci_runner"):
            for result in ("pass", "fail", "partial", "inconclusive", "not_run"):
                with self.subTest(environment=environment, result=result):
                    record = self.valid_record(environment=environment, result=result)
                    self.assert_invalid(record, "bare-metal-only test rejects environment")

    def test_all_evidence_bearing_outcomes_require_evidence(self):
        for result in ("pass", "fail", "partial", "inconclusive"):
            with self.subTest(result=result):
                record = self.valid_record(result=result)
                record.pop("evidence")
                self.assert_invalid(record, f"{result} result is missing required 'evidence'")

    def test_nonexistent_evidence_is_rejected(self):
        self.assert_invalid(
            self.valid_record(evidence="docs/reports/m0/evidence/missing.png"),
            "evidence file does not exist",
        )

    def test_evidence_inside_private_directory_is_rejected(self):
        self.assert_invalid(
            self.valid_record(evidence="docs/reports/m0/evidence-private/raw.txt"),
            "must not be inside evidence-private/",
        )

    def test_evidence_path_traversal_is_rejected(self):
        self.assert_invalid(self.valid_record(evidence="../outside.txt"), "clean repository-relative")

    def test_symlink_resolving_outside_repository_is_rejected(self):
        outside = self.root.parent / f"{self.root.name}-outside-evidence.txt"
        outside.write_text("outside repo\n", encoding="utf-8")
        try:
            link = self.root / "docs/reports/m0/evidence/linked.txt"
            try:
                link.symlink_to(outside)
            except (OSError, NotImplementedError):
                self.skipTest("symlink creation is unavailable on this host")
            self.assert_invalid(
                self.valid_record(evidence="docs/reports/m0/evidence/linked.txt"),
                "resolves outside the repository",
            )
        finally:
            outside.unlink(missing_ok=True)

    def test_each_build_identity_and_execution_field_is_required(self):
        required_fields = (
            "app_commit",
            "binary_sha256",
            "host_architecture",
            "binary_architecture",
            "os_version",
            "engine_version",
            "tester",
            "date",
            "evidence",
        )
        for field in required_fields:
            with self.subTest(field=field):
                record = self.valid_record()
                record.pop(field)
                self.assert_invalid(record, f"pass result is missing required '{field}'")

    def test_empty_evidence_file_is_rejected(self):
        empty = self.root / "docs/reports/m0/evidence/empty.txt"
        empty.write_bytes(b"")
        self.assert_invalid(
            self.valid_record(evidence="docs/reports/m0/evidence/empty.txt"),
            "non-empty file",
        )

    def test_symlink_into_private_evidence_is_rejected(self):
        link = self.root / "docs/reports/m0/evidence/private-link.txt"
        try:
            link.symlink_to(self.root / "docs/reports/m0/evidence-private/raw.txt")
        except (OSError, NotImplementedError):
            self.skipTest("symlink creation is unavailable on this host")
        self.assert_invalid(
            self.valid_record(evidence="docs/reports/m0/evidence/private-link.txt"),
            "resolves inside evidence-private/",
        )

    def test_binary_hash_must_be_full_sha256(self):
        self.assert_invalid(self.valid_record(binary_sha256="b" * 40), "64 lowercase hexadecimal")

    def test_application_commit_must_be_full_git_sha(self):
        self.assert_invalid(self.valid_record(app_commit="a" * 12), "full 40-character lowercase Git SHA")

    def test_well_formed_but_nonexistent_application_commit_is_rejected(self):
        self.assert_invalid(
            self.valid_record(app_commit="a" * 40),
            "is not present in this repository's Git object database",
        )

    def test_valid_application_commit_must_be_a_real_repository_commit(self):
        test = copy.deepcopy(self.test)
        test["results"] = [self.valid_record(app_commit=self.app_commit)]
        self.write_catalog(test)
        self.assertEqual(gen_status.m0b_status()["PK-01"]["counts"], {"pass": 1})

    def test_date_must_be_a_real_iso_date(self):
        self.assert_invalid(self.valid_record(date="2026-02-31"), "real ISO date")

    def test_placeholder_versions_are_rejected(self):
        self.assert_invalid(
            self.valid_record(engine_version="REPLACE_WITH_EXACT_WEBVIEW_VERSION"),
            "must contain a real, non-placeholder value",
        )

    def test_generic_engine_and_os_labels_are_not_exact_versions(self):
        self.assert_invalid(self.valid_record(engine_version="WebView2 Runtime"), "exact numeric version")
        self.assert_invalid(self.valid_record(os_version="macOS"), "exact numeric version")

    def test_all_valid_evidence_bearing_outcomes_are_accepted(self):
        for result in ("pass", "fail", "partial", "inconclusive"):
            with self.subTest(result=result):
                test = copy.deepcopy(self.test)
                test["results"] = [self.valid_record(result=result)]
                self.write_catalog(test)
                summary = gen_status.m0b_status()
                self.assertEqual(summary["PK-01"]["recorded"], 1)
                self.assertEqual(summary["PK-01"]["counts"], {result: 1})

    def test_valid_manual_bare_metal_pass_is_accepted(self):
        test = copy.deepcopy(self.test)
        test["results"] = [self.valid_record()]
        self.write_catalog(test)
        self.assertEqual(
            gen_status.m0b_status()["PK-01"]["counts"],
            {"pass": 1},
        )

    def test_not_run_can_remain_unfilled_without_evidence(self):
        test = copy.deepcopy(self.test)
        test["results"] = [{
            "platform": "macos",
            "environment": "bare_metal",
            "result": "not_run",
        }]
        self.write_catalog(test)
        self.assertEqual(
            gen_status.m0b_status()["PK-01"]["counts"],
            {"not_run": 1},
        )

    def test_valid_vm_result_is_accepted_for_vm_valid_test(self):
        test = copy.deepcopy(self.test)
        test["id"] = "SI-01"
        test["vm_valid"] = True
        test["results"] = [self.valid_record(environment="vm")]
        self.write_catalog(test)
        self.assertEqual(
            gen_status.m0b_status()["SI-01"]["counts"],
            {"pass": 1},
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
