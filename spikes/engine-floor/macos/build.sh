#!/bin/sh
set -eu
src=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
out=$1
mkdir -p "$out/MuffinFloor.app/Contents/MacOS"
cp "$src/Info.plist" "$out/MuffinFloor.app/Contents/Info.plist"
swiftc -O -framework AppKit -framework WebKit "$src/main.swift" -o "$out/MuffinFloor.app/Contents/MacOS/muffin-floor"
(cd "$out" && zip -q -r MuffinFloor.zip MuffinFloor.app)
