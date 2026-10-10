#!/usr/bin/env bash
# Build the release CLI and place it where the plugin's skills expect it.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p echoctl
exe=echoctl; [[ "${OS:-}" == "Windows_NT" ]] && exe=echoctl.exe
# Copy to a temp name and rename: overwriting a binary in place (especially while the watcher
# runs it) invalidates macOS's cached code signature and the kernel SIGKILLs it (exit 137).
mkdir -p plugin/bin
cp "target/release/$exe" "plugin/bin/.$exe.new"
mv -f "plugin/bin/.$exe.new" "plugin/bin/$exe"
echo "plugin/bin/$exe"
