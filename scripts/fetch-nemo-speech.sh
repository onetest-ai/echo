#!/usr/bin/env bash
# NVIDIA's NeMo-Speech.cpp runtime (Nemotron models, `crates/echoctl/src/nemo.rs`): download a
# pinned release, check its SHA-256, and copy the ASR libraries into <dest> (and their licenses
# into <dest>/licenses). Echo loads them at runtime; nothing links against them.
#
#   scripts/fetch-nemo-speech.sh <dest>     # this Mac (Apple Silicon, Metal)
#   ECHOCTL_NEMO_DIR=<dest> echoctl …        # a dev build finds it there
#   scripts/fetch-nemo-speech.sh ~/Library/Application\ Support/echo/nemo-speech   # or here
#
# package-macos.sh puts it in Echo.app/Contents/Frameworks/nemo-speech and signs it; the Windows
# build (package-windows.ps1) fetches its own (the CPU build) the same way.
set -euo pipefail

VERSION=0.2.0
ASSET=nemo-speech-$VERSION-macos-aarch64-metal
SHA256=5cb02ba7c04f0b5585ce5cde9c830be5f0b7dfb4c83083c500109c381b4f2da9
# The ASR C library and what it loads (install names: @rpath/<name>, resolved next to it).
LIBS=(libnemo_speech_asr_c.1.dylib libnemo_speech_asr.dylib libnemo_speech_text_normalization.dylib
      libggml.0.dylib libggml-base.0.dylib libggml-cpu.0.dylib libggml-metal.0.dylib libggml-blas.0.dylib)

dest=${1:?usage: fetch-nemo-speech.sh <dest>}
[[ "$(uname -m)" == arm64 ]] || { echo "the pinned runtime is for Apple Silicon" >&2; exit 1; }
cache=${TMPDIR:-/tmp}/echo-nemo-speech-$VERSION
mkdir -p "$cache"
tarball=$cache/$ASSET.tar.gz
if [[ ! -f "$tarball" ]] || [[ "$(shasum -a 256 "$tarball" | cut -d' ' -f1)" != "$SHA256" ]]; then
  curl -fsSL -o "$tarball.part" "https://github.com/NVIDIA/NeMo-Speech.cpp/releases/download/v$VERSION/$ASSET.tar.gz"
  mv "$tarball.part" "$tarball"
fi
got=$(shasum -a 256 "$tarball" | cut -d' ' -f1)
[[ "$got" == "$SHA256" ]] || { echo "checksum mismatch for $ASSET (got $got)" >&2; rm -f "$tarball"; exit 1; }
rm -rf "$cache/x" && mkdir -p "$cache/x" && tar xzf "$tarball" -C "$cache/x"
src=$cache/x/$ASSET
mkdir -p "$dest/licenses"
for l in "${LIBS[@]}"; do
  cp -L "$src/lib/$l" "$dest/$l" # the release's names are symlinks to versioned files
done
cp -R "$src/share/licenses/nemo-speech/." "$dest/licenses/"
echo "NeMo-Speech $VERSION → $dest"
