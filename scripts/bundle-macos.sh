#!/bin/sh
# Wraps the release build as target/Voyager.app, so Finder, Launchpad and the Dock show its icon.
set -eu
cd "$(dirname "$0")/.."
cargo build --release
app=target/Voyager.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/voyager "$app/Contents/MacOS/voyager"
cp assets/Voyager.icns "$app/Contents/Resources/Voyager.icns"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Voyager</string>
    <key>CFBundleDisplayName</key><string>Voyager</string>
    <key>CFBundleIdentifier</key><string>io.github.trondegil.voyager</string>
    <key>CFBundleExecutable</key><string>voyager</string>
    <key>CFBundleIconFile</key><string>Voyager</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app" >/dev/null 2>&1 || true
echo "built $app"
