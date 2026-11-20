"""Loopback-only pages for the production shell's development tests."""

from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Event, Thread


PAGE_ONE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin fixture | First page</title>
<h1>First page</h1><p>Local navigation fixture</p>
<a href="/redirect">Go to the second page</a>
</html>
"""

PAGE_TWO = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin fixture | Second page</title>
<h1>Second page</h1><label>Fixture text<input autocomplete="off"></label>
<script>
  // These functions are page-owned lookalikes. The native host must never
  // consume a remote page's queue or give it access to trusted chrome.
  let stateReceived = false;
  window.muffinChrome = {
    take() {
      fetch('/unexpected-native-read', {method: 'POST'});
      return {version: 1, action: 'open_home'};
    },
    render() {
      stateReceived = true;
    },
  };
  setTimeout(() => {
    window.open('/unsolicited-popup', '_blank');
    fetch('/remote-page-ready?chrome_state=' + String(stateReceived), {method: 'POST'});
  }, 600);
</script>
</html>
"""


class FixtureServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), FixtureHandler)
        self.requests = []
        self.user_agents = {}
        self.redirect_to = ""
        self.release_slow_page = Event()

    @property
    def origin(self):
        return f"http://127.0.0.1:{self.server_port}"


class FixtureHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.server.requests.append(("GET", self.path))
        self.server.user_agents[self.path] = self.headers.get("User-Agent", "")
        if self.path == "/slow":
            self.send_slow_page()
            return
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", self.server.redirect_to)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        pages = {"/first": PAGE_ONE, "/second": PAGE_TWO}
        if self.path in pages:
            body = pages[self.path].encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_error(404)

    def do_POST(self):
        self.server.requests.append(("POST", self.path))
        self.send_response(204)
        self.end_headers()

    def send_slow_page(self):
        prefix = b"<!doctype html><title>Muffin fixture | Slow page</title>"
        suffix = b"<p>Finished slow page</p>"
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(prefix) + len(suffix)))
        self.end_headers()
        try:
            self.wfile.write(prefix)
            self.wfile.flush()
            if self.server.release_slow_page.wait(timeout=10):
                self.wfile.write(suffix)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, _format, *args):
        pass


@contextmanager
def browser_fixture():
    with FixtureServer() as first, FixtureServer() as second:
        first.redirect_to = second.origin + "/second"
        threads = [Thread(target=server.serve_forever, daemon=True) for server in (first, second)]
        for thread in threads:
            thread.start()
        try:
            yield first, second
        finally:
            first.release_slow_page.set()
            first.shutdown()
            second.shutdown()
            for thread in threads:
                thread.join(timeout=3)
