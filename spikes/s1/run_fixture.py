"""Run a native S1 executable against a loopback-only deterministic fixture."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import subprocess
from threading import Thread
from urllib.parse import urlsplit


class Fixture(BaseHTTPRequestHandler):
    def do_GET(self):
        path = urlsplit(self.path).path
        if path not in ("/start", "/next"):
            self.send_error(404)
            return
        title = "M0 content" if path == "/start" else "M0 next"
        page = f"""<!doctype html><meta charset=utf-8><title>{title}</title>
<style>body {{background:#047c73;color:white;font:24px system-ui;margin:16px}}</style>
<h1>{title}</h1><label>Fixture input <input aria-label="Fixture input"></label>""".encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Security-Policy", "default-src 'none'; style-src 'unsafe-inline'")
        self.send_header("Content-Length", str(len(page)))
        self.end_headers()
        self.wfile.write(page)

    def log_message(self, message, *args):
        print("fixture_http=" + message % args, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--hold", type=int, default=0)
    args = parser.parse_args()
    with ThreadingHTTPServer(("127.0.0.1", 0), Fixture) as server:
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = os.environ.copy()
        env["MUFFIN_FIXTURE_PORT"] = str(server.server_port)
        env["MUFFIN_S1_HOLD"] = str(args.hold)
        try:
            result = subprocess.run([args.binary.resolve()], env=env, timeout=175 + args.hold)
        finally:
            server.shutdown()
            thread.join()
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
