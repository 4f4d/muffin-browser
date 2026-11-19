---
spike: S8
platform: linux_x11
substrate: wry-tao-x64
budget_minutes: 45
started_at: 2026-11-19T17:33:19Z
deadline_at: 2026-11-19T18:18:19Z
stopped_at: 2026-11-19T17:38:25Z
stopped_reason: completed
outcome: pass
---

# S1 follow-up on linux_x11

## Method and result

Built an optimized Ubuntu x86-64 fixture in a network-isolated cached image and created a 547,910-byte .deb declaring the distro's GTK3, WebKitGTK 4.1 and X11 runtime dependencies. Its 1,859,248-byte ELF ran the synthetic feature workflow after extraction under both Xvfb and headless Weston, with 1,832 KiB allocated extracted footprint. The engine and JavaScriptCore packages used 93,416 and 31,624 KiB per package metadata in that image and were external to the archive. Full runtime closure, real GNOME interaction, clean package installation/upgrade/uninstall, other distributions, codecs and AppImage remain open. [Package, dependency and extraction receipts](../evidence/continuation-s8-linux-x64.txt), [package matrix](../S8_PACKAGE_RUNTIME.md)
