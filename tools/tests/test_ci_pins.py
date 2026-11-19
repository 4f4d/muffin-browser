"""Guard immutable GitHub Actions references in workflow files."""
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/status-guard.yml"
EXPECTED_CHECKOUT_SHA = "3d3c42e5aac5ba805825da76410c181273ba90b1"


class GitHubActionPinTests(unittest.TestCase):
    def test_status_guard_uses_verified_checkout_release_commit(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        references = re.findall(r"^\s*-\s+uses:\s+([^@\s]+)@([0-9a-fA-F]+)", text, re.MULTILINE)
        self.assertTrue(references, "status-guard workflow must use at least one action")
        for action, ref in references:
            with self.subTest(action=action):
                self.assertRegex(ref, r"^[0-9a-f]{40}$", "actions must use full lowercase commit SHAs")
        checkout = [ref for action, ref in references if action == "actions/checkout"]
        self.assertEqual(checkout, [EXPECTED_CHECKOUT_SHA])


if __name__ == "__main__":
    unittest.main(verbosity=2)
