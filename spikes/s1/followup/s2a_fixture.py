#!/usr/bin/env python3
"""Serve a loopback-only popup fixture and optionally launch its native host."""
import argparse
import http.server
import json
import subprocess
import threading
import time
from pathlib import Path


class FixtureServer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def popup_page(origins, title, auth=False):
    return f"""<!doctype html><meta charset="utf-8"><title>{title}</title>
<h1>{title}</h1><p id="state"></p>
<button id="notify">Send opener message</button>
<button id="close">Request window.close()</button>
<a href="{origins[0]}/download" download>Download synthetic file</a>
<pre id="events"></pre><script>
const report=(kind,data={{}})=>fetch('/event',{{method:'POST',headers:{{'content-type':'application/json'}},body:JSON.stringify({{kind,data}})}});
document.querySelector('#state').textContent=JSON.stringify({{opener:window.opener?'present':'null',ipc:typeof window.ipc}});
let openerRead = false;
try {{ openerRead = !!window.opener && typeof window.opener.document.title === 'string'; }} catch (_) {{}}
report('popup-ready',{{path:location.pathname,opener:!!window.opener,opener_read:openerRead,ipc:typeof window.ipc,focused:document.hasFocus()}});
document.querySelector('#notify').onclick=()=>{{if(window.opener)window.opener.postMessage({{fixture:'popup-message',origin:location.origin}},'{origins[0]}');}};
document.querySelector('#close').onclick=()=>{{report('close-request',{{opener:!!window.opener}});window.close();}};
window.addEventListener('message',e=>document.querySelector('#events').textContent+=JSON.stringify({{origin:e.origin,data:e.data}})+'\\n');
{"report('auth-callback',{synthetic:true,query:location.search});" if auth else ""}
</script>"""


def opener_page(origins, auto):
    return f"""<!doctype html><meta charset="utf-8"><title>S2a deterministic popup fixture</title>
<h1>S2a deterministic popup fixture</h1><p>Loopback content only.</p>
<button data-case="script" onclick="openCase('/popup/window-open')">window.open</button>
<a data-case="implicit" href="/popup/target-blank" target="_blank">target blank</a>
<a data-case="noopener" href="/popup/noopener" target="_blank" rel="noopener">noopener</a>
<button data-case="cross" onclick="openCase('{origins[1]}/popup/cross-origin')">cross-origin popup</button>
<button data-case="same-redirect" onclick="openCase('/redirect?same')">same-origin redirect</button>
<button data-case="cross-redirect" onclick="openCase('/redirect?cross')">cross-origin redirect</button>
<button data-case="auth" onclick="openCase('/auth/start')">synthetic authentication redirects</button>
<a data-case="blocked" href="muffin-s2a-denied:request" target="_blank">blocked protocol control</a>
<pre id="events"></pre>
<script>
const report=(kind,data)=>fetch('/event',{{method:'POST',headers:{{'content-type':'application/json'}},body:JSON.stringify({{kind,data}})}});
function openCase(url) {{
 const active=navigator.userActivation?.isActive ?? null;
 const popup=window.open(url,'_blank');
 report('open-return',{{url,active,has_handle:!!popup}});
}}
document.addEventListener('click',e=>{{const btn=e.target.closest('[data-case]');if(btn)report('parent-click',{{case:btn.dataset.case,trusted:e.isTrusted}});}});
window.addEventListener('message',e=>{{
 if(!{json.dumps(origins)}.includes(e.origin)||e.data?.fixture!=='popup-message')return;
 const data={{origin:e.origin,data:e.data}};
 document.querySelector('#events').textContent+=JSON.stringify(data)+'\\n';
 fetch('/event',{{method:'POST',headers:{{'content-type':'application/json'}},body:JSON.stringify({{kind:'opener-message',data}})}});
}});
if ({json.dumps(auto)}) {{
 const cases=Array.from(document.querySelectorAll('[data-case]'));
 cases.forEach((el,index)=>setTimeout(()=>el.click(),700+index*850));
}}
</script>"""


def make_handler(port_index, origins, log_file, log_lock):
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def respond(self, code, body, content_type="text/html; charset=utf-8", headers=()):
            raw = body.encode() if isinstance(body, str) else body
            self.send_response(code)
            self.send_header("Content-Type", content_type)
            self.send_header("Cache-Control", "no-store")
            for key, value in headers:
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)

        def do_POST(self):
            if self.path != "/event":
                return self.respond(404, b"not found", "text/plain")
            try:
                size = int(self.headers.get("Content-Length", "-1"))
                if not 0 <= size <= 2048:
                    raise ValueError("body size")
                event = json.loads(self.rfile.read(size))
                allowed = {"opener-message", "auth-callback", "close-request", "popup-ready", "open-return", "parent-click"}
                if not isinstance(event, dict) or event.get("kind") not in allowed:
                    raise ValueError("event shape")
                record = {"at": round(time.time(), 3), "server": origins[port_index], "event": event}
                with log_lock:
                    log_file.write(json.dumps(record, separators=(",", ":")) + "\n")
                self.respond(204, b"", "text/plain")
            except (ValueError, json.JSONDecodeError):
                self.respond(400, b"bad request", "text/plain")

        def do_GET(self):
            if self.path.startswith("/redirect"):
                cross = "cross" in self.path
                target = origins[1 - port_index] if cross else origins[port_index]
                return self.respond(302, b"", "text/plain", [("Location", target + "/popup/redirect-final")])
            if self.path == "/auth/start":
                return self.respond(302, b"", "text/plain", [("Location", origins[1 - port_index] + "/auth/consent")])
            if self.path == "/auth/consent":
                return self.respond(302, b"", "text/plain", [("Location", origins[0] + "/auth/callback?code=SYNTHETIC-CODE")])
            if self.path.startswith("/auth/callback"):
                return self.respond(200, popup_page(origins, "synthetic auth callback", True))
            if self.path == "/download":
                return self.respond(200, b"SYNTHETIC-DOWNLOAD-CONTENT\n", "application/octet-stream",
                                    [("Content-Disposition", 'attachment; filename="muffin-s2a.txt"')])
            if self.path.startswith("/popup/"):
                return self.respond(200, popup_page(origins, self.path))
            if self.path in ("/", "/?mode=auto") and port_index == 0:
                return self.respond(200, opener_page(origins, self.path.endswith("mode=auto")))
            self.respond(404, "not found", "text/plain")

    return Handler


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", help="optional native fixture executable")
    parser.add_argument("--work-dir", default="/private/tmp/muffin-s2a-run")
    parser.add_argument("--timeout-seconds", type=int, default=300)
    parser.add_argument("--auto", action="store_true", help="programmatic clicks without desktop activation")
    args = parser.parse_args()
    work = Path(args.work_dir).resolve()
    work.mkdir(parents=True, exist_ok=True)
    events_path, native_path = work / "content-events.jsonl", work / "native.log"
    log_lock = threading.Lock()
    log_file = events_path.open("w", encoding="utf-8", buffering=1)
    origins, servers = [], []
    for _ in range(2):
        server = FixtureServer(("127.0.0.1", 0), http.server.BaseHTTPRequestHandler)
        origins.append(f"http://127.0.0.1:{server.server_port}")
        servers.append(server)
    for index, server in enumerate(servers):
        server.RequestHandlerClass = make_handler(index, origins, log_file, log_lock)
    threads = [threading.Thread(target=server.serve_forever, daemon=True) for server in servers]
    for thread in threads:
        thread.start()
    start_url = origins[0] + ("/?mode=auto" if args.auto else "/")
    metadata = {"start_url": start_url, "cross_origin": origins[1], "content_log": str(events_path),
                "native_log": str(native_path), "source": "loopback-only synthetic fixture"}
    print(json.dumps(metadata, sort_keys=True), flush=True)
    exit_code = 0
    try:
        if args.binary:
            command = [str(Path(args.binary).resolve()), start_url, origins[1], str(work)]
            if args.auto:
                command.append("--auto")
            with native_path.open("w", encoding="utf-8") as native:
                child = subprocess.Popen(command, stdout=native, stderr=subprocess.STDOUT)
                try:
                    code = child.wait(timeout=args.timeout_seconds)
                    print(f"fixture_finished exit_code={code}", flush=True)
                    exit_code = code if code >= 0 else 128 - code
                except subprocess.TimeoutExpired:
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait()
                    print("fixture_finished outcome=timeout child_terminated=true", flush=True)
                    exit_code = 124
        else:
            for thread in threads:
                thread.join()
    except KeyboardInterrupt:
        pass
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
        log_file.close()
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
