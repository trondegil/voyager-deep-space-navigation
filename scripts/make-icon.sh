#!/bin/sh
# Makes the app icon. The app draws it (its `icon` scene is Saturn alone), then ffmpeg cuts it to
# the macOS tile: an 824 px superellipse on a 1024 px canvas, with a light on its upper rim and a
# shadow under it. Needs ffmpeg, and sips and iconutil (macOS) for the .icns.
# Rebuild the app afterwards: assets/icon.png is compiled into it.
set -eu
cd "$(dirname "$0")/.."
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --release --quiet
target/release/voyager --scene icon --snap "$work/raw.png" --at 2 --size 1024x1024 --scale 1 >/dev/null 2>&1

# In each channel: 0 = distance inside the tile's edge in px, 1 = the tile's coverage, 2 = the rim
# light, 3 and 4 = the same edge 14 px lower and the shadow it casts, 5 = tile over shadow.
shape='st(0,(1-pow(abs(X-512)/412,5)-pow(abs(Y-512)/412,5))*82.4);st(1,clip(ld(0)+0.5,0,1));st(2,clip(1-ld(0)/3,0,1)*(0.25+0.75*clip(0.5-(Y-512)/824,0,1))*70);st(3,(1-pow(abs(X-512)/412,5)-pow(abs(Y-526)/412,5))*82.4);st(4,0.42*pow(clip(ld(3)/44+0.6,0,1),2));st(5,ld(1)+(1-ld(1))*ld(4))'
paint() { printf "%s;if(gt(ld(5),0.001),(%s(X,Y)+ld(2))*ld(1)/ld(5),0)" "$shape" "$1"; }
ffmpeg -y -loglevel error -i "$work/raw.png" -frames:v 1 -update 1 -vf "scale=824:824:flags=lanczos,format=rgba,pad=1024:1024:100:100:color=black@0,geq=r='$(paint r)':g='$(paint g)':b='$(paint b)':a='$shape;255*ld(5)'" "$work/icon-1024.png"

mkdir -p assets
ffmpeg -y -loglevel error -i "$work/icon-1024.png" -frames:v 1 -update 1 -vf scale=512:512:flags=lanczos assets/icon.png

set="$work/Voyager.iconset"
mkdir "$set"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$work/icon-1024.png" --out "$set/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "$work/icon-1024.png" --out "$set/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set" -o assets/Voyager.icns
ls -l assets/icon.png assets/Voyager.icns
