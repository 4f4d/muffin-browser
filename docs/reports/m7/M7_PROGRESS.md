# M7 progress and qualification handoff

The owner deferred human testing on 2026-11-23 and authorized continued engineering. The target for local qualification is Apple Silicon macOS. M6 visual and accessibility acceptance, M0b qualification and release approval remain open. [Human testing checklist](HUMAN_TESTING_CHECKLIST.md), [owner decision](../../adr/0007-defer-human-qualification-during-m7.md)

## Candidate verification

From a clean Apple Silicon checkout, run `python3 tools/package_macos_qualification.py --out <new-directory>`. The command runs the engineering verifier and builds a local ZIP containing the release executable. It records source, binary and archive identity, checks the archive round trip, and tests that the extracted executable rejects debug control. Run `python3 tests/native/run_launchservices_smoke.py --verified-smoke <package-output>/native/smoke.json --out <new-directory>` to exercise macOS URL delivery with the verified debug binary and loopback URLs. The test leaves the system default browser preference alone.

## Open qualification

- Launch the distribution build from Finder and exercise OS default-browser selection, cold launch with a link, Gatekeeper, installation and removal in a disposable environment.
- Measure startup, first paint, CPU and physical memory across the application and its attributable WebKit processes against Safari in fixed scenarios. Include hardware, OS, engine and power conditions. Record installer and runtime dependencies separately.
- Confirm renderer termination and recovery with attributable WebContent evidence.
- Record the distribution identity, signing, notarization, update verification and rollback policy before distribution testing. The decision register tracks approvals and the remaining launch gates. [Decision register](../../DECISIONS_PENDING.md)
- Qualify native accessibility and input with the owner on Mac. Windows 11, GNOME X11 and GNOME Wayland retain their prior evidence and platform status. [Generated status](../../STATUS.json)
