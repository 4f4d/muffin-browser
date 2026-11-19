#!/bin/sh
# Generate local synthetic playback stimuli for the human-operated matrix.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
out=$1
mkdir -p "$out"
ffmpeg -hide_banner -nostdin -loglevel error -n \
    -f lavfi -i 'testsrc2=size=320x180:rate=24:duration=2' \
    -f lavfi -i 'sine=frequency=440:duration=2' \
    -c:v libx264 -pix_fmt yuv420p -c:a aac -movflags +faststart -shortest "$out/h264-aac.mp4"
ffmpeg -hide_banner -nostdin -loglevel error -n \
    -f lavfi -i 'testsrc2=size=320x180:rate=24:duration=2' \
    -c:v libvpx-vp9 -pix_fmt yuv420p "$out/vp9.webm"
ffmpeg -hide_banner -nostdin -loglevel error -n \
    -f lavfi -i 'testsrc2=size=320x180:rate=24:duration=2' \
    -c:v libsvtav1 -preset 12 -svtav1-params lp=2 -pix_fmt yuv420p "$out/av1.webm"
ffmpeg -hide_banner -nostdin -loglevel error -n \
    -f lavfi -i 'sine=frequency=440:duration=2' -c:a libopus "$out/opus.ogg"
python3 - "$repo" "$out" <<'PY'
import runpy
import sys
from pathlib import Path

s4 = runpy.run_path(str(Path(sys.argv[1]) / "spikes/s1/followup/s4_fixture.py"))
target = Path(sys.argv[2])
with (target / "sample.pdf").open("xb") as file:
    file.write(s4["PDF"])
with (target / "upload-sample.txt").open("x", encoding="utf-8") as file:
    file.write("Muffin synthetic manual upload sample\n")
PY
cat > "$out/index.html" <<'HTML'
<!doctype html>
<meta charset="utf-8">
<title>Muffin manual fixture assets</title>
<h1>Muffin manual fixture assets</h1>
<h2>H.264 and AAC</h2>
<video src="h264-aac.mp4" controls></video>
<h2>VP9</h2>
<video src="vp9.webm" controls></video>
<h2>AV1</h2>
<video src="av1.webm" controls></video>
<h2>Opus</h2>
<audio src="opus.ogg" controls></audio>
<p><a href="sample.pdf">Open the sample PDF</a></p>
<p><a href="upload-sample.txt" download>Download the synthetic upload sample</a></p>
HTML
