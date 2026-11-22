"""Loopback-only pages for the production shell's development tests."""

from contextlib import contextmanager
from hashlib import sha256
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from io import BytesIO
import json
from pathlib import Path
import re
import ssl
import subprocess
from threading import Event, Thread
from urllib.parse import parse_qs, urlsplit
import wave


PAGE_ONE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin fixture | First page</title>
<h1>First page</h1><p>Local navigation fixture</p>
<a href="/redirect">Go to the second page</a>
</html>
"""

PAGE_TWO = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin fixture | Second page</title>
<h1>Second page</h1><label>Fixture text<input autocomplete="off"></label>
<p>amber muffin fixture</p>
<a id="fixture-download" href="/download/alpha" download="shared.bin">Download fixture alpha</a>
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

UPLOAD_PAGE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin upload fixture</title>
<h1>Upload fixture</h1><input id="fixture-upload" type="file">
<script>
document.querySelector('#fixture-upload').addEventListener('change', async (event) => {
  const file = event.target.files[0];
  if (file) await fetch('/upload', {method: 'POST', body: await file.arrayBuffer()});
});
</script></html>
"""


def make_inline_pdf():
    content = b"BT /F1 16 Tf 24 72 Td (Muffin inline PDF fixture) Tj ET\n"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 360 120] "
        b"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Length " + str(len(content)).encode("ascii") + b" >>\nstream\n"
        + content + b"endstream",
    ]
    output = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for number, body in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{number} 0 obj\n".encode("ascii") + body + b"\nendobj\n")
    xref = len(output)
    output.extend(f"xref\n0 {len(offsets)}\n".encode("ascii"))
    output.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    output.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode("ascii")
    )
    return bytes(output)


INLINE_PDF = make_inline_pdf()

MEDIA_PAGE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin media fixture</title>
<audio id="fixture-audio" src="/media.wav" controls muted autoplay></audio>
<script>
  const audio = document.getElementById('fixture-audio');
  const tab = new URLSearchParams(location.search).get('tab');
  let sequence = 0;
  setInterval(() => {
    fetch('/media-state?' + new URLSearchParams({
      tab, time: audio.currentTime.toFixed(3), sequence: ++sequence,
    }), {cache: 'no-store'});
  }, 100);
</script></html>
"""

CAPTURE_PAGE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin capture permission fixture</title>
<h1>Capture permission fixture</h1>
<button id="camera">Request camera</button>
<button id="microphone">Request microphone</button>
<p id="result" role="status">No request made</p>
<script>
  let active = false;
  async function request(kind) {
    if (active) return;
    active = true;
    const report = document.getElementById('result');
    report.textContent = 'Waiting for a permission decision';
    let result;
    try {
      const media = await navigator.mediaDevices.getUserMedia(
        kind === 'camera' ? {video: true} : {audio: true}
      );
      for (const track of media.getTracks()) track.stop();
      result = 'stream_returned_then_stopped';
    } catch (error) {
      result = error.name;
    } finally {
      active = false;
    }
    report.textContent = result;
    await fetch('/capture-result?' + new URLSearchParams({kind, result}), {method: 'POST'});
  }
  for (const kind of ['camera', 'microphone']) {
    document.getElementById(kind).addEventListener('click', () => request(kind));
  }
</script></html>
"""

PRIVACY_PAGE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><title>Muffin storage fixture</title>
<h1>Storage fixture</h1>
<script>
(async () => {
  const params = new URLSearchParams(location.search);
  const probe = params.get('probe');
  const value = params.get('write');
  const cache = await caches.open('muffin-fixture');
  const marker = new URL('/fixture-cache-marker', location.origin);
  if (value) {
    document.cookie = 'muffin_fixture=' + value + '; Path=/; SameSite=Lax; Max-Age=600';
    localStorage.setItem('muffin-fixture', value);
    await cache.put(marker, new Response(value));
  }
  const cached = await cache.match(marker);
  const result = {
    probe,
    cookie: document.cookie.split('; ').find(value => value.startsWith('muffin_fixture='))?.split('=')[1] ?? null,
    local: localStorage.getItem('muffin-fixture'),
    cache: cached ? await cached.text() : null,
  };
  await fetch('/storage-result', {method: 'POST', body: JSON.stringify(result)});
})().catch(async (error) => {
  await fetch('/storage-result', {
    method: 'POST',
    body: JSON.stringify({probe: new URLSearchParams(location.search).get('probe'), error: error.name}),
  });
});
</script></html>
"""

with BytesIO() as media:
    with wave.open(media, "wb") as audio:
        audio.setparams((1, 2, 8000, 0, "NONE", "not compressed"))
        audio.writeframes(bytes(2 * 8000 * 30))
    MEDIA_WAV = media.getvalue()

DOWNLOAD_PAYLOADS = {
    "/download/alpha": b"Muffin fixture alpha\x00\x87\xee" * 64,
    "/download/beta": b"Muffin fixture beta\x01\x19\xff" * 48,
    "/download/redirect": b"Muffin fixture redirected\x02\x2a\xfe" * 40,
}
UNREQUESTED_DOWNLOAD = "/download/unrequested"
SLOW_DOWNLOAD = "/download/slow"
SLOW_DOWNLOAD_CHUNK = b"muffin-transfer" * 32768
RETRY_DOWNLOAD = "/download/retry.txt"
RETRY_PAYLOAD = b"Muffin recoverable download fixture\n" * 16384


class FixtureServer(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), FixtureHandler)
        self.requests = []
        self.user_agents = {}
        self.uploads = []
        self.redirect_to = ""
        self.redirect_download_to = ""
        self.release_slow_page = Event()
        self.release_slow_download = Event()
        self.break_retry_download = Event()
        self.media_states = {}
        self.capture_results = []
        self.storage_results = {}

    @property
    def origin(self):
        return f"http://127.0.0.1:{self.server_port}"


@contextmanager
def untrusted_tls_fixture(directory):
    """Serve only loopback with an ephemeral self-signed certificate."""
    certificate = Path(directory) / "untrusted-cert.pem"
    key = Path(directory) / "untrusted-key.pem"
    subprocess.run([
        "/usr/bin/openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
        "-days", "1", "-subj", "/CN=Muffin loopback fixture",
        "-keyout", str(key), "-out", str(certificate),
    ], check=True, capture_output=True, timeout=10)
    with FixtureServer() as server:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(certificate, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        worker = Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            yield server
        finally:
            server.shutdown()
            worker.join(timeout=3)


class FixtureHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.server.requests.append(("GET", self.path))
        self.server.user_agents[self.path] = self.headers.get("User-Agent", "")
        if self.path.startswith("/media-state?"):
            state = parse_qs(urlsplit(self.path).query)
            try:
                tab = state["tab"][0]
                sample = float(state["time"][0]), int(state["sequence"][0])
            except (KeyError, ValueError):
                self.send_error(400)
                return
            if sample[1] > self.server.media_states.get(tab, (0, 0))[1]:
                self.server.media_states[tab] = sample
            self.send_response(204)
            self.end_headers()
            return
        if self.path == "/media.wav":
            requested = self.headers.get("Range")
            byte_range = re.fullmatch(r"bytes=(\d+)-(\d*)", requested) if requested else None
            if byte_range:
                start = int(byte_range[1])
                end = min(int(byte_range[2] or len(MEDIA_WAV) - 1), len(MEDIA_WAV) - 1)
                if start > end:
                    self.send_error(416)
                    return
                self.send_response(206)
                self.send_header("Content-Range", f"bytes {start}-{end}/{len(MEDIA_WAV)}")
                body = MEDIA_WAV[start:end + 1]
            else:
                self.send_response(200)
                body = MEDIA_WAV
            self.send_header("Content-Type", "audio/wav")
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path == "/slow":
            self.send_slow_page()
            return
        if self.path == SLOW_DOWNLOAD:
            self.send_slow_download()
            return
        if self.path == RETRY_DOWNLOAD:
            first = self.server.requests.count(("GET", RETRY_DOWNLOAD)) == 1
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Disposition", 'attachment; filename="retry.txt"')
            self.send_header("Content-Length", str(len(RETRY_PAYLOAD) * (2 if first else 1)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            try:
                self.wfile.write(RETRY_PAYLOAD)
                self.wfile.flush()
                if first:
                    self.server.break_retry_download.wait(timeout=20)
            except (BrokenPipeError, ConnectionResetError):
                pass
            self.close_connection = True
            return
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", self.server.redirect_to)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if self.path == "/redirect-download":
            self.send_response(302)
            self.send_header("Location", self.server.redirect_download_to)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        if self.path in DOWNLOAD_PAYLOADS or self.path == UNREQUESTED_DOWNLOAD:
            body = DOWNLOAD_PAYLOADS.get(self.path, b"unrequested fixture transfer")
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Disposition", 'attachment; filename="shared.bin"')
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)
            return
        pages = {
            "/first": PAGE_ONE,
            "/second": PAGE_TWO,
            "/upload": UPLOAD_PAGE,
            "/media": MEDIA_PAGE,
            "/capture-permission": CAPTURE_PAGE,
            "/storage-fixture": PRIVACY_PAGE,
            "/attempt-download": (
                "<!doctype html><title>Automatic download fixture</title>"
                f"<script>location.assign('{UNREQUESTED_DOWNLOAD}');</script>"
            ),
        }
        page_path = self.path.split("?", 1)[0]
        if page_path in pages:
            body = pages[page_path].encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)
            return
        if page_path == "/inline-document":
            self.send_response(200)
            self.send_header("Content-Type", "application/pdf")
            self.send_header("Content-Disposition", 'inline; filename="inline-fixture.pdf"')
            self.send_header("Content-Length", str(len(INLINE_PDF)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(INLINE_PDF)
            return
        self.send_error(404)

    def do_POST(self):
        self.server.requests.append(("POST", self.path))
        if self.path.startswith("/capture-result?"):
            result = parse_qs(urlsplit(self.path).query)
            self.server.capture_results.append({
                key: result.get(key, [""])[0] for key in ("kind", "result")
            })
        if self.path == "/storage-result":
            length = int(self.headers.get("Content-Length", "0"))
            if not 0 < length <= 1024:
                self.send_error(400)
                return
            received = json.loads(self.rfile.read(length))
            self.server.storage_results[received["probe"]] = received
        if self.path == "/upload":
            length = int(self.headers.get("Content-Length", "0"))
            if length < 0 or length > 1024 * 1024:
                self.send_error(413)
                return
            body = self.rfile.read(length)
            self.server.uploads.append({"bytes": len(body), "sha256": sha256(body).hexdigest()})
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

    def send_slow_download(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Disposition", 'attachment; filename="partial.bin"')
        self.send_header("Content-Length", str(2 * len(SLOW_DOWNLOAD_CHUNK)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        try:
            self.wfile.write(SLOW_DOWNLOAD_CHUNK)
            self.wfile.flush()
            if self.server.release_slow_download.wait(timeout=20):
                self.wfile.write(SLOW_DOWNLOAD_CHUNK)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, _format, *args):
        pass


@contextmanager
def browser_fixture():
    with FixtureServer() as first, FixtureServer() as second:
        first.redirect_to = second.origin + "/second"
        first.redirect_download_to = second.origin + "/download/redirect"
        threads = [Thread(target=server.serve_forever, daemon=True) for server in (first, second)]
        for thread in threads:
            thread.start()
        try:
            yield first, second
        finally:
            first.release_slow_page.set()
            first.release_slow_download.set()
            second.release_slow_download.set()
            first.shutdown()
            second.shutdown()
            for thread in threads:
                thread.join(timeout=3)
