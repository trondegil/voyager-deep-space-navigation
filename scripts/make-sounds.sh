#!/bin/sh
# Synthesizes the app's sounds (examples/make_sounds.rs) and packs the two long clips as AAC.
# Needs ffmpeg. Rebuild the app afterwards: the clips are compiled into it.
set -eu
cd "$(dirname "$0")/.."
cargo run --release --quiet --example make_sounds -- assets/sounds
for clip in trip ambient; do
    # 256k: at lower rates the encoder overshoots the booms past full scale.
    ffmpeg -y -loglevel error -i "assets/sounds/$clip.wav" -c:a aac -b:a 256k "assets/sounds/$clip.m4a"
    rm "assets/sounds/$clip.wav"
done
ls -l assets/sounds
