# Decisions to Close During M0

Do not ask the user about routine engineering choices that can be settled by evidence. Record each decision as an ADR and update this list. Rows marked **Human** are the human's decisions: Codex presents evidence and a recommendation and does not decide them.

| Decision | Default approach | Evidence required | Owner/status |
|---|---|---|---|
| Tauri vs WRY+TAO | Run comparable multi-WebView spikes; choose the least complex option that passes | API stability, native handles, security boundaries, lifecycle, overlays, X11/Wayland/Windows/macOS tests | **Human** / WRY + TAO is the provisional validation substrate. Final choice open, see [ADR 0001](adr/0001-m0a-substrate-evidence.md) |
| Minimum OS/runtime versions | Choose versions that balance reach and security/feature support | Current OS share not required; test matrix, WebKit/WebView2 requirements, packaging dependencies | M0 / tested engine versions, external runtime requirements and selected Linux dependency sizes recorded; shipping floors open |
| Linux support status | Validate X11 and Wayland separately; beta-label unsupported combinations | GTK/WebKitGTK baseline, package deps, focus/bounds/z-order, GPU/IME tests | **Human** / Separate GNOME X11 and Wayland desktop evidence pending. Linux DL-01 is held pending verified destination bytes. Stock Tauri layout failed both original probes. |
| CPU architectures | Start with architectures that can be built and tested natively; add others only with CI evidence | Runner availability and actual install tests | M0 / Mac arm64, translated x86-64, and x64 Windows/Linux fixtures measured; Windows ARM64 and native Intel support open |
| App chrome layout | Prototype compact horizontal tabs and collapsible vertical tabs | Real app screenshots, keyboard navigation, narrow-window usability | M1 / open |
| OAuth and passkeys | Test provider flows; document any policy/entitlement blocker | Google/Microsoft/GitHub; Apple entitlement eligibility; WebAuthn on each target | M0 / open |
| PDF rendering | Prefer reliable native viewer; use pdf.js if a supported platform lacks one | Fixture tests on each engine, size, security, licence | M0 / Mac and GTK samples render; Windows display and complex PDF/print remain open |
| Content blocking | Basic local rules only if supported reliably and licensed | API hook parity, list licensing, false-positive behaviour, performance | M0 / WebKit sample rule observed; Windows adapter, list and cost open |
| DRM and codecs | No promise until verified | Representative licensed test accounts/services, codecs, GPU/runtime | M0 / open |
| Signing and updates | Produce local/unsigned builds first; do not use paid credentials without approval | macOS notarization and Windows signing cost/credentials; updater key custody | M7 / open |
| Muffin name | Use as working name only | Trademark search in intended markets, app stores, domains, GitHub/org namespace | Before public branding / open |
| User checkpoints | Auto-advance objective engineering gates only between hard stops | Actual artifact availability; M0a/M0b, M6 and publish/release approvals remain human gates | Project / open |
| Repository visibility | Public | Decided by the human on 2026-11-18 | Closed |
| M0b evidence validation | Full build/binary identity plus redacted evidence for every non-`not_run` result | `tools/gen_status.py` and `tools/tests/test_gen_status.py`; bare-metal-only tests reject VM and CI-runner results | Project / Validator operational. The 2026-11-20 audit found 0 human results across 69 applicable cells |
| DEC-UA: user-agent policy | Engine default UA + truthful `Muffin/<version>` token; no impersonation | M0b sign-in results (SI-01 to SI-05); policy review | **Human** / open |
| Sign-in product gate | Pass = SI-01 to SI-04 as defined in `docs/M0B_MANUAL_TESTS.md` | M0b recorded results on bare metal or valid VMs | **Human** / open |
| Per-platform go / beta / no-go | Recommendation from the [M0a continuation](reports/m0/M0A_CONTINUATION_SUMMARY.md) | M0a + M0b evidence | **Human** / open |
| LICENSE for the public repository | None until decided | Dependency licence compatibility; intent (permissive vs copyleft) | **Human** / open |

See the [S1 follow-up handoff](reports/m0/M0A_S1_FOLLOW_UP.md) for updated evidence and recommendations. See the [original M0a handoff](reports/m0/M0A_SUMMARY.md) for the evidence, remaining S1 criteria, and dependent spikes held at the substrate gate. M0b remains human-operated.

The [continuation handoff](reports/m0/M0A_CONTINUATION_SUMMARY.md) adds the evidence and risk register. The [M0b preparation](reports/m0/M0B_PREPARATION.md) describes the scoped kits and remaining test-build prerequisites.

## M0b owner checkpoints

Human testing is underway by owner report. The repository has no submitted manual batch as of the 2026-11-20 audit. Windows 11 desktop, GNOME X11 and GNOME Wayland sessions and the hardware cases are still needed. Linux DL-01 is held after the download investigation. Final M0b acceptance, DEC-UA, the sign-in gate and release decisions for each platform remain with the owner. M1 requires explicit authorization after that review. [Progress and blockers](reports/m0/M0B_PROGRESS.md)
