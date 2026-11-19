# Decisions to Close During M0

Do not ask the user about routine engineering choices that can be settled by evidence. Record each decision as an ADR and update this list. Rows marked **Human** are the human's decisions: Codex presents evidence and a recommendation and does not decide them.

| Decision | Default approach | Evidence required | Owner/status |
|---|---|---|---|
| Tauri vs WRY+TAO | Run comparable multi-WebView spikes; choose the least complex option that passes | API stability, native handles, security boundaries, lifecycle, overlays, X11/Wayland/Windows/macOS tests | M0 / open |
| Minimum OS/runtime versions | Choose versions that balance reach and security/feature support | Current OS share not required; test matrix, WebKit/WebView2 requirements, packaging dependencies | M0 / open |
| Linux support status | Validate X11 and Wayland separately; beta-label unsupported combinations | GTK/WebKitGTK baseline, package deps, focus/bounds/z-order, GPU/IME tests | M0 / open |
| CPU architectures | Start with architectures that can be built and tested natively; add others only with CI evidence | Runner availability and actual install tests | M0 / open |
| App chrome layout | Prototype compact horizontal tabs and collapsible vertical tabs | Real app screenshots, keyboard navigation, narrow-window usability | M1 / open |
| OAuth and passkeys | Test provider flows; document any policy/entitlement blocker | Google/Microsoft/GitHub; Apple entitlement eligibility; WebAuthn on each target | M0 / open |
| PDF rendering | Prefer reliable native viewer; use pdf.js if a supported platform lacks one | Fixture tests on each engine, size, security, licence | M0 / open |
| Content blocking | Basic local rules only if supported reliably and licensed | API hook parity, list licensing, false-positive behaviour, performance | M0 / open |
| DRM and codecs | No promise until verified | Representative licensed test accounts/services, codecs, GPU/runtime | M0 / open |
| Signing and updates | Produce local/unsigned builds first; do not use paid credentials without approval | macOS notarization and Windows signing cost/credentials; updater key custody | M7 / open |
| Muffin name | Use as working name only | Trademark search in intended markets, app stores, domains, GitHub/org namespace | Before public branding / open |
| User checkpoints | Auto-advance objective engineering gates only between hard stops | Actual artifact availability; M0a/M0b, M6 and publish/release approvals remain human gates | Project / open |
| Repository visibility | Public | Decided by the human on 2026-11-18 | Closed |
| M0b evidence validation | Full build/binary identity plus redacted evidence for every non-`not_run` result | `tools/gen_status.py` and `tests/test_gen_status.py`; bare-metal-only tests reject VM and CI-runner results | Project / implemented in starter package |
| DEC-UA: user-agent policy | Engine default UA + truthful `Muffin/<version>` token; no impersonation | M0b sign-in results (SI-01 to SI-05); policy review | **Human** / open |
| Sign-in product gate | Pass = SI-01 to SI-04 as defined in `docs/M0B_MANUAL_TESTS.md` | M0b recorded results on bare metal or valid VMs | **Human** / open |
| Per-platform go / beta / no-go | Recommendation from `M0A_SUMMARY.md` | M0a + M0b evidence | **Human** / open |
| LICENSE for the public repository | None until decided | Dependency licence compatibility; intent (permissive vs copyleft) | **Human** / open |
