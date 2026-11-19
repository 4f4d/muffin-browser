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
