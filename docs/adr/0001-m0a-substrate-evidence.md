# ADR 0001 — Keep substrate selection open after S1

Status: proposed for human review
Date: 2026-11-19

## Context

Muffin requires a trusted chrome surface and multiple native content WebViews, precise child geometry and overlay stacking, input and lifecycle control, and a remote-content boundary. M0a compared WRY 0.57.2 with TAO 0.37.2 and stock Tauri 2.12.7 with its unstable child API. Tauri resolved runtime-wry 2.12.5 and WRY 0.57.3. Each spike's starting and stopping commits and exact binary identities are in the [M0a reports](../reports/m0/spikes/).

## Options and evidence

- **WRY + TAO.** Explicit child placement on macOS and Windows and `gtk::Fixed` placement on Linux. The fixture demonstrated bounds and loops on macOS, along with actual overlay composition on Windows and X11. The Mac focus calls left both documents unfocused even with a visible key window, and WindowServer capture yielded no image. Both targeted focus reads succeeded in the Xvfb follow-up. Headless Wayland startup geometry still differed from the expectation. A controlled GTK renderer termination reloaded the loopback page on both backends, with viewport preservation still open. WRY exposed neither of the inspected bridge globals in loopback content. [Mac log](../reports/m0/evidence/s1-wry-macos-native-id.txt), [Windows capture](../reports/m0/evidence/s1-windows-wry-tao.png), [Linux server capture](../reports/m0/evidence/s1-wry-linux-server-x11.png), [pixel check](../reports/m0/evidence/s1-image-check.txt), [Wayland recovery](../reports/m0/evidence/s1-wry-server-wayland-recovery.txt)
- **Stock Tauri.** The Windows child fixture achieved the intended overlap and geometry. Its loopback document observed two bridge globals with no privileged commands registered in the fixture. The Linux stock path allocated three stacked panes under X11 and Wayland. A separate Mac fixture did not build within its attempt limit. [Windows log](../reports/m0/evidence/s1-windows-tauri.txt), [X11 capture](../reports/m0/evidence/s1-x11-tauri.png), [Wayland log](../reports/m0/evidence/s1-wayland-tauri.txt), [Mac report](../reports/m0/spikes/S1-macos-tauri.md)
- **Other substrate or bundled engine.** M0a produced no comparable artifact for this option. It remains outside the recommendation and would need a separate footprint, security-update and maintenance case. [M0a summary](../reports/m0/M0A_SUMMARY.md)

## Recommendation and trade-offs

Carry WRY + TAO forward as the first candidate for a focused follow-up after the human reviews the mixed evidence. It provides explicit GTK placement and the clearest observed separation of fixture bridge surfaces. Its direct integration also puts native input, lifecycle and distribution work on Muffin. Stock Tauri offers framework infrastructure, with Linux child layout, the unstable API and remote capability reachability still requiring resolution. The evidence does not close substrate selection or a platform support decision. [Tagged source evidence](../reports/m0/evidence/source-checks.tsv)

## Revisit triggers

Revisit on comparable actual AppKit/WebView2/GTK evidence for focus, shortcuts and renderer recovery, with native overlay capture and at least one interactive Wayland desktop run. Require a negative capability test against any privileged chrome channel before adopting a remote-content path. Record product-level substrate and per-platform decisions only after human review. [M0a handoff](../reports/m0/M0A_SUMMARY.md)

## S1 follow-up evidence — 2026-11-19

The interactive macOS follow-up extends the original Mac fixture findings. With pinned WRY 0.57.2 and TAO 0.37.2, AppKit first-responder ancestry and actual keyboard events independently established both focus transitions. The fixture used a native app-local event monitor for Cmd+1 and Cmd+2 and explicitly requested DOM input focus after checking the target document. The actual window capture shows the overlay crossing both panes. Unexpected WebContent termination and recovery still lack a runtime result. [Mac spike](../reports/m0/spikes/S1-follow-up-macos-wry-tao.md), [native log](../reports/m0/evidence/s1-followup-macos-native.txt), [overlay capture](../reports/m0/evidence/s1-followup-macos-overlay.jpg)

The two Mac boundary runs support a private chrome resource handler and typed dispatcher confined to the chrome WebView. The untrusted origin could not produce an additional native command effect, retrieve the sampled bundled file, or load the private chrome resource in the tested cases. This evidence is scoped to the synthetic fixture, tested engine and recorded operations. The actual product command set and other platforms remain open. [Boundary assessment](../reports/m0/S1_FOLLOW_UP_SECURITY.md)

Desktop follow-ups for Windows 11, GNOME X11 and GNOME Wayland were environment-blocked. Existing Server, Xvfb and Weston artifacts retain their original scope. WRY + TAO remains the first candidate. Recommend retaining it under evaluation with macOS the strongest supported candidate, Windows 11 native accelerator and recovery evidence pending, and separate GNOME X11 and Wayland validation still needed. A successful desktop and recovery follow-up, plus platform-specific privilege tests, would justify revisiting the recommendation. The owner retains the substrate and per-platform support decisions. [Updated capability matrix and handoff](../reports/m0/M0A_S1_FOLLOW_UP.md), [desktop inventory](../reports/m0/evidence/s1-followup-guest-inventory.txt)
