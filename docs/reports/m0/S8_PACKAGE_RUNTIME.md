# S8 package and runtime footprint

Sizes cover the built feasibility fixture and its selected archive, in bytes unless a different unit is shown. The fixture archives use system engines. The Windows EXE came from the debug runner build. The other fixtures were optimized. [Distribution sources](evidence/continuation-s8-distribution-sources.txt)

| Target | Executable | Download payload | Extracted fixture | Runtime observed |
|---|---:|---:|---:|---|
| macOS arm64 | 1,709,776 | 672,701 ZIP | 1,676 KiB allocated bundle | The unpacked Apple Silicon app ran the synthetic feature workflow on macOS 15.8.3. [Sizing](evidence/continuation-s8-macos-sizing.txt), [smoke](evidence/continuation-s8-macos-arm64-replay.txt) |
| macOS x86-64 | 1,719,280 | 695,150 ZIP | 1,684 KiB allocated bundle | The unpacked Intel slice ran the workflow under existing Rosetta on Apple Silicon. Intel hardware remains untested. [Sizing](evidence/continuation-s8-macos-sizing.txt), [Rosetta smoke](evidence/continuation-s8-macos-x64-replay.txt) |
| macOS universal | 3,446,480 | 1,366,894 ZIP | 3,372 KiB allocated bundle | lipo confirmed both slices and the ZIP passed extraction integrity checks. The separate architecture packages supplied the runtime checks. [Sizing](evidence/continuation-s8-macos-sizing.txt) |
| Windows x64 | 2,500,096 | 850,683 ZIP | 2,500,096-byte PE payload. Windows installed footprint unmeasured. | The archived EXE hash matches the successful Windows Server 2025 feature-fixture run with WebView2 156.0.4314.58. [PE and ZIP evidence](evidence/continuation-s8-windows-x64.txt), [Runner evidence](evidence/continuation-s4-windows-ci.json) |
| Windows ARM64 | Unmeasured | Unmeasured | Unmeasured | Native ABI, loader/runtime selection and execution need a dedicated ARM64 run. [Distribution sources](evidence/continuation-s8-distribution-sources.txt) |
| Ubuntu 24.04 x86-64 | 1,859,248 | 547,910 .deb | 1,832 KiB allocated in scratch extraction | The same extracted ELF passed the fixture under Xvfb X11 and headless Weston using system WebKitGTK 2.52.6. [Package and smoke evidence](evidence/continuation-s8-linux-x64.txt) |

## macOS

The fixture bundle contains an executable and Info.plist. The runtime comes from macOS, with AppKit, WebKit and other system frameworks dynamically linked. The Mach-O headers encode arm64 11.0 and x86-64 10.12 deployment values. Shipping floors require the selected runtime capabilities and real hardware tests. Developer ID signing, notarization, Gatekeeper behavior and installation/uninstallation remain unrun. [Dynamic dependencies and architectures](evidence/continuation-s8-macos-sizing.txt), [Apple universal binaries](https://developer.apple.com/documentation/apple-silicon/building-a-universal-macos-binary)

## Windows

Windows 11 ships with WebView2 Evergreen. A test installer still needs to detect the runtime and handle the missing-runtime case. Online Evergreen uses a small bootstrapper and later runtime download; offline deployments need the architecture-appropriate standalone installer. Microsoft distinguishes per-user and elevated per-machine installation. The Fixed Version distribution adds a vendor-documented runtime payload above 250 MB and shifts security updates to the app vendor. WRY's default runtime lookup uses the installed runtime. Fixed Version would require a separately supplied WebView2 environment. The ZIP excludes WebView2, VC runtime deployment, installer metadata and user data. Native Windows 11 ARM64 remains a separate build and runtime check. [PE imports](evidence/continuation-s8-windows-x64.txt), [Microsoft distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution), [GitHub runner availability](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)

## Linux

A .deb can delegate GTK3, WebKitGTK 4.1 and X11 libraries to the distribution package manager. The test archived the ELF only, declared runtime dependencies and executed an extraction in a preprovisioned Ubuntu image. dpkg installed-size metadata includes WebKitGTK 93,416 KiB and JavaScriptCoreGTK 31,624 KiB in that environment. GTK, X11/Wayland, GStreamer and their transitive runtime dependencies add footprint. Those measurements are selected package metadata. A clean install, full incremental dependency closure, media-codec support, other distributions, package upgrades and uninstalls remain unverified. A self-contained AppImage would need a separate portability and engine security-update plan and a measured payload. [Dependency inventory](evidence/continuation-s8-linux-x64.txt), [Ubuntu WebKitGTK metadata](https://packages.ubuntu.com/en/noble/libs/libwebkit2gtk-4.1-0), [AppImage GTK guidance](https://github.com/AppImage/AppImageKit/wiki/Bundling-GTK3-apps)

The measured fixture archives are below the desired download ceiling. Platform dependency downloads, bundled assets and the size of the final signed distribution remain separate engineering measurements. [Sizing artifacts](evidence/continuation-s8-distribution-sources.txt)
