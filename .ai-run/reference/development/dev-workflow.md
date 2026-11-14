# Dev Workflow — reference

How to build, run, debug and package a dev Echo. It changes with the code and the scripts, in the same PR. The rules
are in `.ai-run/guides/development/dev-workflow.md`.

## Features

- Default features: `transcribe`, `tray`, `brain`, `js` (`crates/echoctl/Cargo.toml::default`).
- Without the C-heavy parts: `cargo build -p echoctl --no-default-features --features tray` keeps the app and window.
- `vulkan` adds whisper.cpp on the GPU (Windows/Linux, needs `VULKAN_SDK`).

## Prerequisites

- `scripts/check-mod.sh` needs `claude`, `node` and `npx`.
- The Windows cross-check needs `rustup target add x86_64-pc-windows-msvc`.

## Windows toolchain

Build with the MSVC toolchain and:

- Visual Studio Build Tools, their CMake on `PATH` (in Git Bash add
  `/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin`);
- LLVM, with `LIBCLANG_PATH` at its `bin`;
- for a release (`--features vulkan`), the Vulkan SDK in `VULKAN_SDK` (`scripts/package-windows.ps1::VULKAN_SDK`).

## Packaging

- Windows: `scripts/package-windows.ps1 [-Phase all | stage | package]` → MSI and zip in `dist`. It builds in
  `ECHO_BUILD_DIR` (default `<system drive>\echo-build`) and stages in `ECHO_STAGE_DIR`; sign between `-Phase stage` and
  `-Phase package`.
- macOS: `scripts/package-macos.sh` makes the signed, notarized app and dmg; `LOCAL=1` builds an ad-hoc signed
  `dist/Echo.app` for permission work on this Mac. After it, macOS asks for every permission again; clear a stale one with
  `tccutil reset <service> ai.onetest.echo`.

## Runtime libraries

A dev build gets the libraries it loads at run time from `scripts/fetch-pdfium.sh <dest>` and
`scripts/fetch-nemo-speech.sh <dest>` (Apple Silicon), pointed at by `ECHOCTL_PDFIUM_DIR` / `ECHOCTL_NEMO_DIR`
(`nemo::candidates`).

## Worktrees

An agent's worktree builds with `CARGO_TARGET_DIR=<worktree>/target`; `serve.sh` and the case steps run that worktree's
`target/debug/echoctl` (`make_data.py::Data`).

## A dev app beside Echo

- Its own folders: `ECHOCTL_CONFIG_DIR=<dir>` and `ECHOCTL_HOME=<dir>` (`app::record_app_path`, `paths::config_dir`).
- Run it in the foreground with `target/debug/echoctl app run` (`--no-tray` without the icon), or in the background with
  `target/debug/echoctl app start`: `app::spawn_app` opens the installed bundle only when the running binary is inside
  one, so a dev build starts itself, with the folders of its environment. Stop it with `echoctl app quit`.

## Debugging

- Permissions: `echoctl doctor` (`--probe`, `--request`, `--open <pane>`, `--json`) while the app runs
  (`permissions::doctor`).
- `ECHOCTL_DEBUG=1` prints Echo's diagnostics on stderr (`ai::terminal_command`).
- Logs: `<home>/.echo/app.log` (the app), `<call>/claude.log` and `<call>/notes-run.json` (a minutes run),
  `<call>/transcribe.error` (transcription) (`notes::LOG`).

## Reviewing the window

- `echoctl window --serve <port>` on a dev data folder serves the window to a browser; `ECHOCTL_SERVE_IPC=1` lets clicks
  reach the app (`window::serve`).
- `--export <dir>` writes it as static files; `--selftest` checks the real webview.

## Trying the Claude Code plugin

`scripts/build-plugin.sh`, then `claude --plugin-dir plugin`.
