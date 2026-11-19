import unittest
from dataclasses import replace

from policy_fixture import BrowserFacts, CHROME, permits_command, permits_content_navigation, permits_resource


class PolicyCases(unittest.TestCase):
    def setUp(self):
        self.active = BrowserFacts("chrome-view", 4, CHROME, True)
        self.valid = {"version": 1, "command": "FixtureStatus"}

    def test_positive_chrome_main_frame(self):
        self.assertTrue(permits_command(self.active, self.active, self.valid))

    def test_remote_view_and_subframe_declined(self):
        remote = replace(self.active, view="content-view")
        subframe = replace(self.active, main_frame=False)
        self.assertFalse(permits_command(self.active, remote, self.valid))
        self.assertFalse(permits_command(self.active, subframe, self.valid))

    def test_prior_document_epoch_declined_after_commit_and_recovery(self):
        for generation in (5, 6):
            current = replace(self.active, epoch=generation)
            self.assertFalse(permits_command(current, self.active, self.valid))
            self.assertTrue(permits_command(current, current, self.valid))

    def test_command_shape_and_origin(self):
        foreign = replace(self.active, committed_document="http://127.0.0.1:30001/")
        self.assertFalse(permits_command(self.active, foreign, self.valid))
        for bad in [None, {"command": "FixtureStatus"}, {"version": True, "command": "FixtureStatus"},
                    {**self.valid, "frame": "top"}, {**self.valid, "command": "OtherFixtureCommand"}]:
            with self.subTest(bad=bad):
                self.assertFalse(permits_command(self.active, self.active, bad))

    def test_exact_assets(self):
        self.assertTrue(permits_resource("GET", CHROME))
        for method, url in (("POST", CHROME), ("GET", CHROME + "?v=2"),
                            ("GET", "muffin-chrome://fixture/missing"), ("GET", "file:///fixture.txt")):
            self.assertFalse(permits_resource(method, url))

    def test_redirect_destinations_rechecked(self):
        allowed = frozenset({"127.0.0.1:30001", "127.0.0.1:30002"})
        for url in ("http://127.0.0.1:30001/start", "http://127.0.0.1:30002/return"):
            self.assertTrue(permits_content_navigation(url, allowed))
        for url in ("http://127.0.0.1:30003/return", "file:///fixture.txt",
                    "muffin-chrome://fixture/index.html", "muffin-fixture-external:deny"):
            self.assertFalse(permits_content_navigation(url, allowed))


if __name__ == "__main__":
    unittest.main()
