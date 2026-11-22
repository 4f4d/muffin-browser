# M5 — macOS privacy and security

## Scope

Private windows, website data removal, unsupported-protocol handling and the TLS error surface are the macOS development scope. The private-window shortcut is Shift–Command–N. The decision record explains the WebKit store lifetime, the role of the regular profile, and what happens to a file saved from a private window. [Privacy decision](../../adr/0006-macos-private-window-and-website-data.md)

## Qualification boundaries

- The verifier uses a disposable regular WebKit store and a disposable SQLite profile. The test compares cookies, localStorage and Cache Storage, opens two private windows, and restarts the normal session. A separate forensic examination of OS or engine caches and crash diagnostics has not been performed. The owner-operated M0b matrix remains deferred. [Native fixture](../../../tests/native/run_smoke.py), [deferred qualification](../../adr/0002-macos-first-implementation-and-deferred-qualification.md)
- Download quarantine evidence covers the metadata preserved on an approved loopback download. Gatekeeper, signed-app launch, private-mode capture permissions, third-party cookies and packaged-app behavior remain qualification work. [M0 capabilities](../m0/S4_EVERYDAY_MATRIX.md)
- WebKit blocks automatic JavaScript windows through its engine preference. Muffin confines requests reaching WRY’s window callback to HTTP and HTTPS and routes them to the opener’s window. WRY does not give Muffin frame or activation provenance. Open-site and sign-in popup compatibility still need qualification. [Apple popup preference](https://developer.apple.com/documentation/webkit/wkpreferences/javascriptcanopenwindowsautomatically), [M0 popup evidence](../m0/spikes/S2a-macos-wry-tao.md)
- Content blocking and HTTPS-first need further validation. The native rule-list feasibility fixture did not exercise production rules, licensing, false positives, exceptions or overhead. The application defaults bare hostnames to HTTPS and offers no upgrade and fallback policy for explicit HTTP navigation. Unsupported external protocols stay blocked. [S4 feasibility](../m0/S4_EVERYDAY_MATRIX.md), [open decisions](../../DECISIONS_PENDING.md)
