#!/usr/bin/env python3
"""Bounded loopback fixture for the Muffin S1 follow-up probe."""
from __future__ import annotations

import argparse
import http.server
import json
import os
import plistlib
import shutil
import subprocess
import tempfile
import threading
import time
import urllib.parse
import uuid
from pathlib import Path

MAX_BODY = 4096
CONTROL = {"fixture": "muffin-followup-control", "version": 1, "ok": True}


def page() -> bytes:
    return b'''<!doctype html><meta charset="utf-8"><title>M0 follow-up content</title>
<style>body{font:16px system-ui;max-width:760px;margin:2rem auto;color:#18212b}input,button{font:inherit;padding:.5rem}section{border:1px solid #ccd4dc;padding:1rem;margin:1rem 0}code{overflow-wrap:anywhere}</style>
<h1>M0 follow-up fixture</h1><p>Local synthetic content; no external sites are loaded.</p>
<label for="content-input">Content input</label> <input id="content-input" autocomplete="off" maxlength="64">
<button id="run">Run safe fetch probes</button> <a id="command" href="muffin-followup:mark">Try muffin-followup:mark</a>
<button id="file-nav">Try local file navigation</button> <button id="chrome-nav">Try private chrome navigation</button>
<section><h2>Fixture results</h2><p id="state">Waiting for probe.</p><pre id="results"></pre></section>
<script>
(()=>{
 const reportPath='/api/report', results=document.querySelector('#results'), state=document.querySelector('#state');
 const bounded=s=>String(s).slice(0,240);
 async function post(name, data={}) {
   const clean={name:bounded(name), data};
   try { await fetch(reportPath,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(clean)}); } catch (_) {}
   results.textContent += JSON.stringify(clean)+'\\n';
 }
 async function probe(){
   state.textContent='Running probes'; const out={};
   try {const r=await fetch(window.__SESSION.control_url+'/control.json'); const body=await r.json(); out.positive_control={status:r.status,expected:body.fixture==='muffin-followup-control'&&body.version===1&&body.ok===true};}
   catch(e){out.positive_control={error:bounded(e.name||'fetch-error')};}
   try {const r=await fetch(window.__SESSION.file_url,{mode:'cors'}); const body=await r.text(); out.private_file={status:r.status,marker_observed:body.includes(window.__SESSION.file_marker)};}
   catch(e){out.private_file={error:bounded(e.name||'fetch-error')};}
   try {const r=await fetch('muffin-chrome://fixture/index.html',{mode:'cors'}); out.chrome_uri={status:r.status};}
   catch(e){out.chrome_uri={error:bounded(e.name||'fetch-error')};}
   await post('fetch-probes',out); state.textContent='Probe requests completed; inspect results and native logs.';
 }
 document.querySelector('#run').addEventListener('click',probe);
 document.querySelector('#command').addEventListener('click',async()=>{
   const out={scheme_link:'activated', window_ipc:typeof window.ipc, webkit_ipc:typeof window.webkit?.messageHandlers?.ipc, postMessage:'available'};
   try { window.postMessage('muffin-followup:mark','*'); } catch(e) {out.postMessage=bounded(e.name);}
   try { if(window.ipc&&typeof window.ipc.postMessage==='function') window.ipc.postMessage('muffin-followup:mark'); } catch(e){out.window_ipc_error=bounded(e.name);}
   try { const h=window.webkit?.messageHandlers?.ipc; if(h&&typeof h.postMessage==='function')h.postMessage('muffin-followup:mark'); } catch(e){out.webkit_ipc_error=bounded(e.name);}
   await post('command-attempt',out);
 });
 document.querySelector('#file-nav').addEventListener('click',async()=>{await post('navigation-attempt',{target:'file'});location.href=window.__SESSION.file_url;});
 document.querySelector('#chrome-nav').addEventListener('click',async()=>{await post('navigation-attempt',{target:'chrome'});location.href='muffin-chrome://fixture/index.html';});
 document.querySelector('#content-input').addEventListener('keydown',e=>{if(e.key==='Enter'){e.preventDefault();post('input-enter',{observed:true});}});
 window.addEventListener('message',e=>{ if(e.source===window&&e.data==='muffin-followup:mark')post('inert-postmessage',{observed:true}); });
 window.addEventListener('DOMContentLoaded',()=>{state.textContent='Ready; focus the labelled input or run the probes.';});
})();
</script>'''


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--binary', required=True, help='built Muffin S1 macOS executable')
    ap.add_argument('--launch', action='store_true', help='open the prepared bundle with macOS Launch Services')
    ap.add_argument('--hold', type=int, default=30, help='seconds to keep fixture servers alive after app exits')
    ap.add_argument('--work-dir', help='optional output directory; default uses a temporary directory')
    args = ap.parse_args()
    binary = Path(args.binary).expanduser().resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        ap.error('--binary must name an executable file')
    base = Path(args.work_dir).expanduser().resolve() if args.work_dir else Path(tempfile.mkdtemp(prefix='muffin-followup-'))
    base.mkdir(parents=True, exist_ok=True)
    bundle = base / 'MuffinFollowup.app'
    resources = bundle / 'Contents' / 'Resources'
    macos = bundle / 'Contents' / 'MacOS'
    resources.mkdir(parents=True, exist_ok=True)
    macos.mkdir(parents=True, exist_ok=True)
    executable = macos / 'MuffinFollowup'
    shutil.copyfile(binary, executable)
    executable.chmod(executable.stat().st_mode | 0o111)
    marker = 'MFF-' + uuid.uuid4().hex
    private_file = base / 'synthetic-private-canary.txt'
    private_file.write_text('Synthetic local canary: ' + marker + '\n', encoding='utf-8')
    (bundle / 'Contents' / 'Info.plist').write_bytes(plistlib.dumps({
        'CFBundleExecutable': 'MuffinFollowup', 'CFBundleIdentifier': 'local.muffin.followup.fixture',
        'CFBundleName': 'Muffin Follow-up Fixture', 'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1'
    }))
    log_path = base / 'fixture.log'
    log = log_path.open('w', encoding='utf-8', buffering=1)
    lock = threading.Lock()

    class ContentHandler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_): pass
        def respond(self, status, body, typ='text/html; charset=utf-8', headers=None):
            self.send_response(status); self.send_header('Content-Type', typ); self.send_header('Cache-Control','no-store')
            for k,v in (headers or {}).items(): self.send_header(k,v)
            self.send_header('Content-Length',str(len(body))); self.end_headers(); self.wfile.write(body)
        def do_GET(self):
            if self.path.split('?',1)[0] in ('/start','/next'):
                html = page()
                bootstrap = ('<script>window.__SESSION='+json.dumps({**session,'control_url':control_origin},separators=(',',':'))+';</script>').encode()
                html = html.replace(b'<script>', bootstrap+b'<script>', 1)
                self.respond(200,html); return
            self.respond(404,b'not found','text/plain')
        def do_POST(self):
            if self.path != '/api/report': self.respond(404,b'not found','text/plain'); return
            try:
                size=int(self.headers.get('Content-Length','-1'))
                if size < 0 or size > MAX_BODY: raise ValueError('size')
                data=json.loads(self.rfile.read(size))
                if not isinstance(data,dict) or data.get('name') not in {'fetch-probes','command-attempt','inert-postmessage','input-enter','navigation-attempt'}: raise ValueError('shape')
                payload=data.get('data',{})
                if not isinstance(payload,dict) or len(json.dumps(payload)) > 2048: raise ValueError('payload')
                entry={'at':round(time.time(),3),'name':data['name'],'data':payload}
                with lock:
                    log.write(json.dumps(entry,separators=(',',':'))+'\n')
                self.respond(204,b'','text/plain'); return
            except Exception:
                self.respond(400,b'bad request','text/plain')

    class ControlHandler(http.server.BaseHTTPRequestHandler):
        def log_message(self,*_): pass
        def do_GET(self):
            origin=self.headers.get('Origin','')
            if self.path != '/control.json' or origin != content_origin:
                self.send_error(403); return
            body=json.dumps(CONTROL,separators=(',',':')).encode()
            self.send_response(200); self.send_header('Content-Type','application/json'); self.send_header('Access-Control-Allow-Origin',content_origin)
            self.send_header('Vary','Origin'); self.send_header('Cache-Control','no-store'); self.send_header('Content-Length',str(len(body))); self.end_headers(); self.wfile.write(body)

    content=Server(('127.0.0.1',0),ContentHandler)
    control=Server(('127.0.0.1',0),ControlHandler)
    content_origin=f'http://127.0.0.1:{content.server_port}'
    control_origin=f'http://127.0.0.1:{control.server_port}'
    session={'content_url':content_origin+'/start','file_url':private_file.as_uri(),
             'file_marker':marker,'control_url':control_origin}
    (resources/'session.json').write_text(json.dumps(session,sort_keys=True),encoding='utf-8')
    threads=[threading.Thread(target=s.serve_forever,daemon=True) for s in (content,control)]
    for t in threads: t.start()
    print(json.dumps({'bundle_path':str(bundle),'log_path':str(log_path),'content_url':session['content_url'],'session_path':str(resources/'session.json')},sort_keys=True),flush=True)
    try:
        if args.launch:
            native_log = base / 'native.log'
            native_log.touch()
            launch = ['/usr/bin/open','-W','-n','--stdout',str(native_log),
                      '--stderr',str(base/'native.stderr'),str(bundle)]
            child=subprocess.run(launch,timeout=620,check=False)
            time.sleep(max(0,min(args.hold,300)))
            if child.returncode:
                return child.returncode
        else:
            time.sleep(620)
    except KeyboardInterrupt:
        pass
    finally:
        content.shutdown(); control.shutdown(); log.close()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
