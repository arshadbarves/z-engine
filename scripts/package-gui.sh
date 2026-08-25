#!/usr/bin/env bash
# Package the desktop app into a macOS .app bundle (ad-hoc signed).
# Prereqs: release build of harness-gui + ui/dist present.
set -euo pipefail
cd "$(dirname "$0")/.."

BIN="target/release/harness-gui"
[ -x "$BIN" ] || { echo "missing $BIN — run: cargo build --release --workspace"; exit 1; }
[ -d "crates/harness-gui/ui/dist" ] || { echo "missing ui/dist — run: (cd crates/harness-gui/ui && npm run build)"; exit 1; }

APP="target/release/bundle/macos/Harness.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/harness-gui"
cp crates/harness-gui/src-tauri/icons/icon.png "$APP/Contents/Resources/icon.png"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>              <string>Harness</string>
    <key>CFBundleDisplayName</key>       <string>Harness</string>
    <key>CFBundleIdentifier</key>        <string>dev.harness.gui</string>
    <key>CFBundleExecutable</key>        <string>harness-gui</string>
    <key>CFBundleIconFile</key>          <string>icon</string>
    <key>CFBundlePackageType</key>       <string>APPL</string>
    <key>CFBundleShortVersionString</key><string>1.1.0</string>
    <key>CFBundleVersion</key>           <string>1</string>
    <key>LSMinimumSystemVersion</key>    <string>10.15</string>
    <key>NSHighResolutionCapable</key>   <true/>
    <key>NSAppTransportSecurity</key>
    <dict>
        <key>NSAllowsArbitraryLoads</key><true/>
    </dict>
</dict>
</plist>
PLIST

codesign --force --deep -s - "$APP" 2>/dev/null || true
echo "packaged: $APP"
