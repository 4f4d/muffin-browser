# Muffin Browser

Muffin is a native desktop browser in development. The initial implementation targets Apple Silicon macOS and uses the system WebKit engine through WRY and TAO. Windows and Linux feasibility builds remain under `spikes/`.

## Run the development app

Install Rust 1.95.0 with `rustfmt` and `clippy`, Node.js 24, and Python 3.11 or later. On Apple Silicon macOS, run these commands from the repository root.

```sh
npm --prefix ui ci
npm --prefix ui run build
cargo run --locked -p muffin
```

The chrome bundle is compiled into the native executable. It uses local assets and has no runtime Node.js dependency. Open `ui/dist/gallery/gallery.html` after the bundle build to inspect chrome components independently of the native window.

## Verify

Install `just` and `cargo-audit` through the development package manager. Run the verification command from an active macOS desktop session.

```sh
just verify
```

The verifier runs the status guard, fixture tests, frontend checks, dependency audits, Rust checks, debug and release builds, and native loopback smoke. It writes development receipts beneath `target/dev-verification/`. The loopback fixture and debug test controls use a disposable session. The release build rejects test control.

The [project plan](docs/PLAN.md) tracks implementation and qualification. [Generated status](docs/STATUS.json) tracks platform evidence and the [manual runbook](docs/M0B_MANUAL_TESTS.md) remains the source for deferred owner-operated cases.
