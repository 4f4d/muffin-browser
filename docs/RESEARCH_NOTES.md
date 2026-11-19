# Muffin Browser — Initial Research Notes

**Checked:** 2026-11-18  
**Purpose:** Seed the M0 verification work. This file is not a substitute for reading the current version-specific documentation when pinning dependencies.

## Current technical references

| Topic | Source | Initial implication |
|---|---|---|
| Tauri child WebViews | https://v2.tauri.app/reference/javascript/api/namespacewebview/ | Tauri's child-WebView creation is documented as requiring the `unstable` Cargo feature. Verify the exact Tauri version and Rust API before selection. |
| Tauri capabilities | https://v2.tauri.app/security/capabilities/ | Capabilities apply to selected windows/WebViews; permissions can merge across capabilities. Keep remote content outside privileged capability scopes. |
| Tauri CSP | https://v2.tauri.app/security/csp/ | Configure strict CSP for bundled chrome. Do not load remote scripts or fonts. |
| WRY current API | https://docs.rs/wry/latest/wry/ | Review current child WebView and platform-specific APIs at the exact pinned version. |
| WRY Wayland path | https://docs.rs/wry/latest/wry/ | The GTK construction path is recommended when supporting Linux X11 and Wayland; ordinary child-window construction is X11-only. Prove it in a real shell. |
| Apple browser passkeys | https://developer.apple.com/documentation/authenticationservices/passkey-use-in-web-browsers | WKWebView can handle WebAuthentication challenges; browser access to passkeys must use Apple's browser credential manager/permission process. |
| Apple browser entitlement | https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.web-browser.public-key-credential | Requesting passkeys for arbitrary relying parties requires the browser entitlement and Apple approval. Verify account eligibility and timing; do not assume it is granted. |
| Google OAuth policies | https://developers.google.com/identity/protocols/oauth2/policies | Google disallows OAuth in embedded user agents under app control. Test real browser navigation and do not evade policy with UA spoofing. |
| Google desktop OAuth errors | https://developers.google.com/identity/protocols/oauth2/native-app | Google documents `disallowed_useragent`, including examples involving WKWebView on macOS. Investigate impact on daily browsing. |
| WebKitGTK download API | https://webkitgtk.org/reference/webkit2gtk/stable/method.Download.cancel.html | Current API reference documents cancellation; do not promise a cross-platform pause/resume feature without further proof. |
| WebKitGTK releases | https://webkitgtk.org/news.html | Check current stable releases and distro package baselines when deciding Linux minimum versions. |

## Verified in review pass (2026-11-18)

| Claim | Source | Confidence | Note |
|---|---|---|---|
| Creating child WebViews in Tauri 2 requires the `unstable` Cargo feature; the single-WebView `WebviewWindow` path is stable | https://v2.tauri.app/reference/javascript/api/namespacewebview | High (official docs) | Re-check against the exact pinned version. |
| WRY `build_as_child` is supported on macOS, Windows, and Linux (X11 only) | https://docs.rs/wry/latest/ | High (official docs) | |
| For X11 and Wayland, WRY recommends the GTK path (`new_gtk` / `build_gtk`) with a `gtk::Fixed` container | https://docs.rs/wry/latest/ | High (official docs) | Prove focus, bounds, and resize in a real shell. |
| WRY exposes permission, new-window, and similar handler types (`PermissionKind`, `PermissionResponse`, `NewWindowResponse`) | https://docs.rs/wry/latest/ | Medium | Start S3 from the current API before assuming native FFI is needed. Verify per-platform coverage. |
| WRY `transparent` and `fullscreen` features call private macOS APIs and are unsuitable for App Store publication | WRY README feature list (seen via a mirror; confirm in tauri-apps/wry) | Medium | Acceptable for direct notarized distribution; record the trade-off. Fullscreen video is on the launch list. |
| Stock Tauri on Linux puts child WebViews in a vertical GTK box and ignores their bounds; the fix is open, unreviewed PRs | https://github.com/Fledge-Software/Syrinx-app/wiki/Background-01-shell-and-engine | **Low (secondary project notes)** | `needs-verification`. If true, Tauri-on-Linux fails S1. Treat as presumed failing until S1 shows otherwise. |

## Mandatory M0 checks

1. Confirm version-specific Tauri child-WebView stability and capabilities on all targets.
2. Confirm WRY event-loop, child-WebView, focus, bounds, and GTK Wayland behaviour on supported Linux environments.
3. Test native multi-view z-order and overlays; a framework's API surface does not prove correct rendering.
4. Test common identity-provider flows (Google, Microsoft, GitHub) and passkeys where possible.
5. Test DRM and common codecs rather than assuming system WebView support.
6. Verify native PDF behaviour per engine/distribution and select a bundled PDF fallback only if required.
7. Inspect current licensing of every filter list, font, icon set, and bundled third-party asset.
8. Verify safe browsing, TLS interstitial, download quarantine, and permission features rather than assuming browser-engine defaults.
9. Record minimum OS/WebView versions and security-update cadence per platform.
10. Capture actual package size, installed footprint, and process-tree measurements.

## Evidence discipline

- Official documentation establishes API and platform policy, not necessarily that the complete app integration works.
- GitHub issues are implementation evidence and may describe a specific version or bug; record the affected version and status.
- Reddit and community posts are qualitative user sentiment. Record sample size and do not claim statistical popularity.
- Mark any unverified assumption as `needs-verification` in the research log and do not turn it into a product promise.

## M0a source recheck (2026-11-19)

The dated source log at `docs/reports/m0/evidence/source-checks.tsv` records primary sources, confidence, and what each establishes. Version-specific source supersedes the seed notes where they differ.

- **Tauri Linux.** The Tauri 2.12.7 tag confirms the stock child path uses the TAO default GTK vertical box. Its use of WRY's GTK constructor is confirmed at source level. Runtime bounds and overlay behavior require S1 evidence. Issue #10420 reports stacking in older builds and supplies historical context.
- **WRY Linux.** WRY's documented `gtk::Fixed` route and TAO's exposed GTK container support a distinct S1 setup. The ordinary child-window API covers Linux X11 only. Test GTK separately under both backends.
- **Tauri isolation.** The tagged runtime registers an IPC handler as it constructs views. Safe treatment of remote Tauri children depends on which capabilities and commands are reachable. Record the result of an explicit remote-content boundary test before treating this path as safe for browsing.
- **Provider gates.** Google's embedded OAuth restriction and Apple's browser entitlement documentation are current policy evidence. Actual provider behavior, entitlement eligibility and sign-in outcomes remain needs-verification in the human runbook.
- **Runtime distribution.** Treat WebView2 Evergreen as the runner observation and a candidate deployment policy. A published system/runtime minimum remains open.
