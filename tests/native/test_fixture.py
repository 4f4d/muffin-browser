import unittest
from urllib.request import urlopen

from fixture import browser_fixture


class FixtureTests(unittest.TestCase):
    def test_fixture_redirect_crosses_origins_and_serves_distinct_pages(self):
        with browser_fixture() as (first, second):
            self.assertNotEqual(first.origin, second.origin)
            with urlopen(first.origin + "/first", timeout=2) as response:
                self.assertIn(b"<h1>First page</h1>", response.read())
            with urlopen(first.origin + "/redirect", timeout=2) as response:
                self.assertEqual(response.geturl(), second.origin + "/second")
                self.assertIn(b"<h1>Second page</h1>", response.read())
            self.assertEqual(first.requests, [("GET", "/first"), ("GET", "/redirect")])
            self.assertEqual(second.requests, [("GET", "/second")])


if __name__ == "__main__":
    unittest.main()
