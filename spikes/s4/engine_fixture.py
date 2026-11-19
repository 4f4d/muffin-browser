"""Compare a native WebKit resource rule with an unfiltered control on loopback."""
import argparse
import hashlib
import http.server
import importlib.util
import json
import subprocess
import threading
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "s1/followup/s4_fixture.py"
SPEC = importlib.util.spec_from_file_location("s4_fixture", SOURCE)
S4 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(S4)

CONTROL = b'<svg width="80" height="40" xmlns="http://www.w3.org/2000/svg"><rect width="80" height="40" fill="#315888"/></svg>'
PAGE = b'<!doctype html><title>S4 request rule</title><img id="blocked" src="/blocked.svg"><img id="control" src="/control.svg">'


def run(binary, mode, output):
    requests = []
    responses = {
        "/images": ("text/html", PAGE),
        "/blocked.svg": ("image/svg+xml", CONTROL),
        "/control.svg": ("image/svg+xml", CONTROL),
        "/sample.pdf": ("application/pdf", S4.PDF),
    }

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            if self.path not in responses:
                self.send_error(404)
                return
            requests.append(self.path)
            mime, body = responses[self.path]
            self.send_response(200)
            self.send_header("Content-Type", mime)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            with (output / f"{mode}.log").open("w", encoding="utf-8") as log:
                result = subprocess.run(
                    [binary, f"http://127.0.0.1:{server.server_port}", mode, output],
                    stdout=log, stderr=subprocess.STDOUT, timeout=22, check=False,
                )
        finally:
            server.shutdown()
    return {"mode": mode, "exit_code": result.returncode, "requests": requests}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    output = args.output.resolve()
    rows = [run(args.binary.resolve(), mode, output) for mode in ("control", "filtered")]
    png = output / "pdf-view.png"
    if png.is_file():
        rows[1]["png_sha256"] = hashlib.sha256(png.read_bytes()).hexdigest()
    (output / "requests.json").write_text(json.dumps(rows, indent=2) + "\n")
    assert all(row["exit_code"] == 0 and "/control.svg" in row["requests"] for row in rows)
    assert "/blocked.svg" in rows[0]["requests"] and "/blocked.svg" not in rows[1]["requests"]


if __name__ == "__main__":
    main()
