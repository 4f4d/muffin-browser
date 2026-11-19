#!/usr/bin/env bash
set -euo pipefail
binary=$1
title=$2
log=$3
png=$4
python3 spikes/s1/run_fixture.py "$binary" --hold 12 > "$log" 2>&1 &
runner=$!
for _ in {1..700}; do
  if grep -q '^screenshot_ready=true$' "$log"; then
    id=$(xwininfo -root -tree | awk -v title="$title" 'index($0, "\"" title "\"") { print $1; exit }')
    if [[ -n "$id" ]]; then
      import -silent -window "$id" "$png" || printf 'capture_failed=true\n' >&2
    else
      printf 'capture_failed=window_not_found\n' >&2
    fi
    break
  fi
  kill -0 "$runner" 2>/dev/null || break
  sleep 0.25
done
wait "$runner"
