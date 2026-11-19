#!/usr/bin/env bash
set -euo pipefail
binary=$1
helper=$2
log=$3
png=$4
{
  printf 'source_commit='
  git rev-parse HEAD
  printf 'binary_sha256='
  shasum -a 256 "$binary" | cut -d ' ' -f 1
  sw_vers
  uname -m
  rustc -Vv
  python3 spikes/s1/run_fixture.py "$binary" --mac-helper "$helper" --hold 15
} > "$log" 2>&1 &
worker=$!
for _ in {1..650}; do
  if rg -qx 'screenshot_ready=true' "$log"; then
    pid=$(sed -n 's/^fixture_app_pid=//p' "$log" | head -1)
    "$helper" capture "$pid" "$png" || printf 'capture_failed=true\n' >&2
    break
  fi
  kill -0 "$worker" 2>/dev/null || break
  sleep .25
done
wait "$worker"
