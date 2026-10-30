#!/usr/bin/env bash
# PDFium (Chromium's PDF engine, BSD-3) for the Brain's thread folders (`crates/echoctl/src/brain/files/pdf.rs`):
# PDF text layers, page objects and page images. bblanchon/pdfium-binaries' build pinned to the version
# pdfium-render expects (chromium/7881), checked by SHA-256, copied into <dest> with its license. Echo loads
# it at runtime; nothing links against it.
#
#   scripts/fetch-pdfium.sh <dest>          # this Mac (Apple Silicon)
#   ECHOCTL_PDFIUM_DIR=<dest> echoctl …      # a dev build finds it there
#   scripts/fetch-pdfium.sh ~/Library/Application\ Support/echo/pdfium   # or here
#
# package-macos.sh puts it in Echo.app/Contents/Frameworks/pdfium and signs it.
set -euo pipefail

BUILD=7881
ASSET=pdfium-mac-arm64
SHA256=52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40

dest=${1:?usage: fetch-pdfium.sh <dest>}
[[ "$(uname -m)" == arm64 ]] || { echo "the pinned build is for Apple Silicon" >&2; exit 1; }
cache=${TMPDIR:-/tmp}/echo-pdfium-$BUILD
mkdir -p "$cache"
tarball=$cache/$ASSET.tgz
if [[ ! -f "$tarball" ]] || [[ "$(shasum -a 256 "$tarball" | cut -d' ' -f1)" != "$SHA256" ]]; then
  curl -fsSL -o "$tarball.part" "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F$BUILD/$ASSET.tgz"
  mv "$tarball.part" "$tarball"
fi
got=$(shasum -a 256 "$tarball" | cut -d' ' -f1)
[[ "$got" == "$SHA256" ]] || { echo "checksum mismatch for $ASSET (got $got)" >&2; rm -f "$tarball"; exit 1; }
rm -rf "$cache/x" && mkdir -p "$cache/x" && tar xzf "$tarball" -C "$cache/x"
mkdir -p "$dest/licenses"
cp "$cache/x/lib/libpdfium.dylib" "$dest/libpdfium.dylib"
# PDFium's own license and those of what it bundles (FreeType, libjpeg-turbo, ICU, …).
cp "$cache/x/LICENSE" "$dest/licenses/LICENSE"
cp -R "$cache/x/licenses/." "$dest/licenses/"
echo "PDFium $BUILD → $dest"
