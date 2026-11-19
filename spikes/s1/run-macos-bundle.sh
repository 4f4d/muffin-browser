#!/usr/bin/env bash
set -euo pipefail
binary=$1
helper=$2
out=$3
bundle="$out/MuffinS1Wry.app"
mkdir -p "$bundle/Contents/MacOS"
cp spikes/s1/wry-tao/Info.plist "$bundle/Contents/Info.plist"
cp "$binary" "$bundle/Contents/MacOS/muffin-s1-wry"
{
  printf 'source_commit='
  git rev-parse HEAD
  printf 'binary_sha256='
  shasum -a 256 "$binary" | cut -d ' ' -f 1
  sw_vers
  uname -m
  rustc -Vv
} > "$out/host.txt"
log="$out/spike.txt"
: > "$log"
python3 spikes/s1/run_fixture.py "$bundle" --hold 20 --bundle-log "$log" > "$out/fixture.txt" 2>&1 &
worker=$!
for _ in {1..650}; do
  if rg -qx 'screenshot_ready=true' "$log"; then
    pid=$(sed -n 's/^native_app_pid=//p' "$log" | head -1)
    number=$(sed -n 's/^native_window_number=//p' "$log" | head -1)
    if [[ "$number" =~ ^[1-9][0-9]*$ ]]; then
      /usr/sbin/screencapture -x -o -l "$number" "$out/spike.png" || printf 'capture_failed=true\n' >&2
    else
      "$helper" capture "$pid" "$out/spike.png" || printf 'capture_failed=true\n' >&2
    fi
    break
  fi
  kill -0 "$worker" 2>/dev/null || break
  sleep .25
done
wait "$worker"
