---
spike: S1-follow-up
platform: linux_wayland
substrate: wry-tao
budget_minutes: 30
started_at: 2026-11-19T15:27:33Z
deadline_at: 2026-11-19T15:57:33Z
stopped_at: 2026-11-19T15:35:57Z
stopped_reason: blocked
outcome: blocked
---

# S1 follow-up on linux_wayland

## Attempt and result

Probed locally available guests and the authorized Ubuntu server. The server offered KVM and QEMU but exposed only a tty session and no identified GNOME Wayland desktop. A bounded range request for the official Ubuntu Desktop live image confirmed a rate incompatible with the time available on that route. No interactive session was reached. [Guest inventory and bounded media probe](../evidence/s1-followup-guest-inventory.txt)

## Remaining evidence

The earlier headless Wayland run demonstrated child creation, navigation and controlled reload, and left startup viewport geometry unresolved. Interactive GNOME Wayland geometry, resize, z-order, native focus, input, shortcuts, IME, scaling, GPU composition and unexpected renderer recovery remain untested. A VM can answer functional questions. Wayland rendering and GPU evidence require a suitable bare-metal desktop. [Original Wayland report](S1-linux_wayland-wry-tao.md)
