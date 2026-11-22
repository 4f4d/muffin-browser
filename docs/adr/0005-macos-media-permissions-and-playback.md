# ADR 0005 — macOS media permissions and playback

**Status** Implemented for the macOS development browser  
**Date** 2026-11-23

## Context

WRY 0.57.2 delivers the permission kind to Muffin and handles the WebKit callback's origin and frame internally. Removing Muffin's handler would select WRY's grant fallback. WebKit's native prompt supports the requesting origin. Apple requires a bundle usage description for each capture type. [Pinned WRY UI delegate](https://github.com/tauri-apps/wry/blob/wry-v0.57.2/src/wkwebview/class/wry_web_view_ui_delegate.rs), [Apple capture callback](https://developer.apple.com/documentation/webkit/wkuidelegate/webview(_:requestmediacapturepermissionfor:initiatedbyframe:type:decisionhandler:)), [Apple bundle requirements](https://developer.apple.com/documentation/bundleresources/requesting-authorization-for-media-capture-on-macos)

## Decision

Trusted chrome denies permission requests. Content requests for camera and microphone use WebKit's native prompt when the corresponding app bundle usage description is present. Muffin denies other requests delivered to this handler. WebKit and macOS own the decision flow. Muffin keeps no independent site permission store and exposes no capture IPC to content. Revisiting a Muffin-owned permission dialog would require an adapter with the native frame and origin and an audited one-shot decision path. [Native policy](../../crates/muffin/src/main.rs), [development bundle](../../tools/bundle_macos.py)

Page Tools offers Pause all media and Resume all media for the selected content view using WebKit's playback suspension API. The app tracks the requested state per live WebView. Recreating the view starts with ordinary playback behavior. This control suspends playback, including video. Audio-only mute and the ME-04 acceptance decision remain open. [Apple playback suspension](https://developer.apple.com/documentation/webkit/wkwebview/setallmediaplaybacksuspended(_:completionhandler:)), [open decisions](../DECISIONS_PENDING.md)

Development prompt observation uses a disposable LaunchServices app bundle, a loopback origin, both usage strings, and a new bundle identifier. Capture grants, device operation, distribution entitlements, and release packaging require their own qualification. The mutual separation of bundled chrome and remote content still applies. [M4 handoff](../reports/m4/M4_PROGRESS.md), [chrome boundary](0003-trusted-chrome-and-content-webviews.md)
