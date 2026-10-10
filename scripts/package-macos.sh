#!/usr/bin/env bash
# Build Echo.app for beta testers: release build → bundle (Info.plist, icon) → Developer ID
# signature with the hardened runtime → notarization → stapled .app inside a signed .dmg.
# The .dmg itself is not notarized: the stapled app inside is what Gatekeeper checks, and the
# container rides along.
#
#   scripts/package-macos.sh              # host architecture
#   UNIVERSAL=1 scripts/package-macos.sh  # arm64 + x86_64 (rustup target add x86_64-apple-darwin)
#   NOTARIZE=0 scripts/package-macos.sh   # signed but not notarized (local testing)
#
# Notarization credentials, either:
#   App Store Connect API key (preferred):
#     APPLE_API_KEY=<path to AuthKey_XXXX.p8>  APPLE_API_KEY_ID=<key id>  APPLE_API_ISSUER=<issuer uuid>
#     (load the issuer from its file without echoing it; never print it or the .p8)
#   or a keychain profile: xcrun notarytool store-credentials echo-notary …
#     (ECHOCTL_NOTARY_PROFILE, default echo-notary)
#
# Env: ECHOCTL_SIGN_ID (default: the first "Developer ID Application" identity).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
BUILD=$(git rev-list --count HEAD 2>/dev/null || echo 1)
SIGN_ID=${ECHOCTL_SIGN_ID:-$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)}
PROFILE=${ECHOCTL_NOTARY_PROFILE:-echo-notary}
[[ -n "$SIGN_ID" ]] || { echo "no Developer ID Application identity in the keychain" >&2; exit 1; }

if [[ "${UNIVERSAL:-0}" == 1 ]]; then
  cargo build --release -p echoctl --target aarch64-apple-darwin
  cargo build --release -p echoctl --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create -output target/universal/echoctl \
    target/aarch64-apple-darwin/release/echoctl target/x86_64-apple-darwin/release/echoctl
  BIN=target/universal/echoctl; ARCH=universal
else
  cargo build --release -p echoctl
  BIN=target/release/echoctl; ARCH=$(uname -m)
fi

APP=dist/Echo.app
rm -rf "$APP" dist/echo.iconset
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/echoctl"
cp LICENSE.md "$APP/Contents/Resources/LICENSE.md"
# The plugin travels with the app: `echoctl notes` runs Claude Code with --plugin-dir on it,
# so the process-call skill always matches this CLI (no dev binary inside).
rsync -a --exclude bin --exclude node_modules --exclude '.claude-plugin/types' --exclude '*.test.ts' plugin/ "$APP/Contents/Resources/plugin/"
python3 scripts/third-party.py --texts "$APP/Contents/Resources/THIRD-PARTY-LICENSES.txt"
# CFBundleShortVersionString stays numeric (0.1.1); the pre-release tag lives in the DMG name.
sed -e "s/__VERSION__/${VERSION%%-*}/" -e "s/__BUILD__/$BUILD/" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
"$BIN" dev-icon dist/echo.iconset --from packaging/macos/onetest-logo-1024.png
iconutil -c icns dist/echo.iconset -o "$APP/Contents/Resources/echo.icns"
rm -rf dist/echo.iconset

echo "signing with: $SIGN_ID"
codesign --force --timestamp --options runtime \
  --entitlements packaging/macos/echo.entitlements --sign "$SIGN_ID" "$APP"
codesign --verify --strict --verbose=2 "$APP"

notarize() { # file
  if [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_KEY_ID:-}" && -n "${APPLE_API_ISSUER:-}" ]]; then
    xcrun notarytool submit "$1" --key "$APPLE_API_KEY" --key-id "$APPLE_API_KEY_ID" \
      --issuer "$APPLE_API_ISSUER" --wait
  else
    xcrun notarytool submit "$1" --keychain-profile "$PROFILE" --wait
  fi
}

if [[ "${NOTARIZE:-1}" == 1 ]]; then
  ditto -c -k --keepParent "$APP" dist/Echo-notarize.zip
  notarize dist/Echo-notarize.zip
  rm dist/Echo-notarize.zip
  xcrun stapler staple "$APP"
fi

# Drag-to-Applications disk image.
DMG="dist/Echo-$VERSION-macos-$ARCH.dmg"
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "Echo $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGE"
codesign --force --timestamp --sign "$SIGN_ID" "$DMG"
[[ "${NOTARIZE:-1}" == 1 ]] && { spctl -a -vv -t exec "$APP" || true; }
echo "$DMG"
