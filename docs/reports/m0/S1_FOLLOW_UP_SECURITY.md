# S1 WRY trust-boundary assessment

## Scope and result

Two interactive macOS WKWebView runs exercised the pinned WRY 0.57.2 fixture. The active content page was a separate loopback HTTP origin with its own WebView. It ran the same ordinary web operations available to an arbitrary web document. The second run used a confirmed synthetic file inside the fixture app's `Contents/Resources`. The file contained a random marker generated for the run. These are observations about the implemented fixture boundary on this engine. [Run identity](evidence/s1-followup-macos-bundle-environment.txt), [fixture source](../../../spikes/s1/followup/fixture.py)

| Surface | Controlled attempt | Observation |
|---|---|---|
| Native command | The content page attempted the exact fixture command through its link, page-local message delivery and any available IPC objects. The trusted pane invoked the same command before and after. | The native host denied the content command URL. Page-local postMessage was delivered only to the page. The host recorded two accepted chrome calls and shut down with count 2. Neither content IPC object was exposed. |
| Privileged IPC | The untrusted pane attempted the available message path amid the positive controls. | No additional native call reached the registered chrome-only handler. The positive call worked again after the untrusted attempts. |
| Local application file | The content page fetched and attempted to navigate to the synthetic bundled file. An allowed second HTTP origin served a control. | The control returned the expected data and HTTP 200. The file fetch rejected with `TypeError`. The attempted top-level file navigation left the content on its HTTP page. No native file navigation denial was logged, so the observed layers behind that navigation outcome remain unresolved. |
| Chrome-only asset | The page fetched and attempted to navigate to the private chrome URI. | Fetch rejected with `TypeError`. The host logged a navigation denial for the private scheme in the content view. The trusted pane rendered the actual private document. |

The native and content logs bracket the negative attempts with two positive chrome actions. The fixture verifier checks the fetch control, both attempted navigations, native denials for command and chrome URI, and exact native call count. [Native log](evidence/s1-followup-macos-bundle-native.txt), [content log](evidence/s1-followup-macos-bundle-content.jsonl), [verifier](../../../spikes/s1/followup/verify_boundary.py)

## Proposed communication architecture

- Bundle the chrome UI and use a private, exact resource allowlist with a restrictive CSP. Install its scheme handler and IPC handler on the trusted chrome WebView only. Deny chrome navigation away from its trusted origin and omit subframes and inline user-generated HTML.
- Construct every remote-content WebView through a separate builder with no privileged handler, initialization scripts, private scheme handler, or host file access. Validate top-level navigation and popup decisions independently of page script. Reserve local-file opens and external protocol actions for explicit native UI paths.
- Bind each native request to the trusted WebView identity and current document. Validate the request URI, command tag and typed payload at dispatch. Deny unknown commands, untrusted frames and stale documents. Keep host file paths and application state in native code. Send remote titles and URLs as data to the chrome, without executing them there or reflecting them as HTML.
- Keep future renderer lifecycle and permission handling on the native side. A renderer restart should recreate the same trust boundary and load an explicitly validated URL, with a bounded retry policy.

## Evidence still needed

The research fixture validates one canary effect and one synthetic bundled file. It does not establish safety for the future command set, other operating systems, WebKit engine vulnerabilities, redirects across all origin classes, or any intentionally exposed local-file access workflow. Validate per-platform private scheme behavior and the actual privileged commands when the browser shell exists. Repeat the boundary cases for subframes and popups before product integration. No network or WebView security protection was disabled for this test.
