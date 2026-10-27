#!/usr/bin/env bash
# Build Echo.app for beta testers: release build → bundle (Info.plist, icon) → Developer ID
# signature with the hardened runtime → notarization → stapled .app inside a signed .dmg.
# The .dmg itself is not notarized: the stapled app inside is what Gatekeeper checks, and the
# container rides along.
#
#   scripts/package-macos.sh              # host architecture
#   UNIVERSAL=1 scripts/package-macos.sh  # arm64 + x86_64 (rustup target add x86_64-apple-darwin)
#   NOTARIZE=0 scripts/package-macos.sh   # signed but not notarized (local testing)
#   LOCAL=1 scripts/package-macos.sh      # no Apple developer account: dist/Echo.app for this Mac
#
# LOCAL=1: an ad-hoc signature, no notarization, no .dmg. An ad-hoc signature has no Team ID, so
# the hardened runtime's library validation would refuse the bundled NeMo-Speech libraries: the
# local build adds com.apple.security.cs.disable-library-validation. macOS keys its permissions
# (Microphone, System Audio Recording, Accessibility, Screen Recording) to the signature, which
# changes with every build, so it asks again after each one; a stale Accessibility or Screen
# Recording switch that stays on without working is cleared with
# `tccutil reset Accessibility ai.onetest.echo` (and `ScreenCapture`).
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
# -P: the physical path, as lsof reports a running Echo's executable (not a symlinked alias).
cd -P "$(dirname "$0")/.."

LOCAL=${LOCAL:-0}
APP=dist/Echo.app
# The bundle is rebuilt from scratch: never from under an Echo running from it. Asked of the
# kernel: lsof gives each running echoctl's executable file, whatever path started it (relative,
# through a symlink, another spelling of the same folder).
refuse_if_running() {
  local exe="$PWD/$APP/Contents/MacOS/echoctl" running
  [[ -e "$exe" ]] || return 0
  # All of lsof's answer first: a pipe into `grep -q` could cut it short, and pipefail would then
  # read the match as a failure.
  running=$(lsof -a -c echoctl -d txt -Fn 2>/dev/null) || true
  if grep -qxF "n$exe" <<<"$running"; then
    echo "Echo is running from $APP: quit it first ($APP/Contents/MacOS/echoctl app quit)" >&2
    exit 1
  fi
}
refuse_if_running

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
BUILD=$(git rev-list --count HEAD 2>/dev/null || echo 1)
PROFILE=${ECHOCTL_NOTARY_PROFILE:-echo-notary}
ENTITLEMENTS=packaging/macos/echo.entitlements
if [[ "$LOCAL" == 1 ]]; then
  SIGN_ID=- TIMESTAMP=--timestamp=none NOTARIZE=0
  ENTITLEMENTS_DIR=$(mktemp -d)
  trap 'rm -rf "$ENTITLEMENTS_DIR"' EXIT
  ENTITLEMENTS=$ENTITLEMENTS_DIR/echo.entitlements
  cp packaging/macos/echo.entitlements "$ENTITLEMENTS"
  /usr/libexec/PlistBuddy -c "Add :com.apple.security.cs.disable-library-validation bool true" "$ENTITLEMENTS"
else
  SIGN_ID=${ECHOCTL_SIGN_ID:-$(security find-identity -v -p codesigning | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)}
  TIMESTAMP=--timestamp
  [[ -n "$SIGN_ID" ]] || { echo "no Developer ID Application identity in the keychain (LOCAL=1 builds without one)" >&2; exit 1; }
fi

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

refuse_if_running # again: Echo may have started during the build
rm -rf "$APP" dist/echo.iconset
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/echoctl"
cp LICENSE.md "$APP/Contents/Resources/LICENSE.md"
# The plugin travels with the app: `echoctl notes` runs Claude Code with --plugin-dir on it,
# so the process-call skill always matches this CLI (no dev binary inside).
rsync -a --exclude bin --exclude node_modules --exclude '.claude-plugin/types' --exclude '*.test.ts' plugin/ "$APP/Contents/Resources/plugin/"
python3 scripts/third-party.py --texts "$APP/Contents/Resources/THIRD-PARTY-LICENSES.txt"
# NVIDIA's NeMo-Speech runtime for the Nemotron models (loaded at runtime, `nemo.rs`): the
# libraries in Frameworks/nemo-speech, signed below; their licenses in Resources. Apple Silicon
# only (the pinned build is arm64 + Metal): a universal build ships without it.
if [[ "$ARCH" == arm64 ]]; then
  rm -rf dist/nemo-speech
  scripts/fetch-nemo-speech.sh dist/nemo-speech
  mkdir -p "$APP/Contents/Frameworks/nemo-speech" "$APP/Contents/Resources/nemo-speech"
  mv dist/nemo-speech/licenses "$APP/Contents/Resources/nemo-speech/licenses"
  cp dist/nemo-speech/*.dylib "$APP/Contents/Frameworks/nemo-speech/"
  rm -rf dist/nemo-speech
else
  echo "warning: $ARCH build: no NeMo-Speech runtime, so no Nemotron models" >&2
fi
# CFBundleShortVersionString stays numeric (0.1.1); the pre-release tag lives in the DMG name.
sed -e "s/__VERSION__/${VERSION%%-*}/" -e "s/__BUILD__/$BUILD/" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
# App icon: an Icon Composer document compiled by actool (Xcode 26) into Assets.car — macOS
# 26 draws the shape, edge and shadow itself — plus echo.icns for older macOS. Without that
# actool, our own rounded-square .icns (macOS 26 then puts it on a tile).
rm -rf dist/echo.icon dist/icon-out dist/echo.iconset
"$BIN" dev-icon dist/echo.icon --from packaging/macos/onetest-logo-1024.png
mkdir -p dist/icon-out
if xcrun actool dist/echo.icon --compile dist/icon-out --platform macosx --target-device mac \
    --minimum-deployment-target 14.2 --app-icon echo --output-partial-info-plist dist/icon-out/partial.plist \
    --errors --output-format human-readable-text >/dev/null 2>&1 && [[ -f dist/icon-out/Assets.car ]]; then
  cp dist/icon-out/Assets.car dist/icon-out/echo.icns "$APP/Contents/Resources/"
else
  echo "warning: actool couldn't compile the icon (needs Xcode 26); using the plain .icns" >&2
  "$BIN" dev-icon dist/echo.iconset --from packaging/macos/onetest-logo-1024.png
  iconutil -c icns dist/echo.iconset -o "$APP/Contents/Resources/echo.icns"
fi
rm -rf dist/echo.icon dist/icon-out dist/echo.iconset

echo "signing with: $SIGN_ID"
# Nested code first (the hardened runtime loads only libraries signed by this team).
for lib in "$APP"/Contents/Frameworks/nemo-speech/*.dylib; do
  [[ -e "$lib" ]] && codesign --force "$TIMESTAMP" --options runtime --sign "$SIGN_ID" "$lib"
done
codesign --force "$TIMESTAMP" --options runtime \
  --entitlements "$ENTITLEMENTS" --sign "$SIGN_ID" "$APP"
codesign --verify --strict --verbose=2 "$APP"
if [[ "$LOCAL" == 1 ]]; then
  echo "$APP (ad-hoc signed for this Mac: open $APP)"
  exit 0
fi

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
