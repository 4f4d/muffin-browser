# M0b preparation from M0a

The local diagnostic kit is staged under `docs/reports/m0/evidence-private/m0b-prep-2026-11-19/`. Its manifest records source commits, executable and archive hashes, architecture and exact sizes. Only the manifest and generator are tracked. The archives are local feasibility builds. [Manifest](evidence/continuation-m0b-prep-manifest.json), [S8 package evidence](S8_PACKAGE_RUNTIME.md)

| Material | Use |
|---|---|
| macOS arm64, x86-64 and universal S4 ZIPs | Extract the selected app and run the documented S4 launcher against its executable. The native arm64 and translated x86-64 slices completed a package smoke. |
| Windows x64 S4 ZIP | Extract the EXE on a suitable system with WebView2 installed. Use the launcher below to collect runtime and synthetic feature evidence. Windows Server results are already available. The archive carries no installer. |
| Ubuntu x86-64 S4 .deb | Extract with `dpkg-deb --extract <package> <scratch-directory>`. The fixture ELF is under `usr/lib/muffin-s4-fixture/`. Use a legitimate desktop session with GTK3/WebKitGTK 4.1 installed for further desktop validation. |
| Media, PDF and upload assets | The media folder has two-second H.264/AAC, VP9, AV1 and Opus samples, an inline PDF and plain-text upload stimulus. ffprobe confirmed codec identities. Playback, print and upload acceptance remain with the human. |

For a standalone synthetic diagnostic on any tested architecture, give the launcher the executable from the archive and an empty scratch directory. The Python launcher starts its own loopback server; the output includes native logs, HTTP events and a structured summary.

```sh
python3 spikes/s1/followup/s4_fixture.py --binary <extracted-executable> --work-dir <new-scratch-directory> --timeout-seconds 80
```

For the static test assets, use the local media directory only. A compatible FFmpeg build can recreate these stimuli using the tracked recipe.

```sh
sh spikes/m0b-prep/make_media.sh <new-media-directory>
python3 -m http.server 8778 --bind 127.0.0.1 --directory <media-directory>
```

The media page is at `http://127.0.0.1:8778/` when the human starts the server. Use an appropriate browser-candidate build that can navigate to that page. The S4 binaries use their own fixture-only URL policy.

The full manual matrix still requires a general-browsing test build with safe navigation and popup controls, appropriate systems, real provider workflows and the listed hardware. The fixture kit covers selected synthetic preparation and can help plan DL-01, PD-01, ME-03 and UP-01. It supplies no full acceptance result for those cells. Preserve the existing requirements for account/provider tests, passkeys, protected content, desktop sessions and bare-metal-only cases. All result cells remain for the human to record with the exact tested executable and source commit. [Manual runbook](../../M0B_MANUAL_TESTS.md), [status](../../STATUS.json)
