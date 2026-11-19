---
spike: S7
platform: macos
substrate: wry-tao
budget_minutes: 60
started_at: 2026-11-19T17:22:30Z
deadline_at: 2026-11-19T18:22:30Z
stopped_at: 2026-11-19T17:32:21Z
stopped_reason: completed
outcome: pass
---

# S1 follow-up on macos

## Methods and evidence

Replayed the committed WRY everyday workflow on native macOS with a new loopback authority and scratch directory. The runner again returned two exact synthetic downloads, isolated storage and a successful denial. The S7 verifier checked actual native AppKit focus and keyboard evidence, popup opener behavior, both WRY boundary log pairs, cookie gating and download outputs, deliberate GTK renderer recovery, and three native overlay image geometries plus a historical negative. Six synthetic policy unit cases passed. Cargo's fixture test targets compiled and contain zero unit tests. [Verification transcript](../evidence/continuation-s7-checks.txt), [replay identity](../evidence/continuation-s7-replay-identity.txt), [replay results](../evidence/continuation-s7-replay-summary.json)

Both immutable action tags matched their official Git objects. The corrected Windows Server workflow passed floor/S1/S2a/S4 and the status guard passed at the published source. On the corrected Linux workflow both floor steps and S2a/S4 passed, and the independent S1 step kept the overall run red. Both published runner jobs uploaded artifacts even on a step failure. Windows now has separate runtime traces for the sample popup download and JS-close/retained-window gap. No headless and runner result was promoted to interactive acceptance. [Tag output](../evidence/continuation-s7-action-pins.txt), [CI step evidence](../evidence/continuation-s7-ci-corrected.json), [Linux fixture log](../evidence/continuation-s7-ci-linux-log.txt), [Windows popup trace](../evidence/continuation-s7-ci-windows-popup-native.txt)

## Recommendation and limits

Share a small deterministic fixture server, native launcher contract and evidence verifiers. Keep page, native and print/capture signals distinct. Scope controlled renderer disruption and shortcut automation to owned applications and retain manual gates for desktop, GPU, IME and real providers. The policy model tests expected view/frame/origin/epoch authorization. Binding those facts to native product dispatch remains unverified. No production harness or browser code was started. [Testing and CI proposal](../S7_TEST_AND_CI.md), [security assessment](../CONTINUATION_SECURITY.md)
