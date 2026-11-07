#!/usr/bin/env bash
# Build the macOS bundle from source, in the only order that works:
#
#   1. vite build      -> ui/dist
#   2. cargo build     -> embeds ui/dist into the binary (tauri-build)
#   3. copy + sign     -> dist/24bit.app
#
# Doing these out of order silently ships a stale UI inside a fresh binary,
# which is exactly the kind of bug that makes "I rebuilt it" a lie.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')"
APP="$ROOT/dist/24bit.app"

echo "==> frontend"
(cd "$ROOT/twentyfourbit-tauri/ui" && npm run build --silent)

echo "==> rust"
cargo build --release --manifest-path "$ROOT/Cargo.toml" -p twentyfourbit-tauri

echo "==> bundle"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/target/release/twentyfourbit-tauri" "$APP/Contents/MacOS/24bit"
cp "$ROOT/twentyfourbit-tauri/icons/icon.icns" "$APP/Contents/Resources/icon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>24bit</string>
  <key>CFBundleDisplayName</key><string>24bit</string>
  <key>CFBundleIdentifier</key><string>net.twentyfourbit.client</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>24bit</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Ad-hoc signature: without one Gatekeeper refuses the bundle outright.
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true

echo "==> $APP"
du -sh "$APP" | cut -f1
