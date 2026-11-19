#!/usr/bin/env python3
"""Bounded two-origin loopback fixture for human-run M0b validation."""
from __future__ import annotations

import argparse
import email.parser
import email.policy
import http.server
import json
import os
from pathlib import Path
import threading
import time
import wave
from urllib.parse import urlsplit


AUTH_COOKIE = "muffin_m0b_fixture=synthetic"
COOKIE_NAME = "muffin_m0b_fixture"
PRIVACY_COOKIE = "muffin_m0b_privacy=synthetic"
MAX_UPLOAD_BYTES = 256 * 1024
MAX_EVENTS = 64
MAX_REQUESTS = 256
LOCAL_STORAGE_KEY = "muffin-m0b-synthetic-marker"
LOCAL_STORAGE_VALUE = "synthetic"
AUTH_DOWNLOAD = (b"MUFFIN-M0B-SYNTHETIC-AUTHORIZED-DOWNLOAD\n" * 512)
MEDIA_FILES = {
    "h264-aac.mp4": "video/mp4",
    "vp9.webm": "video/webm",
    "av1.webm": "video/webm",
    "opus.ogg": "audio/ogg",
}


def make_pdf() -> bytes:
    """Build a one-page PDF from fixed objects without external resources."""
    content = b"BT /F1 18 Tf 24 60 Td (Muffin M0b PDF fixture) Tj ET\n"
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
    xref_offset = len(result)
    result.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    result.extend(b"".join(f"{offset:010d} 00000 n \n".encode() for offset in offsets[1:]))
    result.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n".encode())
    return bytes(result)


def make_wav() -> bytes:
    from io import BytesIO

    output = BytesIO()
    # Short, silent PCM WAV suitable for an audio element. No codec dependency.
    with wave.open(output, "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(8000)
        audio.writeframes(b"\0\0" * 8000)
    return output.getvalue()


PDF_BYTES = make_pdf()
WAV_BYTES = make_wav()


def html_response(body: str) -> bytes:
    return body.encode("utf-8")


def page_home(peer: str) -> bytes:
    return html_response(f"""<!doctype html><meta charset="utf-8"><title>Muffin M0b local fixture</title>
<style>#open-cross-origin-popup{{position:absolute;top:110px;left:35px}} #popup-status{{margin-top:55px}}</style>
<h1>Muffin M0b local fixture</h1>
<nav><a href="/nav/final">Navigation</a> · <a href="/auth/login">Synthetic download</a> ·
<a id="slow-download" href="/download/large">Large download</a> · <a href="/upload">Upload</a> · <a href="/media">Media</a> · <a href="/document.pdf">PDF</a> ·
<a href="/privacy">Storage isolation</a></nav>
<p><button id="open-cross-origin-popup">Open cross-origin popup</button></p><pre id="popup-status">waiting</pre>
<script>
const target = {json.dumps(peer)};
document.querySelector('#open-cross-origin-popup').onclick = () => window.open(target + '/popup', '_blank');
window.addEventListener('message', event => {{
  if (event.origin !== target || !event.data || event.data.type !== 'muffin-m0b-popup-ready') return;
  document.querySelector('#popup-status').textContent = 'cross-origin popup message received';
}});
</script>""")


def page_popup(parent: str) -> bytes:
    return html_response(f"""<!doctype html><meta charset="utf-8"><title>Muffin popup fixture</title>
<h1>Cross-origin popup fixture</h1><p id="popup-status">checking opener</p>
<button id="popup-notify-parent">Notify opener</button> <button id="popup-close">Close popup</button>
<script>
const parentOrigin = {json.dumps(parent)};
const message = {{type:'muffin-m0b-popup-ready'}};
document.querySelector('#popup-status').textContent = window.opener ? 'opener-present' : 'opener-null';
if (window.opener) window.opener.postMessage(message, parentOrigin);
document.querySelector('#popup-notify-parent').onclick = () => {{
  if (window.opener) window.opener.postMessage({{type:'muffin-m0b-popup-ready'}}, parentOrigin);
}};
document.querySelector('#popup-close').onclick = async () => {{
  await fetch('/event/popup-close', {{method:'POST',headers:{{'content-type':'application/json'}},body:'{{"event":"popup-close"}}'}});
  window.close();
}};
</script>""")


def page_auth() -> bytes:
    return html_response("""<!doctype html><meta charset="utf-8"><title>Synthetic local download gate</title>
<h1>Synthetic local download gate</h1>
<form method="post" action="/auth/session"><button id="auth-login-button" type="submit">Start local fixture session</button></form>""")


def page_auth_ready(authorized: bool) -> bytes:
    link = '<a id="auth-download-link" href="/download/authenticated">Download synthetic file</a>' if authorized else "No fixture session"
    return html_response(f"""<!doctype html><meta charset="utf-8"><title>Fixture session state</title>
<p id="auth-status">{"synthetic session ready" if authorized else "not authorized"}</p>{link}""")


def page_upload() -> bytes:
    return html_response(f"""<!doctype html><meta charset="utf-8"><title>Safe upload fixture</title>
<h1>Safe upload fixture</h1><form method="post" action="/upload" enctype="multipart/form-data">
<label>Choose synthetic text fixture <input id="upload-file" type="file" name="file" accept=".txt,text/plain" required></label>
<button id="upload-submit" type="submit">Submit fixture</button></form><p>Echo is limited to receipt status and byte count.</p>""")


def page_media() -> bytes:
    return html_response("""<!doctype html><meta charset="utf-8"><title>Local media fixture</title>
<h1>Local media fixture</h1><audio id="m0b-audio-wav" controls src="/media/audio.wav"></audio>
<video id="m0b-video-h264-aac" controls src="/media/h264-aac.mp4"></video>
<video id="m0b-video-vp9" controls src="/media/vp9.webm"></video>
<video id="m0b-video-av1" controls src="/media/av1.webm"></video>
<audio id="m0b-audio-opus" controls src="/media/opus.ogg"></audio>
<p>WAV is generated locally. Optional video files are read from --media-dir.</p>""")


def page_privacy() -> bytes:
    return html_response("""<!doctype html><meta charset="utf-8"><title>Storage isolation fixture</title>
<h1>Storage isolation fixture</h1><pre id="privacy-state"></pre><button id="privacy-write">Write synthetic state</button>
<button id="privacy-clear">Clear synthetic state</button>
<script>
const key='muffin-m0b-synthetic-marker', marker='synthetic';
function observe(){const result={localStorage:localStorage.getItem(key)===marker,cookie:document.cookie.includes('muffin_m0b_privacy=synthetic')};document.querySelector('#privacy-state').textContent=JSON.stringify(result);}
document.querySelector('#privacy-write').onclick=()=>{localStorage.setItem(key,marker);document.cookie='muffin_m0b_privacy=synthetic; Path=/; Max-Age=3600; SameSite=Strict';observe();};
document.querySelector('#privacy-clear').onclick=()=>{localStorage.removeItem(key);document.cookie='muffin_m0b_privacy=; Max-Age=0; Path=/; SameSite=Strict';observe();};
observe();
</script>""")


class FixtureState:
    def __init__(self, log_path: Path | None):
        self.lock = threading.Lock()
        self.events = 0
        self.requests = 0
        self.uploads = 0
        self.log = log_path.open("a", encoding="utf-8", buffering=1) if log_path else None

    def record(self, route: str, status: int, size: int | None = None) -> None:
        # Route is selected by this module, never taken from the request URL.
        entry = {"at": round(time.time(), 3), "route": route, "status": status}
        if size is not None:
            entry["bytes"] = size
        with self.lock:
            if self.events >= MAX_EVENTS:
                return
            self.events += 1
            if self.log:
                self.log.write(json.dumps(entry, separators=(",", ":")) + "\n")

    def count_request(self) -> bool:
        with self.lock:
            self.requests += 1
            return self.requests <= MAX_REQUESTS


class LoopbackServer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def make_handler(origin: str, peer: str, parent: str, state: FixtureState, is_peer: bool, media_dir: Path | None):
    expected_host = origin.removeprefix("http://")

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def respond(self, status: int, body: bytes, content_type: str, headers=(), route: str = "not-found"):
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            for name, value in headers:
                self.send_header(name, value)
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(body)
            state.record(route, status, len(body))
            return None

        def do_HEAD(self):
            self.do_GET()

        def streamed(self, route: str, parts, content_type: str, length: int, disposition: str | None = None):
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(length))
            self.send_header("Cache-Control", "no-store")
            if disposition:
                self.send_header("Content-Disposition", disposition)
            self.end_headers()
            sent = 0
            try:
                for part, delay in parts:
                    self.wfile.write(part)
                    self.wfile.flush()
                    sent += len(part)
                    if delay:
                        time.sleep(delay)
            except (BrokenPipeError, ConnectionError):
                pass
            state.record(route, 200, sent)

        def do_GET(self):
            if self.headers.get("Host") != expected_host:
                return self.respond(421, b"unexpected fixture authority\n", "text/plain", route="bad-authority")
            if not state.count_request():
                return self.respond(429, b"fixture request limit\n", "text/plain", route="request-limit")
            route = urlsplit(self.path).path
            if route == "/":
                return self.respond(200, page_home(peer), "text/html; charset=utf-8", route="home")
            if route == "/healthz":
                return self.respond(200, b"ok\n", "text/plain", route="health")
            if route == "/nav/redirect":
                return self.respond(302, b"", "text/plain",
                                    [("Location", peer + "/nav/final")], route="cross-origin-redirect")
            if route == "/nav/slow":
                start = b'<!doctype html><title>Slow synthetic navigation</title><p id="slow-start">Waiting</p>' + b' ' * 4096
                finish = b'<p id="slow-complete">Ready</p>'
                return self.streamed("navigation-slow", [(start, 4), (finish, 0)],
                                     "text/html; charset=utf-8", len(start) + len(finish))
            if route == "/download/large":
                block = b"Muffin validation slow download block.".ljust(65536, b".")
                return self.streamed("large-download", ((block, 0.045) for _ in range(128)),
                                     "application/octet-stream", len(block) * 128,
                                     'attachment; filename="muffin-large.bin"')
            if route == "/nav/final":
                return self.respond(200, html_response('<!doctype html><title>Navigation target</title><h1 id="result">navigation target reached</h1>'), "text/html; charset=utf-8", route="navigation-target")
            if route == "/popup" and is_peer:
                return self.respond(200, page_popup(parent), "text/html; charset=utf-8", route="cross-origin-popup")
            if route == "/auth/login":
                return self.respond(200, page_auth(), "text/html; charset=utf-8", route="auth-form")
            if route == "/auth/ready":
                authorized = AUTH_COOKIE in self.headers.get("Cookie", "")
                return self.respond(200, page_auth_ready(authorized), "text/html; charset=utf-8", route="auth-ready")
            if route == "/download/authenticated":
                if AUTH_COOKIE not in self.headers.get("Cookie", ""):
                    return self.respond(403, b"synthetic session required\n", "text/plain", route="download-denied")
                return self.respond(200, AUTH_DOWNLOAD, "application/octet-stream",
                                    [("Content-Disposition", 'attachment; filename="muffin-m0b-fixture.bin"')],
                                    route="authenticated-download")
            if route == "/upload":
                return self.respond(200, page_upload(), "text/html; charset=utf-8", route="upload-form")
            if route == "/media":
                return self.respond(200, page_media(), "text/html; charset=utf-8", route="media-page")
            if route == "/media/audio.wav":
                return self.respond(200, WAV_BYTES, "audio/wav", route="generated-audio")
            if route.startswith("/media/") and route.removeprefix("/media/") in MEDIA_FILES:
                filename = route.removeprefix("/media/")
                if media_dir is None:
                    return self.respond(404, b"optional synthetic media unavailable\n", "text/plain", route="optional-media-unavailable")
                path = (media_dir / filename)
                if path.is_symlink() or not path.is_file() or path.stat().st_size > 32 * 1024 * 1024:
                    return self.respond(404, b"optional synthetic media unavailable\n", "text/plain", route="optional-media-unavailable")
                payload = path.read_bytes()
                range_header = self.headers.get("Range")
                headers = [("Accept-Ranges", "bytes")]
                status = 200
                if range_header:
                    match = range_header.removeprefix("bytes=").split("-", 1)
                    try:
                        start = int(match[0])
                        end = min(int(match[1]) if match[1] else len(payload) - 1, len(payload) - 1)
                        if start < 0 or start >= len(payload) or end < start:
                            raise ValueError("range")
                    except ValueError:
                        return self.respond(416, b"invalid range\n", "text/plain", route="optional-media-invalid-range")
                    payload = payload[start:end + 1]
                    headers.append(("Content-Range", f"bytes {start}-{end}/{path.stat().st_size}"))
                    status = 206
                return self.respond(status, payload, MEDIA_FILES[filename], headers, route="optional-media")
            if route == "/document.pdf":
                return self.respond(200, PDF_BYTES, "application/pdf",
                                    [("Content-Disposition", 'inline; filename="muffin-m0b-fixture.pdf"')],
                                    route="generated-pdf")
            if route == "/privacy":
                return self.respond(200, page_privacy(), "text/html; charset=utf-8", route="privacy-page")
            if route == "/event/popup-close" and is_peer:
                return self.respond(405, b"method not allowed\n", "text/plain", route="popup-close-method")
            return self.respond(404, b"not found\n", "text/plain", route="not-found")

        def do_POST(self):
            if self.headers.get("Host") != expected_host:
                return self.respond(421, b"unexpected fixture authority\n", "text/plain", route="bad-authority")
            if not state.count_request():
                return self.respond(429, b"fixture request limit\n", "text/plain", route="request-limit")
            route = urlsplit(self.path).path
            try:
                size = int(self.headers.get("Content-Length", "-1"))
            except ValueError:
                size = -1
            if size < 0 or size > MAX_UPLOAD_BYTES + 4096:
                return self.respond(413, b"request too large\n", "text/plain", route="request-too-large")
            body = self.rfile.read(size)
            if route == "/auth/session":
                if size != 0:
                    return self.respond(400, b"empty synthetic session request required\n", "text/plain", route="auth-invalid")
                return self.respond(303, b"", "text/plain",
                                    [("Set-Cookie", f"{AUTH_COOKIE}; Path=/; HttpOnly; SameSite=Strict"),
                                     ("Location", "/auth/ready")],
                                    route="auth-session-created")
            if route == "/upload":
                if not self.headers.get("Content-Type", "").lower().startswith("multipart/form-data;"):
                    return self.respond(415, b"multipart form required\n", "text/plain", route="upload-invalid")
                try:
                    envelope = (b"Content-Type: " + self.headers["Content-Type"].encode("ascii") +
                                b"\r\nMIME-Version: 1.0\r\n\r\n" + body)
                    message = email.parser.BytesParser(policy=email.policy.default).parsebytes(envelope)
                    candidates = []
                    if message.is_multipart():
                        for part in message.iter_parts():
                            if part.get_content_disposition() == "form-data" and part.get_param("name", header="content-disposition") == "file":
                                payload = part.get_payload(decode=True) or b""
                                filename = part.get_filename()
                                if filename == "synthetic.txt" and len(payload) <= MAX_UPLOAD_BYTES:
                                    candidates.append(payload)
                    if len(candidates) != 1:
                        return self.respond(400, b"expected one bounded synthetic.txt file\n", "text/plain", route="upload-invalid")
                    payload_size = len(candidates[0])
                    with state.lock:
                        state.uploads += 1
                        receipt = state.uploads
                    state.record("upload-received", 200, payload_size)
                    return self.respond(200, html_response(f'<p id="upload-result">Upload received. Bytes: {payload_size}. Receipt: {receipt}.</p>'),
                                        "text/html; charset=utf-8", route="upload-result")
                except (UnicodeEncodeError, ValueError, TypeError):
                    return self.respond(400, b"invalid multipart request\n", "text/plain", route="upload-invalid")
            if route == "/event/popup-close" and is_peer:
                allowed_publisher = self.headers.get("Origin") == origin
                if allowed_publisher and body == b'{"event":"popup-close"}':
                    state.record("popup-closed", 204)
                    return self.respond(204, b"", "text/plain", route="popup-close")
                return self.respond(403, b"event denied\n", "text/plain", route="popup-close-denied")
            return self.respond(404, b"not found\n", "text/plain", route="not-found")

        def do_OPTIONS(self):
            # No permissive CORS policy is needed by this same-origin fixture.
            self.respond(405, b"method not allowed\n", "text/plain", route="options-denied")

    return Handler


def serve(ready_file: Path, log_path: Path | None = None, media_dir: Path | None = None) -> tuple[LoopbackServer, LoopbackServer]:
    state = FixtureState(log_path)
    # Bind sockets first to learn both ports, then attach handlers with exact peer origins.
    parent_server = LoopbackServer(("127.0.0.1", 0), http.server.BaseHTTPRequestHandler)
    peer_server = LoopbackServer(("127.0.0.1", 0), http.server.BaseHTTPRequestHandler)
    parent_origin = f"http://127.0.0.1:{parent_server.server_port}"
    peer_origin = f"http://127.0.0.1:{peer_server.server_port}"
    parent_server.RequestHandlerClass = make_handler(parent_origin, peer_origin, parent_origin, state, False, media_dir)
    peer_server.RequestHandlerClass = make_handler(peer_origin, parent_origin, parent_origin, state, True, media_dir)
    for server in (parent_server, peer_server):
        threading.Thread(target=server.serve_forever, daemon=True).start()
    ready_file.parent.mkdir(parents=True, exist_ok=True)
    ready_file.write_text(json.dumps({"origin": parent_origin, "peer": peer_origin}, sort_keys=True) + "\n", encoding="utf-8")
    return parent_server, peer_server


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ready-file", required=True, type=Path)
    parser.add_argument("--log-file", type=Path)
    parser.add_argument("--media-dir", type=Path)
    parser.add_argument("--max-seconds", type=int, default=900)
    args = parser.parse_args()
    if not 1 <= args.max_seconds <= 3600:
        parser.error("--max-seconds must be between 1 and 3600")
    media_dir = args.media_dir.resolve() if args.media_dir else None
    if media_dir and not media_dir.is_dir():
        parser.error("--media-dir must name a directory")
    servers = serve(args.ready_file, args.log_file, media_dir)
    print(args.ready_file.read_text(encoding="utf-8").strip(), flush=True)
    try:
        time.sleep(args.max_seconds)
    except KeyboardInterrupt:
        pass
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
