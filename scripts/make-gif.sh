#!/bin/sh
# Records scripts/tour.json headless and cuts the first jump into docs/voyager.gif. Needs ffmpeg.
set -eu
cd "$(dirname "$0")/.."
cargo build --release --quiet
mkdir -p out docs
target/release/voyager --script scripts/tour.json --video out/voyager.mp4 --size 1200x750 --scale 1 --fps 60 --pointer --keys bottom
# One palette for the whole clip, 160 colours, ordered dither: about 10 MB for 9 seconds.
ffmpeg -y -loglevel error -ss 1.6 -t 9.3 -i out/voyager.mp4 -vf "fps=12,scale=640:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=160:stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" docs/voyager.gif
ls -l docs/voyager.gif
