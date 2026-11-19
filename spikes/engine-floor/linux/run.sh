#!/usr/bin/env bash
set -euo pipefail
backend=$1
shift
case "$backend" in
  x11) exec xvfb-run -a -s '-screen 0 1280x900x24' env GDK_BACKEND=x11 "$@" ;;
  wayland)
    runtime=$(mktemp -d)
    chmod 700 "$runtime"
    export XDG_RUNTIME_DIR="$runtime" WAYLAND_DISPLAY=muffin-wayland GDK_BACKEND=wayland
    unset DISPLAY
    weston --backend=headless-backend.so --use-pixman --socket="$WAYLAND_DISPLAY" --idle-time=0 --log="$runtime/weston.log" &
    compositor=$!
    cleanup() {
      kill "$compositor" 2>/dev/null || true
      wait "$compositor" 2>/dev/null || true
    }
    trap cleanup EXIT
    for _ in {1..50}; do
      [[ -S "$runtime/$WAYLAND_DISPLAY" ]] && break
      if ! kill -0 "$compositor" 2>/dev/null; then
        tail -15 "$runtime/weston.log" || true
        exit 1
      fi
      sleep 0.2
    done
    [[ -S "$runtime/$WAYLAND_DISPLAY" ]]
    "$@"
    ;;
  *) echo "Use x11 or wayland" >&2; exit 2 ;;
esac
