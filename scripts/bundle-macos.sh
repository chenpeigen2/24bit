#!/usr/bin/env bash
# Build a double-clickable 24bit.app from the release binary.
#
# Deliberately dependency-free (no cargo-bundle / cargo-packager) so it works on
# a machine with only the Command Line Tools installed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/dist/24bit.app"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')"

echo "==> building release binary"
cargo build --release -p twentyfourbit-gui --manifest-path "$ROOT/Cargo.toml"

echo "==> assembling $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$ROOT/target/release/24bit-gui" "$APP/Contents/MacOS/24bit"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>24bit</string>
    <key>CFBundleDisplayName</key>       <string>24bit</string>
    <key>CFBundleIdentifier</key>        <string>net.twentyfourbit.client</string>
    <key>CFBundleVersion</key>           <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleExecutable</key>        <string>24bit</string>
    <key>LSMinimumSystemVersion</key>    <string>11.0</string>
    <key>NSHighResolutionCapable</key>   <true/>
    <key>NSHumanReadableCopyright</key>  <string>Unofficial client. MIT OR Apache-2.0.</string>
</dict>
</plist>
PLIST

if [ -f "$ROOT/assets/icon.icns" ]; then
    cp "$ROOT/assets/icon.icns" "$APP/Contents/Resources/icon.icns"
    /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string icon.icns" \
        "$APP/Contents/Info.plist" >/dev/null
fi

# Ad-hoc signature keeps Gatekeeper quiet for a locally built app.
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || \
    echo "    (codesign unavailable; the app still runs locally)"

echo "==> done: $APP"
