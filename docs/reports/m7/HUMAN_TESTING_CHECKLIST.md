# Muffin human testing checklist

**Owner testing deferred.** Start with macOS Apple Silicon, the initial qualification target. Continue Windows and Linux when their target builds and desktop environments are available. Use the exact candidate binary for each batch. [Owner decision](../../adr/0007-defer-human-qualification-during-m7.md), [M7 candidate and verification](M7_PROGRESS.md)

## macOS visual and accessibility review

- [ ] Review light and dark chrome at regular and 640-point widths. Overflow the tab strip, switch tabs and open More. Check contrast, clipping, the active tab, Close, the adjacent New Tab button and all eight actions.
- [ ] Test keyboard traversal from chrome to page and back. Use address submission, New Tab, Private Window, tab search and Downloads shortcuts. Check arrow navigation, match selection, Escape and focus return.
- [ ] Check VoiceOver headings, landmarks, selected tab state, controls and live announcements. Include omnibox Enter submission, Downloads approval, private window identity, TLS error and Back to Start, and website-data confirmation.
- [ ] Check larger text or 200% zoom, Increase Contrast, Reduce Transparency and Reduce Motion. Review hit areas, focus visibility and layout at normal and compact widths.
- [ ] Record native Mac IME composition in the address field and a page input. Enter text, confirm candidates and check composition is not interrupted by shortcuts.
- [ ] Record visual and accessibility findings, OS and VoiceOver versions, and the checkpoint decision. [M6 screenshots and evidence](../m6/M6_CHECKPOINT.md)

## M0b qualification matrix

The source ledger contains **69 unrecorded cells**, including four conditional SI-05 cells. The regular queue is 15 macOS, 15 Windows, 17 GNOME X11 and 18 GNOME Wayland checks. A check box marks a pending applicable test. A dash marks a platform outside that case. Use the runbook for full acceptance criteria. [Manual runbook](../../M0B_MANUAL_TESTS.md#test-details), [source ledger](../m0/results.json), [generated status](../../STATUS.json)

| ID | Check | Mac | Windows 11 | GNOME X11 | GNOME Wayland | Environment or dependency |
|---|---|---|---|---|---|---|
| SI-01 | Google login and Gmail after restart | ☐ | ☐ | ☐ | ☐ | Throwaway account |
| SI-02 | Sign in with Google on three unrelated sites | ☐ | ☐ | ☐ | ☐ | Throwaway account and live provider sites |
| SI-03 | Microsoft sign-in and SSO redirect | ☐ | ☐ | ☐ | ☐ | Throwaway account |
| SI-04 | GitHub sign-in and 2FA | ☐ | ☐ | ☐ | ☐ | Throwaway account |
| SI-05 | User-agent diagnostic | conditional | conditional | conditional | conditional | Run only after relevant SI-01 or SI-02 failure. Use the prescribed diagnostic. |
| PK-01 | Platform passkey creation and sign-in | ☐ | ☐ | ☐ | ☐ | Bare metal and platform authenticator |
| PK-02 | USB security key sign-in | ☐ | ☐ | ☐ | ☐ | Bare metal and USB key |
| ME-01 | HD video, seek and fullscreen | ☐ | ☐ | ☐ | ☐ | Real playback |
| ME-02 | DRM subscription service and public test page | ☐ | ☐ | ☐ | ☐ | Bare metal and an existing authorized account |
| ME-03 | H.264/AAC, VP9, AV1 and Opus | ☐ | ☐ | ☐ | ☐ | Video and audio output |
| ME-04 | Tab audio indicator and audio-only mute | ☐ | ☐ | ☐ | ☐ | Record partial when applicable. Pause all media has different behavior. |
| DL-01 | Large and authenticated downloads, quarantine or Mark-of-the-Web | ☐ | ☐ | held | held | Linux destination failure is held pending engineering evidence. |
| UP-01 | Native upload dialog | ☐ | ☐ | ☐ | ☐ | Disposable file |
| PD-01 | PDF open, scroll, print and download | ☐ | ☐ | ☐ | ☐ | Native interactions |
| PO-01 | Live popups, opener and callback flows | ☐ | ☐ | ☐ | ☐ | Live providers and throwaway accounts |
| LX-01 | Wayland child-view geometry, focus, resize and shortcuts | — | — | — | ☐ | Interactive GNOME Wayland |
| LX-02 | Fractional scaling at 125% and 150% | — | — | ☐ | ☐ | Bare metal with a display |
| LX-03 | IME composition in chrome and content | — | — | ☐ | ☐ | Interactive GNOME with an installed IME |
| LX-04 | GPU versus software rendering | — | — | ☐ | ☐ | Bare metal with a suitable GPU and native captures |
| HD-01 | HiDPI and mixed-DPI window and input behavior | ☐ | ☐ | — | — | Bare metal with suitable displays |

Use Windows 11 or separate interactive GNOME X11 and Wayland sessions. VMs can cover function where the runbook allows them. The 18 bare-metal cases require physical hardware. Windows Server, headless Ubuntu Server, Xvfb and headless Weston are useful engineering environments. Desktop behavior needs the named desktop session and physical GPU claims need physical hardware. [Platform follow-up](../m0/M0A_S1_FOLLOW_UP.md), [Linux download hold](../m0/M0B_PROGRESS.md#linux-dl-01-investigation)

## Packaged application and device checks

- [ ] On the Mac candidate, check actual camera and microphone prompts and attributed allow and deny behavior with the intended devices. Include a private window, prompt origin, and whether reopening a window preserves permissions. Record the system and site decisions separately.
- [ ] On the candidate, choose and cancel a download destination, approve or cancel a download, reveal the saved file, and open a supported file. Check the real file bytes and quarantine. Exercise native upload selection and cancellation, system print panel and context menus. Preserve the original M0b overlap in its existing rows.
- [ ] Install the eventual distribution candidate in a disposable user environment. Inspect Finder and Gatekeeper behavior, launch from Finder, choose Muffin as default browser through the OS, and exercise HTTP and HTTPS from another app with Muffin closed and running. Check that a private window does not receive external links. Restore the prior default.
- [ ] On a disposable installation, relaunch after a crash and confirm regular tabs, bookmarks and history recover as intended. Check preservation and cleanup of disposable profile data on upgrade, rollback and removal. Review what survives in OS or WebKit caches and crash diagnostics, including private-mode artifacts and explicitly saved files.
- [ ] When the owner approves the signing and update path, verify signature, notarization, quarantine, update and rollback on the actual distribution artifact. Repeat system integration and removal on the packaged build. Record any warnings shown by macOS.
- [ ] On Windows 11, check native accelerators with content focused, focus transitions, keyboard input, renderer recovery and mixed-DPI behavior. On both GNOME sessions, check initial and resized geometry, overlay order, focus, keyboard and IME, and renderer recovery. Use bare metal for the scaling and GPU checks above. [S1 desktop checklist](../m0/M0A_S1_FOLLOW_UP.md#platform-capability-matrix)

## Evidence and decisions

For each completed case record source commit, binary SHA-256, host and binary architectures, exact OS and engine versions, tester, date, outcome and redacted evidence. Keep original recordings and account details in the gitignored evidence directory. Append actual M0b results to the source ledger. Keep open cases unrecorded. Use the M6 checkpoint for visual and accessibility results. [Recording instructions](../../M0B_MANUAL_TESTS.md#recording-a-result)

Owner decisions remain open for M6 and M0b acceptance, substrate, minimum OS and runtime versions, DEC-UA and sign-in, media scope, platform eligibility, signing, update policy and release. Content blocking, HTTPS-first fallback and external app protocols need separate policy and validation before they can be qualified. [Decision register](../../DECISIONS_PENDING.md)
