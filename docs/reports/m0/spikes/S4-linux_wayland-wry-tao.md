---
spike: S4
platform: linux_wayland
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T16:48:50Z
deadline_at: 2026-11-19T18:18:50Z
stopped_at: 2026-11-19T17:07:04Z
stopped_reason: completed
outcome: inconclusive
---

# S1 follow-up on linux_wayland

## Method and observed evidence

Ran committed WRY + TAO in a cached Ubuntu x86_64 container with WebKitGTK 2.52.6 and headless Weston on the authorized host. Two sample downloads matched all 786,432 bytes and carried the XDG origin attribute. The denied attachment was refused. The private view could neither read the normal cookie or localStorage nor access the cookie-gated attachment. WRY finished navigation to the PDF response. The native GTK control showed inline PDF text and eliminated one selected image request while retaining the control. [Source, build, runtime and results](../evidence/continuation-s4-linux-wayland.txt), [native control](../evidence/continuation-s4-engine-wayland.txt), [PDF snapshot](../evidence/continuation-s4-pdf-wayland.png)

## Open criteria

Desktop parity, printing and local file handling, range of PDF documents, native transfer progress, wider network filtering and private teardown/cache/crash behavior remain open. XDG metadata is an origin marker. The fixture did not test OS quarantine enforcement. [Feasibility matrix](../S4_EVERYDAY_MATRIX.md)
