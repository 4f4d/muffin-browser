# ADR 0002 — macOS-first implementation and deferred qualification

Status: Accepted by the project owner for implementation  
Date: 2026-11-20

## Decision

Proceed through the planned engineering milestones in dependency order. Build the production browser for macOS Apple Silicon first, using WRY + TAO as the provisional substrate. Preserve the existing Windows, Linux X11 and Linux Wayland implementations under `spikes/` and keep them on the roadmap. Platform deferral applies to the first release.

Defer the 15 regular macOS M0b cases and conditional SI-05 diagnostic to qualification of the feature-complete browser. Preserve the runbook, result records, binaries, manifests and historical evidence. The owner retains M0b acceptance, the sign-in gate, DEC-UA, final platform eligibility and release authorization.

## Context and trade-offs

The temporary validation build supports synthetic engineering checks. It remains available as a reference for native behavior and evidence. Production development will use distinct application code and independent automated reviews. Deferring real account, hardware and GPU checks allows development to proceed with known qualification risks. A failed qualification case may still change initial support or the scope of a release. [Validation review](../reports/m0/M0B_VALIDATION_REVIEW.md), [manual matrix](../M0B_MANUAL_TESTS.md)

S1 remains open for unexpected WKWebView WebContent termination and recovery. Windows 11 and interactive GNOME X11 and Wayland remain unverified. The Linux download destination failure remains open. The native security boundary requires production implementation and tests. S5 found substantial multitab engine memory overhead. Measure complete process trees and use conservative view retention. [S1 handoff](../reports/m0/M0A_S1_FOLLOW_UP.md), [Linux blocker](../BLOCKERS.md), [S5 comparative baselines](../reports/m0/S5_COMPARATIVE_BASELINES.md), [provisional substrate evidence](0001-m0a-substrate-evidence.md)

## Gates

- Engineering milestones require their executable checks, evidence and independent review.
- The M6 human visual and accessibility checkpoint and publication, signing and release approvals remain explicit gates.
- At feature completion, assemble a consolidated macOS qualification plan and resume the deferred M0b matrix on the production browser.
- Preserve the unresolved feasibility and security requirements until their evidence exists. No M0 or M0b acceptance is implied by opening M1.
