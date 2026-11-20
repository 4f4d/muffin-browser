# Muffin Browser — Product and Engineering Plan

**Plan status:** Revision 3. Production implementation authorized. Substrate remains provisional.
**Plan date:** 2026-11-20
**Product:** Muffin Browser
**Initial release target:** macOS Apple Silicon
**Deferred release targets:** Windows, Linux X11 and Linux Wayland

The owner authorized M1 and subsequent implementation milestones ahead of manual M0b qualification. [Owner decision and gates](adr/0002-macos-first-implementation-and-deferred-qualification.md)

## 1. Product thesis

Muffin is a small, fast, thoughtfully designed browser for everyday browsing, reading, research, and study. It should be approachable to a non-technical user and capable enough to serve as a regular browser for users whose needs fit its supported feature set.

The product should feel calm, refined, warm, responsive, and coherent. It should not look childish or imitate Chrome's interface wholesale. The bakery identity should be subtle: brand mark, illustration, and occasional copy—not pastry decorations throughout the UI.

The initial release is a **focused daily-use browser**, not a promise of Chrome feature parity. Be explicit about limitations involving extensions, DRM, authentication, passkeys, notifications, and engine differences.

## 2. Non-negotiable product principles

1. Reliability before feature count.
2. Lightweight architecture, measured by the complete process tree and installed footprint—not executable size alone.
3. A consistent Muffin chrome/design system across platforms, while respecting native window controls and unavoidable engine differences.
4. Local-first storage; no account, cloud sync, analytics, or telemetry in v1.
5. Privacy and security by default.
6. Honest per-platform feature parity and release labels.
7. Keyboard accessibility, sensible defaults, and an uncluttered interface.
8. No feature is advertised until it has executable or documented manual verification evidence.

## 3. Initial technical hypotheses — validate in M0

### Application and browser engine

Candidate A: Tauri 2 with its child-WebView API. This is attractive for packaging, permissions, and application infrastructure, but child WebView creation is currently documented as requiring the `unstable` Cargo feature. Treat API stability and platform behaviour as risks; verify the exact versions pinned in the project. Official docs confirm child-WebView creation still sits behind the `unstable` feature (checked 2026-11-18). A secondary source (needs-verification) reports that stock Tauri lays child WebViews out in a GTK box that ignores their bounds on Linux. Until S1 shows otherwise, treat Tauri-on-Linux as presumed failing and spike WRY + TAO with the GTK/`gtk::Fixed` path first on Linux.

Candidate B: Rust core using WRY + TAO (or another justified native windowing layer), with explicit native integration where WRY is insufficient. WRY's usual child-window path is documented as X11-only on Linux; its GTK construction path is the route to investigate for both X11 and Wayland. Validate resizing, focus, event-loop integration, z-order, lifecycle, and multiple views.

Candidate C: another architecture only if research and M0 evidence justify it. Do not bundle Chromium/CEF merely to make the rendering engine uniform; calculate its actual download and installed footprint, process memory, update burden, and maintenance cost before reopening this choice.

**Decision rule:** Do not choose a substrate based on framework familiarity. Run comparable spikes on macOS, Windows, Linux X11, and Linux Wayland. Select the simplest architecture that passes the browser's required native multi-WebView, security, and lifecycle tests. If a platform fails, consider platform-specific beta status or narrower v1 support before changing the entire stack.

### Browser chrome and frontend

Candidate: Svelte 5 + TypeScript + Vite with CSS variables and a shared component system, rendered in a trusted local chrome WebView. Compare alternatives only if there is a concrete technical reason. Keep the chrome frontend lean; avoid expensive blur, always-running animation, excessive dependencies, and remote font/script loading.

### Native core and persistence

Candidate: Rust owns tab/session/history/bookmark/download/permission/settings state. Candidate persistence layer: SQLite via `rusqlite`, WAL mode, local FTS5 history search and a transparent frecency score. Secrets belong in OS credential storage, not SQLite plaintext. Keep storage APIs behind a native service boundary.

### IPC

Define a typed, versioned command/event contract before parallel implementation. The trusted chrome may call a narrow set of commands. Remote browsing WebViews must not receive the app's privileged IPC bridge, Tauri capabilities, local file access, or native command surface. A debug-only test-control channel must be isolated from release builds and inaccessible to web content.

## 4. Platform policy

M0 must define and record:
- Minimum supported macOS version and WebKit baseline.
- Windows versions and WebView2 Evergreen/runtime policy.
- Linux distribution/version baseline, WebKitGTK baseline, GTK dependencies, and package formats.
- Supported CPU architectures for each release artifact.
- Wayland and X11 status separately.
- GPU/software-rendering behaviour and known driver limits.

Do not claim Linux desktop-wide support from Ubuntu/X11 alone. Include Wayland checks. Do not claim Windows ARM64 or Linux ARM64 support unless built and tested on those architectures.

Expected environment (audit the actual one): an Apple Silicon macOS development machine where Codex runs, plus a hypervisor-hosted Windows 11 VM and Ubuntu *Desktop* VM (GNOME, with both Wayland and X11 sessions) that the human operates. A headless Ubuntu Server host cannot test X11/Wayland GUI behaviour and does not count as a Linux desktop target. Codex cannot drive the VMs directly; Windows and Linux evidence comes from GitHub-hosted runners (public repository, standard runners) and from human-run tests recorded per `docs/M0B_MANUAL_TESTS.md`.

Virtualized runs validate functionality only. Do not use them as evidence for GPU/Wayland rendering behaviour, DRM, hardware-authenticator passkeys, or performance. Use bare-metal runs or native CI runners for platform builds/smoke tests, and treat CI performance numbers as indicative only because shared runners are noisy.

## 5. Feature scope

### Launch scope — qualify the relevant platform capabilities before release

**Core navigation**
- Address/omnibox with URL recognition and search fallback.
- Back, forward, reload, stop, page-load indication, home/new-tab.
- Search engine selection; local history/bookmark autocomplete and frecency ranking.
- Optional remote search suggestions, disabled by default and clearly disclosed.
- Native Cmd/Ctrl shortcuts and menu accelerators.
- Committed-URL display, secure-origin indicator, error/offline page.

**Tabs and windows**
- Multiple tabs, create/close/switch, reorder, duplicate, pin, tab search, tab context menu.
- Loading, crash, audio/media, and private-tab state indicators where reliably detectable.
- Restore recently closed tabs and last session using Muffin's own navigation metadata.
- Crash/termination recovery.
- Single-instance behaviour and forwarding external links to the running instance.
- Default-browser registration and HTTP/HTTPS URL scheme handling.
- Lazy creation and measured background-tab discard/restore. Use conservative rules; never claim unsaved form state can always be detected. Never discard active media/calls/downloads or a tab with known active capture. Provide user control and predictable restoration.

**Personal library**
- Bookmarks, folders, search, import/export (HTML format).
- History search, individual deletion, clear by time range, and optional clear-on-exit.
- New-tab page with configurable shortcuts/bookmarks and recently visited sites, with privacy-aware controls.

**Downloads and page tools**
- Download manager with progress, cancel, retry by restarting when safe, open and reveal, duplicate filename handling, location setting, and clear-list action independent of deleting files.
- Do not promise pause/resume until proven on every shipped platform. On WebKitGTK, the current documented `WebKitDownload` API exposes progress and cancellation but not a pause/resume operation; verify the exact baseline used.
- Find in page, per-site zoom, fullscreen, print, safe file upload/download, copy link/address, context actions, and PDF strategy.
- Use bundled pdf.js only if native PDF viewing is not reliable on a target platform; measure its size and maintain its licensing notices.

**Privacy and security**
- Private windows with isolated temporary storage and a verifiable no-profile-persistence test.
- Clear browsing data and site data controls.
- Site permissions for APIs actually exposed by the engine; explain unsupported permissions.
- Cookie controls and HTTPS-first only if implemented correctly with the relevant engine/network hooks.
- Basic content blocking only after an M0 capability spike. Record filter-list licenses and attribution; do not assume EasyList or any list is permissively licensed.
- No silent TLS bypass. External protocol launches require validation and an appropriate user gesture/confirmation.

**Settings and appearance**
- Light, dark, and system modes.
- Search engine, homepage/new tab, downloads, startup/session restore, privacy, permission, and tab behaviour settings.
- Accessible keyboard focus, target sizes, labels, contrast, and reduced-motion support.

### Launch only with feasibility and qualification evidence
- Reliable Google/Microsoft/GitHub sign-in and common OAuth flows.
- Passkey/WebAuthn access, including the Apple entitlement and approval path where applicable.
- DRM streaming compatibility on representative services and OS versions.
- Cross-platform content blocking.
- Native password-manager/autofill integration.
- Web notifications and picture-in-picture.

Do not make a failing capability a launch promise.

**Sign-in is a product gate, decided by the human.** The pass criteria are fixed in advance (see `docs/M0B_MANUAL_TESTS.md`, tests SI-01 to SI-04): direct Google, Microsoft, and GitHub login, and "Sign in with Google" on three third-party sites. Google's policy restricts OAuth in embedded user agents, but how it treats a general-purpose browser is an empirical question. Record results; do not guess.

**User-agent policy (human decision, tracked as DEC-UA).** Until the human records a decision, Muffin ships the engine's default user agent plus a truthful `Muffin/<version>` product token and must not impersonate another browser. Codex must not alter UA strings to change provider behaviour. The human may run a clearly labelled, never-shipped diagnostic (SI-05) to learn whether blocking is UA-based, and decides what to do with the result. An external-browser handoff does not solve this for a browser: a site's own sign-in popup has nothing to hand off to.

### 1.x candidates
- Reader mode.
- Split view.
- Tab groups and workspaces.
- Vertical tabs (prototype against horizontal tabs before deciding default).
- Per-site settings page and improved site controls.
- Picture-in-picture where the platform API supports it.

### Deferred / excluded from v1
- Browser extension ecosystem and extension compatibility claims.
- Built-in password vault.
- Cloud sync or account system.
- Multiple user profiles unless a strong use case and storage-isolation plan emerge.
- Download pause/resume unless a reliable portable implementation is proven.
- Custom developer-tools UI; use the engine's supported inspector for development/advanced troubleshooting where available.
- Custom browser engine, bundled Chromium, telemetry, AI assistant, and advertising.

## 6. UX and visual design

### Brand direction

Name: Muffin Browser. Use a mature, understated bakery/café identity. Avoid cartoon pastry UI, excessive orange, novelty typography, noisy gradients, and decorative glass effects.

Starting palette, subject to contrast testing:
- Warm cream `#F7F3EC`
- Soft ivory `#FFFDF9`
- Espresso `#342820`
- Cocoa `#655044`
- Toasted caramel `#C58B55` (accent fill only until contrast-tested)
- Muted peach `#E7BDA3` (decorative/surface fill only)
- Warm grey `#8D8982` (not body text until verified)

Do not use caramel or warm grey as small text/focus colour on cream unless measured contrast passes. Generate and verify all light/dark token pairs. Target WCAG 2.2 AA where applicable: normal text 4.5:1, large text 3:1, and UI components/focus indicators 3:1 against adjacent colours.

Typography: choose a high-quality, openly licensed UI typeface only after checking license and script coverage. Bundle a small subset only if worthwhile; use system fallbacks for unsupported scripts. Never fetch fonts from a CDN. Use one coherent icon family as inline SVG.

Window chrome: preserve native traffic lights on macOS and begin with native decorations on Windows/Linux. Do not break Windows 11 snap layouts or Wayland behaviour with custom caption controls. Shared identity applies to the browser chrome; system title bars, file dialogs, engine form controls, scrollbars, and context menus may necessarily differ.

The owner requested a premium, macOS-native feel with Liquid Glass-inspired controls and edges. Use subtle depth and highlights on the navigation layer, with a quiet page surface and warm tint. Preserve legibility and keyboard focus in light, dark, increased-contrast and reduced-transparency settings. Prefer system material where the target OS and native composition path support it. Qualify actual window composition and measure rendering costs before expanding effects. [Apple material guidance](https://developer.apple.com/design/human-interface-guidelines/materials)

Use a tonal surface and slightly stronger type for the selected tab. Reserve a visible ring for keyboard focus and an edge for the increased-contrast setting. Inspect the native chrome before keeping decorative highlights on compact controls.

### Design workflow

Do not create a large detached mockup waterfall. Build the real chrome stack and a component-gallery route first. Deliver four initial visual checkpoints: browser frame in light/dark, new-tab page, settings, and downloads panel. Capture screenshots at normal, narrow, and high-DPI sizes. Then iterate using the actual app and platform screenshots. A chrome-only screenshot does not prove content-WebView z-order.

Explore horizontal and collapsible vertical-tab prototypes. Keep the initial interaction model familiar and uncluttered. Pick a default from usability evidence, not personal preference alone.

## 7. Architecture risks that must be spiked

1. **Multi-WebView substrate:** create/destroy multiple content views, size/position, focus, keyboard routing, event handling, and memory impact.
2. **Z-order/overlays:** omnibox suggestions, dropdowns, permission sheets, context menus, and dialogs must appear above native child content. Test the selected overlay strategy on all platforms.
3. **Navigation/popup semantics:** `window.open`, `target=_blank`, OAuth popup flows, `window.opener`, redirects, downloads from popups, and external links.
4. **Authentication:** Google, Microsoft, GitHub, common SSO, WebAuthn/passkeys, third-party password managers, and cookie/redirect behaviour.
5. **Media:** codec playback, DRM services, fullscreen, PiP, autoplay, and per-tab audio controls.
6. **Downloads/uploads:** destination selection, progress, cancellation, duplicate names, safe file handling, quarantine/Mark-of-the-Web, authentication cookies, and large files.
7. **PDFs:** native support or pdf.js fallback, print, download, and file-origin security.
8. **Permissions:** camera, microphone, location, notifications, clipboard, downloads, and geolocation as exposed by each engine.
9. **Privacy:** independent normal/private stores; clearing data; disk writes; process termination and crash recovery.
10. **Linux:** X11 and Wayland, fractional scaling, IME, GPU and software rendering, WebKitGTK/GStreamer package dependencies.
11. **Lifecycle:** WebView process termination/crash, application restart, single instance, URL handler invocation, default-browser registration.
12. **App distribution:** signing, notarization, SmartScreen reputation, package formats, updater security and key management.

## 8. Milestones and gates

### M0a — Automatable feasibility spikes (Codex)

Create a `/spikes` area, not production architecture. Every spike report lives in `docs/reports/m0/spikes/` and uses `_TEMPLATE.md`. Before starting, commit an initial header containing `spike`, `platform`, `substrate`, `budget_minutes`, `started_at`, and `deadline_at`; omit the stop fields until the spike ends. The generator accepts the committed initial header as `in_progress`. After that report commit, run `python3 tools/gen_status.py`, run `python3 tools/gen_status.py --check`, and commit the refreshed `docs/STATUS.json` before starting timed work. After stopping, fill `stopped_at`, `stopped_reason`, and `outcome`, complete the report, and commit the final version. Then regenerate/check/commit `docs/STATUS.json` again. The completed report must be tracked, clean, and represented by at least two distinct commits. The initial header commit must be before and within five minutes of `started_at`; the final report commit must be after and within five minutes of `stopped_at`. Git commit timestamps are corroborating metadata, not a cryptographically trusted clock; the reviewer must inspect the commits and supporting artifacts. The pre-commit hook defers its generated-status check only while an M0a spike report is staged, because that commit is not visible in Git until the hook completes; immediately regenerate and validate `STATUS.json` after the commit. Place raw M0 artifacts under `docs/reports/m0/`. Budgets are hard wall-clock caps for active work: setup, dependency downloads, builds, CI waits, and failed attempts count. At the deadline, stop and report partial findings. Do not silently extend or restart a spike; an additional budget requires explicit human approval and a documented reason (`extension_minutes`, `extension_approved_by`, `extension_reason` in the report header). The generator rejects missing/untracked/dirty reports, insufficient or mistimed Git history, inconsistent stop-reason/outcome pairs, and per-spike and aggregate overruns. **Outcome rules:** a deadline stop must be `inconclusive`; a blocked stop must be `blocked` or `inconclusive`; `outcome: blocked` requires `stopped_reason: blocked`; `outcome: fail` requires `stopped_reason: completed`. A completed S1 failure on macOS or Windows triggers the kill rule; a timeout never does. The per-spike and aggregate budgets sum to about 34 hours of agent wall-clock if every row is used in full. Produce concise ADRs and a platform capability matrix. Run in this order:

| Spike | Hard time budget | Stop condition |
|---|---:|---|
| Engine floor | 45 minutes per available platform; 3 hours aggregate | Record the bare-WebView result or mark the platform `not_run`/blocked with evidence; do not simulate another OS. |
| S1 multi-WebView shell | 90 minutes per substrate/platform combination; 12 hours aggregate | Record create/destroy, bounds, focus, z-order, overlays, event loop, crash, shortcuts, and basic navigation results; stop at the cap. |
| S2a popup/`window.opener` fixtures | 60 minutes | Record fixture outcomes and unresolved cases. Live provider flows remain M0b. |
| S3 native hooks inventory | 60 minutes | Record current WRY API coverage and the specific native hooks still required. |
| S4 downloads/PDF/content-blocking/private-profile feasibility | 90 minutes per platform; 6 hours aggregate | Record per-engine capability and gaps; do not implement production features. |
| S5 comparative baselines | 60 minutes per platform; 4 hours aggregate | Record available startup, shell memory, 1/5/10-tab process-tree memory, and idle CPU data; unavailable measurements remain `not_run`. |
| S7 test-harness and CI artifact feasibility | 60 minutes | Demonstrate the smallest viable fixture/control/screenshot path or report the blocker. |
| S8 package/runtime footprint | 45 minutes per platform/architecture; 4 hours aggregate | Record installer/package and runtime-dependency footprint or mark unavailable. |
| Windows/Linux CI workflow setup | 60 minutes per workflow; 2 hours aggregate | Get the minimal workflow running or report the exact blocker. |

1. **Engine floor (first).** A bare single-window WebView on `about:blank` on each available platform, measured for startup, process-tree memory, and package/installed footprint. Muffin's own overhead is the delta above this floor. Everything later is judged against it.
2. **S1: multi-WebView shell**, WRY + TAO (GTK path on Linux) and Tauri child WebViews, on macOS, Windows, and Linux X11, then Linux Wayland. Test 1/5/10/20 create-destroy cycles, resize, focus, z-order, overlays, event loop, crash, keyboard shortcuts, and basic navigation using controlled fixtures.
   **Kill rule:** if S1 *completes and fails* on macOS or Windows (not a timeout, which is `inconclusive` and goes to the human), stop all other M0 work and report. If it fails only on Linux, mark Linux beta/deferred and continue.
3. After S1 chooses the leading substrate, run S2a, S3, S4, S5, S7, and S8 against that substrate only; do not fully benchmark both candidates. Apply each row's hard budget.
4. Write CI workflows for Windows and Linux runners so the human-run M0b scripts and Codex's automated spikes use the same binaries, within the workflow budget above.

**Hard stop after M0a.** Write `docs/reports/m0/M0A_SUMMARY.md` (architecture recommendation, capability matrix, baselines, risks) and halt. Do not begin M1.

**2026-11-19 execution checkpoint.** The engine-floor and S1 findings, including the mixed substrate evidence and held follow-on spikes, are in the [M0a handoff](reports/m0/M0A_SUMMARY.md). The product and platform decisions remain with the human.

**2026-11-19 authorized S1 follow-up.** The [separate scope and timing record](reports/m0/S1_FOLLOW_UP_SCOPE.md) bounds four WRY + TAO desktop investigations. The [updated capability matrix](reports/m0/M0A_S1_FOLLOW_UP.md) records interactive Mac input and composition evidence, the Mac fixture trust boundary, and blocked Windows 11 and GNOME desktop investigations. The S1 gate remains open for human review.

**2026-11-19 authorized M0a continuation.** The owner provisionally authorized WRY + TAO for the remaining feasibility investigations. S2a, S3, S4, S5, S7 and S8 are recorded with their separate budgets and evidence. The S1 desktop and unexpected-recovery gaps remain open. The [continuation handoff](reports/m0/M0A_CONTINUATION_SUMMARY.md) and [prepared manual diagnostics](reports/m0/M0B_PREPARATION.md) are ready for human review. Architecture, platform support and M0b results remain open. At that checkpoint the repository stayed at M0a pending the human-operated stage. The 2026-11-20 owner decision below supersedes that implementation hold.

### M0b — Human-in-the-loop validation (human)

M0b is human-operated, not an autonomous Codex milestone. Codex may prepare test pages/binaries and validate or summarize records when asked, but it must not execute tests requiring the human, fabricate results, or fill blank cells.

The human runs `docs/M0B_MANUAL_TESTS.md` on bare metal where required, and on the Windows VM / Ubuntu Desktop VM where VM results are valid, then records results in `docs/reports/m0/results.json`. Codex prepares the spike binaries and test pages beforehand and, on request, reviews the recorded results. Blank cells stay `not_run`.

**2026-11-20 owner decision.** Defer the 15 regular macOS tests and conditional SI-05 diagnostic until feature-complete qualification. Leave unperformed cases unrecorded. Windows, Linux X11 and Linux Wayland are deferred from the initial release. Preserve their prior implementations and evidence. [Owner decision](adr/0002-macos-first-implementation-and-deferred-qualification.md)

**M0 acceptance (owner gate before initial release):** architecture decision, target OS and engine floors, feature parity table, risk register, measured baselines, M0b results and a platform decision for the initial release. Implementation may proceed before this gate. Qualification retains the original navigation, multi-view, security and sign-in criteria.

### M1 — Walking skeleton and verification harness

The owner explicitly authorized implementation on 2026-11-20. WRY + TAO is provisional and macOS Apple Silicon is the production target. Preserve native Windows and GTK feasibility coverage through their existing workflows. Record platform evidence according to the platform and test level actually exercised.

- Establish repository structure and typed IPC contract.
- Make CI write `ci_runner` result records (run URL, artifact hash) so agents never hand-write platform-level evidence. Platform-level artifact files must be committed under `docs/reports/ci/` or `docs/reports/platform/`; do not use ordinary project files as evidence. Add a Dependabot `github-actions` config so action pin bumps arrive as reviewed pull requests (update `tools/tests/test_ci_pins.py` with each reviewed bump).
- Build one-tab trusted chrome plus one remote content view.
- Implement native shortcuts, load state, committed URL handling, and external navigation guard.
- Create the local fixture server and test-control bridge (debug/test only).
- Create CI builds on native macOS, Windows, and Linux runners; add Wayland coverage where feasible.
- Add screenshot artifacts and a `just verify` command.
- Build design tokens and component gallery; first visual checkpoint is a human taste gate, but safe implementation work may continue using the documented palette if no feedback is provided.

### M2 — Tab and window core

- Multi-tab state model, create/close/switch/reorder/pin/duplicate/search.
- Lazy content WebView creation and conservative discard/restore.
- Native keyboard accelerators, popup handling, single-instance, URL forwarding.
- Crash recovery and session restore based on Muffin-owned navigation metadata.

### M3 — Local data and omnibox

- SQLite persistence, migrations, history, bookmarks/folders, import/export.
- Omnibox local ranking, settings, new-tab shortcuts.
- Clear data by time range and session behaviour.

### M4 — Core page tools and downloads

- Downloads, uploads, find-in-page, zoom, fullscreen, print, context menus, PDF path.
- Permission prompt integration and media controls where supportable.

### M5 — Privacy and security

- Private windows with temporary store and persistence tests.
- Cookie/site data controls, content blocking if validated, HTTPS-first if validated.
- Security hardening, external protocol safeguards, download protection, dependency review.

### M6 — Product polish and accessibility

- Final themes, responsive layouts, empty/loading/error states, keyboard traversal, accessibility, visual consistency and onboarding.
- Human visual review checkpoint with actual application screenshots on the initial release target. Other platforms require their own visual evidence before a later release. Do not claim visual verification without the artifacts.

### M7 — Hardening and release readiness

- Regression, security, performance and packaging review.
- Install/uninstall, default-browser, URL scheme, update and crash recovery tests.
- Sign/notarize only with explicit user-provided credentials and approval. Unsigned beta artifacts may be generated with clear installation caveats.
- At feature completion, prepare a consolidated macOS qualification plan and resume the original deferred manual matrix. Final readiness requires the owner's M0b acceptance and release decision.
- Final documentation, known limitations, platform support matrix and release notes.

## 9. Verification model

### Test layers

- Unit: URL classifier, session model, state transitions, storage, migrations, frecency, permissions, download metadata.
- Chrome UI: Playwright component and browser-chrome tests; axe-core or equivalent accessibility checks.
- Native integration: debug-only local control channel to drive/query WebViews and inspect URL/title/loading state; never expose this channel in release builds or to remote content.
- Deterministic fixtures: redirects, authentication, file upload/download, slow/failed responses, TLS errors, popups, cross-origin navigation, large pages, media, forms and local storage.
- Real websites: non-blocking canary suite; avoid brittle live-site assertions in the blocking CI path.
- Manual matrix: DRM, passkeys, IME, fractional scaling, GPU/Wayland edge cases, and any platform feature CI cannot reproduce.

### CI and test integrity

`just verify` must run formatting, lint, unit tests, integration fixtures, security/dependency checks, and build steps applicable to the runner. Generate status from actual results. A platform that only compiles is **build-verified**, not functionally verified.

Changes to tests, thresholds, fixtures or the verify script need a rationale and independent reviewer approval. Preserve historical benchmark data. A flaky real-site canary must not be made a blocking test without a reproducible fixture.

### Completion labels

For each OS/architecture, label:
- **Build verified:** correct artifact builds.
- **Smoke verified:** installs/launches and passes a basic navigation workflow.
- **Functionally verified:** relevant launch acceptance suite passes.
- **Release verified:** packaging, signing policy, upgrade/uninstall and release checks pass.

Only use “supported” for the level actually achieved. Mark Linux X11 and Wayland separately.

## 10. Performance methodology

No absolute startup or idle-RAM number is a requirement until M0 records a baseline. Use relative comparison in fixed scenarios against Safari on macOS, Edge on Windows, and Epiphany on Linux where available. Report methodology and avoid cherry-picked configurations.

Measure separately:
- Shell-only startup to interactive.
- First content paint.
- Shell memory and total browser process-tree memory.
- 1/5/10-tab memory with fixed fixture pages.
- CPU at idle and during fixed navigation workloads.
- Time and memory to discard/restore tabs.
- Installer/download size and installed footprint including external runtime dependencies where relevant.

Use OS-appropriate metrics (e.g. macOS physical footprint, Windows private bytes/working set with method documented, Linux PSS). Include OS/runtime/WebView versions, machine specs, power mode, commit SHA and raw JSON. Virtualized runs are functional checks, not representative performance benchmarks.

## 11. Security and privacy acceptance requirements

- Remote content WebViews have no privileged native IPC or Tauri capabilities.
- Chrome content is bundled locally and has restrictive CSP; no remote scripts/fonts.
- Address bar shows committed URL, handles IDN/punycode and bidirectional control characters safely.
- Restrict dangerous schemes (`javascript:`, unexpected `file:`, internal schemes); external protocols require a controlled user gesture and prompt as appropriate.
- Do not silently bypass invalid certificates; provide a clear interstitial and no global persisted exception by default.
- Downloads use safe destination selection, sanitized names, no automatic execution/opening, and platform quarantine/Mark-of-the-Web where applicable.
- Private browsing uses a separate ephemeral store; after close, verify no Muffin profile history, cookies, cache or session records remain. Explain unavoidable OS/engine crash/diagnostic artefacts if any.
- No password vault in v1. Never store passwords/cookies/tokens in application logs or SQLite plaintext.
- Run dependency/license audits (`cargo-deny`, `cargo-audit`, frontend lockfile audit or current equivalents), pin CI actions, and fuzz URL/omnibox parsing where practical.
- Add a threat model and independent security review before release.

## 12. Research and documentation

Use official documentation for APIs and policies; GitHub issues for implementation caveats; community forums/Reddit for user sentiment. Label source date and confidence. Do not convert a few posts into a quantitative popularity claim.

Maintain only documents that affect execution:
- `AGENTS.md`
- `docs/PLAN.md`
- `docs/STATUS.json` (generated)
- `docs/RESEARCH_NOTES.md`
- `docs/DECISIONS_PENDING.md`
- `docs/BLOCKERS.md`
- `docs/adr/`
- `docs/reports/ci/` and `docs/reports/platform/` for platform-level artifacts referenced by build/smoke/functional/release records
- `docs/reports/m0/evidence/` for redacted M0b evidence; private raw material stays in gitignored `docs/reports/m0/evidence-private/`
- Other `docs/reports/` subdirectories for milestone reports and artifacts that are not referenced as platform-level status evidence

Avoid duplicating the same information across many hand-maintained files. Generate support and benchmark summaries from machine-readable test/benchmark results when possible.

## 13. Human gates, scope cut line, and public-repository policy

### Gates
| Point | Type | Who decides |
|---|---|---|
| End of M0a | Hard stop | Human reads `M0A_SUMMARY.md` |
| Deferred M0b qualification | Required before initial production release | Human reviews the feature-complete browser, DEC-UA, sign-in gate and platform readiness |
| End of M1 | Soft checkpoint | Human reviews design tokens and the first screenshots; work may continue meanwhile |
| M2–M5 | Auto-advance on objective gates | Reviewer sign-off plus `verify` pass; handoff file at each boundary |
| End of M6 | Hard stop | Human visual and accessibility review on real artifacts |
| Any publish/release/signing | Hard stop | Human |

### Scope cut line
If M2–M5 run long, cut in this order and record each cut as an ADR, never by lowering quality gates: clear-on-exit; optional remote search suggestions; bookmark import/export; tab search; per-site zoom; new-tab shortcut customization. Core navigation, tabs, session restore, downloads, private windows, and the security requirements are never cut.

### Public repository
The repository is public. Use GitHub-hosted standard runners and keep CI matrices small; check current GitHub terms before relying on any allowance. The existing status-guard workflow pins `actions/checkout` to a verified immutable v7.0.1 commit SHA. CI must run the status-generator regression suite before checking generated status. Follow the "Public repository hygiene" rules in `AGENTS.md`. A LICENSE choice and name clearance are human decisions listed in `docs/DECISIONS_PENDING.md`.
