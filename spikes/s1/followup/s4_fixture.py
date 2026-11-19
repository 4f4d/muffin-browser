#!/usr/bin/env python3
"""Bounded local S4 download, storage, and PDF fixture launcher."""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import threading
import time
from http.cookies import SimpleCookie

CHUNK = b"MUFFIN-S4-SYNTHETIC-CONTENT\n"
PAYLOAD = (CHUNK * 32768)[:768 * 1024]


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


class Handler(http.server.BaseHTTPRequestHandler):
    log_file = None
    log_lock = threading.Lock()
    event_count = 0
    request_count = 0
    expected_host = None

    def log_message(self, *_):
        pass

    def send_body(self, code, body, content_type="text/html; charset=utf-8", headers=()):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Cache-Control", "no-store")
        for name, value in headers:
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if self.headers.get("Host") != self.expected_host:
            return self.send_body(421, b"unexpected fixture authority\n", "text/plain")
        if self.path != "/event":
            return self.send_body(404, b"not found", "text/plain")
        try:
            length = int(self.headers.get("Content-Length", "-1"))
            if not 0 <= length <= 1024:
                raise ValueError("length")
            event = json.loads(self.rfile.read(length))
            if not isinstance(event, dict) or event.get("kind") not in {"storage_observation", "pdf_observation"}:
                raise ValueError("shape")
            with self.log_lock:
                if type(self).event_count >= 12:
                    raise ValueError("event cap")
                type(self).event_count += 1
                self.log_file.write(json.dumps({"at": round(time.time(), 3), "event": event}, separators=(",", ":")) + "\n")
            self.send_body(204, b"", "text/plain")
        except (ValueError, json.JSONDecodeError):
            self.send_body(400, b"bad request", "text/plain")

    def do_GET(self):
        if self.headers.get("Host") != self.expected_host:
            return self.send_body(421, b"unexpected fixture authority\n", "text/plain")
        with self.log_lock:
            type(self).request_count += 1
            if type(self).request_count > 20:
                return self.send_body(429, b"fixture request cap\n", "text/plain")
        if self.path.startswith("/download?"):
            copy = self.path.split("copy=", 1)[-1]
            marker = SimpleCookie(self.headers.get("Cookie", "")).get("muffin_s4")
            authorized = marker is not None and marker.value == "synthetic"
            record = {"at": round(time.time(), 3), "event": {"kind": "download_request", "copy": copy[:20],
                      "authorized": authorized, "bytes": len(PAYLOAD) if authorized else 0}}
            with self.log_lock:
                self.log_file.write(json.dumps(record, separators=(",", ":")) + "\n")
            if copy not in {"1", "2", "private"} or not authorized:
                return self.send_body(403, b"synthetic authorization required\n", "text/plain")
            return self.send_body(200, PAYLOAD, "application/octet-stream",
                                  [("Content-Disposition", 'attachment; filename="muffin-s4.bin"')])
        if self.path.startswith("/fixture.pdf"):
            with self.log_lock:
                self.log_file.write(json.dumps({"at": round(time.time(), 3), "event": {"kind": "pdf_response", "bytes": len(PDF)}}) + "\n")
            return self.send_body(200, PDF, "application/pdf",
                                  [("Content-Disposition", 'inline; filename="muffin-s4.pdf"')])
        if self.path.startswith("/denied"):
            with self.log_lock:
                self.log_file.write(json.dumps({"at": round(time.time(), 3), "event": {"kind": "denied_attachment_response"}}) + "\n")
            return self.send_body(200, b"DENIED-SYNTHETIC-ATTACHMENT\n", "application/octet-stream",
                                  [("Content-Disposition", 'attachment; filename="blocked.bin"')])
        if self.path.startswith("/") and self.path.split("?", 1)[0] == "/":
            profile = "private" if "profile=private" in self.path else "normal"
            page = PAGE.replace(b"__PROFILE__", profile.encode())
            return self.send_body(200, page)
        self.send_body(404, b"not found\n", "text/plain")


PAGE = b"""<!doctype html><meta charset="utf-8"><title>S4 local feature fixture</title>
<h1>S4 local feature fixture</h1><p>Only synthetic loopback resources.</p><pre id="state"></pre>
<script>
(async()=>{
 const profile='__PROFILE__';
const state={profile,bridge:typeof window.ipc,localStoragePresent:localStorage.getItem('muffin-s4-marker')==='S4-SYNTHETIC-MARKER',cookiePresent:document.cookie.includes('muffin_s4=synthetic')};
 document.querySelector('#state').textContent=JSON.stringify(state);
 await fetch('/event',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({kind:'storage_observation',data:state})});
})();
</script>"""

def make_pdf():
    content = b"BT /F1 18 Tf 24 60 Td (Muffin S4 PDF fixture) Tj ET\n"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 120] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        b"<< /Length " + str(len(content)).encode() + b" >>\nstream\n" + content + b"endstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    result = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, obj in enumerate(objects, 1):
        offsets.append(len(result))
        result.extend(f"{index} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(result)
    result.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    result.extend(b"".join(f"{offset:010d} 00000 n \n".encode() for offset in offsets[1:]))
    result.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(result)


PDF = make_pdf()


def marks(path):
    if os.name == "nt":
        return {"zone_identifier": Path(f"{path}:Zone.Identifier").is_file()}
    if Path("/usr/bin/xattr").is_file():
        result = subprocess.run(["/usr/bin/xattr", "-p", "com.apple.quarantine", path],
                                capture_output=True, timeout=3, check=False)
        return {"apple_quarantine": result.returncode == 0}
    if hasattr(os, "listxattr"):
        return {"xdg_origin_url": "user.xdg.origin.url" in os.listxattr(path)}
    return {}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--work-dir", required=True)
    parser.add_argument("--timeout-seconds", type=int, default=80)
    args = parser.parse_args()
    if not 5 <= args.timeout_seconds <= 80:
        parser.error("--timeout-seconds must be between 5 and 80")
    work = Path(args.work_dir).resolve()
    work.mkdir(parents=True, exist_ok=True)
    downloads = work / "downloads"
    downloads.mkdir()
    Handler.log_file = (work / "content-events.jsonl").open("w", encoding="utf-8", buffering=1)
    server = Server(("127.0.0.1", 0), Handler)
    Handler.expected_host = f"127.0.0.1:{server.server_port}"
    Handler.log_file.write(json.dumps({"environment": "loopback synthetic only", "payload_bytes": len(PAYLOAD),
                                       "expected_sha256": hashlib.sha256(PAYLOAD).hexdigest()}) + "\n")
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{server.server_port}"
    metadata = {"base_url": base, "native_log": str(work / "native.log"),
                "content_log": str(work / "content-events.jsonl"), "downloads": str(downloads)}
    print(json.dumps(metadata, sort_keys=True), flush=True)
    binary = str(Path(args.binary).resolve())
    env = os.environ.copy()
    env["MUFFIN_S4_DOWNLOAD_DIR"] = str(downloads)
    code = None
    timed_out = False
    try:
        with open(metadata["native_log"], "w", encoding="utf-8") as native:
            child = subprocess.Popen([binary, base, str(work)], stdout=native, stderr=subprocess.STDOUT, env=env)
            try:
                code = child.wait(timeout=args.timeout_seconds)
            except subprocess.TimeoutExpired:
                timed_out = True
                child.terminate()
                try:
                    child.wait(timeout=4)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
    finally:
        server.shutdown()
        server.server_close()
        Handler.log_file.close()
    results = []
    for path in sorted(downloads.glob("*")):
        if path.is_file():
            payload = path.read_bytes()
            results.append({"name": path.name, "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest(),
                            "matches_expected": payload == PAYLOAD, **marks(path)})
    native_log = Path(metadata["native_log"]).read_text(encoding="utf-8", errors="replace")
    core_observed = (len(results) == 2 and all(item["matches_expected"] for item in results)
                     and "private_storage_isolated=true" in native_log
                     and "download_start allowed=false reason=fixture-denied" in native_log)
    summary = {"exit_code": code, "timeout": timed_out, "core_observed": core_observed,
               "pdf_navigation_finished": "pdf_page_load_finished=true" in native_log,
               "download_results": results}
    (work / "launcher-summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, sort_keys=True), flush=True)
    if timed_out:
        return 124
    if code != 0:
        return code if code >= 0 else 128 - code
    return 0 if core_observed else 3


if __name__ == "__main__":
    raise SystemExit(main())
