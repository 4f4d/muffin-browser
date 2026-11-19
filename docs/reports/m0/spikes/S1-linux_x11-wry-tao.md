---
spike: S1
platform: linux_x11
substrate: wry-tao
budget_minutes: 90
started_at: 2026-11-19T14:08:06Z
deadline_at: 2026-11-19T15:38:06Z
stopped_at: 2026-11-19T14:58:52Z
stopped_reason: blocked
outcome: inconclusive
---

# Spike report: S1 on linux_x11 (wry-tao)

## Method

Ran native WRY 0.57.2 + TAO 0.37.2 with `gtk::Fixed` on an Ubuntu 24.04.5 x64 runner under Xvfb and in an isolated Ubuntu container on a physical x86_64 server. Measured DOM viewports, local navigation, overlay and group lifecycle. The follow-up enabled WebKitGTK's explicit process termination API for the content view, waited for the termination callback, reloaded the loopback page and deferred focus reads by a native event-loop tick.

## Results

`docs/reports/m0/evidence/s1-x11-wry-tao.txt` and `s1-wry-linux-server-x11.txt` show content/chrome layout and resize, loopback navigation, all four create/load/drop groups and neither inspected bridge global. Both X11 window captures contain the 270×144 red overlay spanning the two regions at the requested coordinates. See `docs/reports/m0/evidence/s1-image-check.txt` and `s1-wry-linux-server-x11.png`.

The two pilot runs reported content focus true and chrome false. The server follow-up at source 59faf2f reported `TerminatedByApi`, loaded /next?recovery=1, and had true focus readings for both targeted views. Recovered content reported a 900×548 viewport following the earlier 1000×628 resized reading. Geometry preservation through recovery remains unverified. See `docs/reports/m0/evidence/s1-wry-server-x11-recovery.txt`.

## Findings and implications

Xvfb provides functional native composition evidence. The explicit GTK termination and reload path recovered controlled content. Desktop hardware rendering, real input, keyboard accelerators and unplanned renderer crash recovery remain open. The server package setup was isolated in Docker.
