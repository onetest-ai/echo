#!/usr/bin/env bash
# Build TestCall.app (bundle id ai.onetest.echo.testcall) into tests/tools/testcall/build/: `tests/tools/testcall/build.sh`.
# Run it as tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall … (see main.swift for the commands).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
app="$here/build/TestCall.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>ai.onetest.echo.testcall</string>
  <key>CFBundleName</key><string>TestCall</string>
  <key>CFBundleExecutable</key><string>testcall</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>LSUIElement</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>TestCall uses a (virtual) microphone to take part in Echo's test calls.</string>
</dict></plist>
PLIST
swiftc -O -swift-version 5 "$here/main.swift" -o "$app/Contents/MacOS/testcall"
codesign --force --sign - "$app" > /dev/null
echo "$app"
