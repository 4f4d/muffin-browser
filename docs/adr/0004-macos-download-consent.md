# ADR 0004 — macOS download consent and transfer identity

**Status** Implemented for the macOS development browser  
**Date** 2026-11-20

## Context

The pinned WRY download callback exposes the original URL and a destination at start. macOS completion reports the URL and result without the destination or a transfer identifier. The M4 baseline granted a single address-bar download by URL. An ordinary page link could not use that grant, and identical URLs could not identify concurrent transfers. [Pinned WRY navigation](https://github.com/tauri-apps/wry/blob/wry-v0.57.2/src/wkwebview/navigation.rs), [pinned WRY download callback](https://github.com/tauri-apps/wry/blob/wry-v0.57.2/src/wkwebview/download.rs)

## Decision

Install a macOS navigation delegate around the existing content-view delegate. Keep WRY's ordinary navigation policy, load events, and process-termination handling. When WebKit converts an eligible HTTP or HTTPS attachment or downloadable link, hold its native destination callback. Muffin's Downloads surface displays the filename and response host. Save and Cancel send an opaque ID over the existing trusted chrome channel. The original WebKit request stays in the same transfer and keeps its session and method. [Apple navigation callback](https://developer.apple.com/documentation/webkit/wknavigationdelegate/webview(_:navigationresponse:didbecome:)), [Apple download delegate](https://developer.apple.com/documentation/webkit/wkdownloaddelegate), [native adapter](../../crates/muffin/src/download_delegate.rs)

Each `WKDownload` stays associated with its own ID until completion or cancellation. The app creates a staging file path only after consent. A successful regular file is published into Downloads with an atomic no-clobber hard link under its final visible name. Outstanding approvals expire. Muffin checks known response sizes before offering, polls staging sizes during a transfer, and checks the final size before publishing. Polling permits a transfer to temporarily exceed the 512 MiB publication limit. [Store tests](../../crates/muffin/src/downloads.rs), [chrome contract](../../ui/src/lib/bridge.ts)

## Trade-offs and verification

An address grant covered only direct navigation. A native modal inside the WRY callback would still correlate completion by URL and could interrupt other browsing. A fresh fetch would lose WebKit's request context. The retained native delegate adds a version-sensitive adapter and keeps approval in the user's Downloads view. Engines without the required download selector keep the fail-closed fallback. Verify browser-initiated attachments, duplicate names, redirects, denial before filesystem allocation, and the strict chrome action contract whenever the adapter or the WRY version changes. [Native smoke](../../tests/native/run_smoke.py), [UI tests](../../ui/tests/app.test.ts)

The old address-grant regression becomes approval-state coverage. It checks one-time Save, Cancel without staging, idempotent completion, and distinct IDs for the same URL. The native fixture also checks file size and digest after consent, including a cross-origin redirect with the final response host. UI tests check escaped filenames and hosts, the pending count, and the exact Save and Cancel messages. [Store tests](../../crates/muffin/src/downloads.rs), [fixture](../../tests/native/fixture.py), [IPC tests](../../ui/tests/bridge.test.ts)

## Destination selection follow-up

The folder picker is a native action from Settings. Trusted chrome can request the picker or reset to the system Downloads folder. The native profile stores the validated path and sends only the folder name to chrome. Each transfer captures its destination when approved and publishes there, so a new preference applies to subsequent approvals. A missing custom directory prevents a new save until the user chooses a valid directory or resets the preference. The existing exclusive publication rule applies in both locations. [Native routing](../../crates/muffin/src/main.rs), [store](../../crates/muffin/src/downloads.rs), [profile](../../crates/muffin/src/profile.rs)

## Retry and Open

For an eligible failed HTTP or HTTPS GET, Retry starts a fresh native download in the owning content view. Eligibility excludes requests with URL credentials, a body, or a body stream. The new request drops the original headers and disables Foundation's request cookie handling. The engine manages the network request in the owning WebView. An authenticated download may require the user to return to the site and download it again. Each successful restart receives a new ID and needs explicit Save before Muffin creates a staging file. Closing the owning view or clearing the row invalidates its retry token. [Native adapter](../../crates/muffin/src/download_delegate.rs), [loopback scenario](../../tests/native/run_smoke.py)

Open accepts a finished download ID from the Downloads surface. The native opener checks the recorded file identity, regular-file status, canonical path, and execute bits. It sends allowed text, PDF, and image types to TextEdit or Preview and offers Show in folder for everything else. It leaves OS quarantine and security controls in place. Revisit file identity and launch handling if shared download destinations with concurrent local writers become a supported threat model. [Native opener and tests](../../crates/muffin/src/download_open.rs)
