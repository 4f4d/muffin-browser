"""HTTP semantics and evidence-redaction checks for the local M0b fixture."""
import http.cookiejar
import json
import tempfile
import unittest
import urllib.error
import urllib.request
import http.server
from pathlib import Path

import server


class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        self.ready = root / "ready.json"
        self.log = root / "events.jsonl"
        self.state = server.FixtureState(self.log)
        self.parent_server = server.LoopbackServer(("127.0.0.1", 0), http.server.BaseHTTPRequestHandler)
        self.peer_server = server.LoopbackServer(("127.0.0.1", 0), http.server.BaseHTTPRequestHandler)
        self.origin = f"http://127.0.0.1:{self.parent_server.server_port}"
        self.peer = f"http://127.0.0.1:{self.peer_server.server_port}"
        self.parent_server.RequestHandlerClass = server.make_handler(
            self.origin, self.peer, self.origin, self.state, False, None
        )
        self.peer_server.RequestHandlerClass = server.make_handler(
            self.peer, self.origin, self.origin, self.state, True, None
        )
        import threading
        threading.Thread(target=self.parent_server.serve_forever, daemon=True).start()
        threading.Thread(target=self.peer_server.serve_forever, daemon=True).start()

    def tearDown(self):
        self.parent_server.shutdown()
        self.peer_server.shutdown()
        self.parent_server.server_close()
        self.peer_server.server_close()
        if self.state.log:
            self.state.log.close()
        self.temp.cleanup()

    def get(self, url, opener=None, headers=None):
        request = urllib.request.Request(url, headers=headers or {})
        return (opener or urllib.request.build_opener()).open(request, timeout=3)

    def test_home_navigation_and_host_allowlist(self):
        body = self.get(self.origin + "/").read().decode()
        self.assertIn("/nav/final", body)
        self.assertIn(self.peer, body)
        self.assertIn("target + '/popup'", body)
        target = self.get(self.origin + "/nav/final").read().decode()
        self.assertIn("navigation target reached", target)
        with self.assertRaises(urllib.error.HTTPError) as failure:
            self.get(self.origin + "/", headers={"Host": "127.0.0.1:1"})
        self.assertEqual(failure.exception.code, 421)

    def test_popup_is_cross_origin_and_message_origin_is_exact(self):
        page = self.get(self.peer + "/popup").read().decode()
        self.assertIn(self.origin, page)
        self.assertIn("postMessage(message, parentOrigin)", page)
        self.assertIn("event.origin !== target", self.get(self.origin + "/").read().decode())
        self.assertIn('id="popup-notify-parent"', page)
        self.assertIn('id="popup-close"', page)
        payload = b'{"event":"popup-close"}'
        request = urllib.request.Request(
            self.peer + "/event/popup-close", data=payload, method="POST",
            headers={"Content-Type": "application/json", "Origin": self.peer},
        )
        self.assertEqual(urllib.request.urlopen(request, timeout=3).status, 204)
        request = urllib.request.Request(
            self.peer + "/event/popup-close", data=payload, method="POST",
            headers={"Content-Type": "application/json", "Origin": "http://127.0.0.1:1"},
        )
        with self.assertRaises(urllib.error.HTTPError) as failure:
            urllib.request.urlopen(request, timeout=3)
        self.assertEqual(failure.exception.code, 403)

    def test_synthetic_auth_cookie_gates_download(self):
        with self.assertRaises(urllib.error.HTTPError) as failure:
            self.get(self.origin + "/download/authenticated")
        self.assertEqual(failure.exception.code, 403)
        jar = http.cookiejar.CookieJar()
        opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
        request = urllib.request.Request(self.origin + "/auth/session", data=b"", method="POST")
        ready = opener.open(request, timeout=3)
        self.assertEqual(ready.status, 200)  # follows the bounded 303 to /auth/ready
        self.assertIn("synthetic session ready", ready.read().decode())
        download = opener.open(self.origin + "/download/authenticated", timeout=3)
        payload = download.read()
        self.assertEqual(download.status, 200)
        self.assertEqual(download.headers["Content-Type"], "application/octet-stream")
        self.assertIn("attachment;", download.headers["Content-Disposition"])
        self.assertEqual(payload, server.AUTH_DOWNLOAD)
        self.assertEqual(jar._cookies["127.0.0.1"]["/"][server.COOKIE_NAME].value, "synthetic")

    def test_upload_is_bounded_and_never_echoes_file_bytes(self):
        source = b"PRIVATE-SYNTHETIC-UPLOAD-CONTENT-7e9c"
        boundary = "muffin-fixture-boundary"
        body = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"synthetic.txt\"\r\n"
                "Content-Type: text/plain\r\n\r\n").encode() + source + f"\r\n--{boundary}--\r\n".encode()
        request = urllib.request.Request(
            self.origin + "/upload", data=body,
            headers={"Content-Type": f"multipart/form-data; boundary={boundary}"}, method="POST",
        )
        response = urllib.request.urlopen(request, timeout=3)
        body = response.read()
        self.assertEqual(response.status, 200)
        self.assertIn(b"Upload received", body)
        self.assertIn(str(len(source)).encode(), body)
        self.assertNotIn(source, body)
        oversized = urllib.request.Request(
            self.origin + "/upload", data=b"x" * (server.MAX_UPLOAD_BYTES + 5000),
            headers={"Content-Type": "multipart/form-data; boundary=synthetic"}, method="POST",
        )
        with self.assertRaises(urllib.error.HTTPError) as failure:
            urllib.request.urlopen(oversized, timeout=3)
        self.assertEqual(failure.exception.code, 413)
        evidence = self.log.read_text()
        self.assertNotIn(source.decode(), evidence)
        self.assertNotIn("PRIVATE-SYNTHETIC", evidence)
        self.assertIn('"route":"upload-received"', evidence)

    def test_generated_pdf_wav_and_storage_selectors(self):
        response = self.get(self.origin + "/document.pdf")
        pdf = response.read()
        self.assertEqual(response.headers["Content-Type"], "application/pdf")
        self.assertEqual(response.headers["Content-Disposition"].split(";")[0], "inline")
        self.assertTrue(pdf.startswith(b"%PDF-1.4"))
        xref = int(pdf.split(b"startxref\n", 1)[1].splitlines()[0])
        self.assertEqual(pdf[xref:xref + 5], b"xref\n")
        wav_response = self.get(self.origin + "/media/audio.wav")
        self.assertEqual(wav_response.headers["Content-Type"], "audio/wav")
        wav = wav_response.read()
        self.assertTrue(wav.startswith(b"RIFF") and b"WAVE" in wav[:16])
        privacy = self.get(self.origin + "/privacy").read().decode()
        for selector in ("privacy-state", "privacy-write", "privacy-clear"):
            self.assertIn(f'id="{selector}"', privacy)
        self.assertIn(server.LOCAL_STORAGE_KEY, privacy)
        self.assertIn(server.LOCAL_STORAGE_VALUE, privacy)
        self.assertIn(server.PRIVACY_COOKIE, privacy)

    def test_optional_prepared_media_supports_bounded_ranges(self):
        with tempfile.TemporaryDirectory() as media_tmp:
            media_dir = Path(media_tmp)
            payload = b"synthetic-prepared-media-fixture"
            (media_dir / "h264-aac.mp4").write_bytes(payload)
            other_state = server.FixtureState(None)
            extra = server.LoopbackServer(("127.0.0.1", 0), http.server.BaseHTTPRequestHandler)
            origin = f"http://127.0.0.1:{extra.server_port}"
            peer = "http://127.0.0.1:1"
            extra.RequestHandlerClass = server.make_handler(origin, peer, origin, other_state, False, media_dir)
            import threading
            threading.Thread(target=extra.serve_forever, daemon=True).start()
            try:
                response = self.get(origin + "/media/h264-aac.mp4", headers={"Range": "bytes=2-7"})
                self.assertEqual(response.status, 206)
                self.assertEqual(response.read(), payload[2:8])
                self.assertEqual(response.headers["Content-Range"], f"bytes 2-7/{len(payload)}")
                self.assertEqual(response.headers["Content-Type"], "video/mp4")
            finally:
                extra.shutdown()
                extra.server_close()
                other_state.log.close() if other_state.log else None

    def test_logs_are_fixed_route_ids_without_query_or_credentials(self):
        self.get(self.origin + "/nav/final?token=SHOULD-NOT-LOG")
        with self.assertRaises(urllib.error.HTTPError):
            self.get(self.origin + "/download/authenticated?token=SHOULD-NOT-LOG")
        rows = [json.loads(line) for line in self.log.read_text().splitlines()]
        self.assertTrue(all(set(row) <= {"at", "route", "status", "bytes"} for row in rows))
        output = self.log.read_text()
        for secret in ("SHOULD-NOT-LOG", "token=", server.AUTH_COOKIE):
            self.assertNotIn(secret, output)
        self.assertIn('"route":"navigation-target"', output)


if __name__ == "__main__":
    unittest.main()
