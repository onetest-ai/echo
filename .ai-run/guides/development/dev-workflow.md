# Dev Workflow

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: build, run and debug a dev Echo; packaging

## Build and package

### Rules
- R1. Build without the C-heavy parts when you don't need them: default features are `transcribe`, `tray`, `brain`, `js`; `cargo build -p echoctl --no-default-features --features tray` keeps the app and window; `vulkan` adds whisper.cpp on the GPU (Windows/Linux, needs `VULKAN_SDK`). Evidence: `crates/echoctl/Cargo.toml::default`
- R2. `scripts/check-mod.sh` needs `claude`, `node` and `npx`; the Windows cross-check needs `rustup target add x86_64-pc-windows-msvc`. Evidence: `scripts/check-mod.sh`
- R3. On Windows build with the MSVC toolchain, Visual Studio Build Tools (their CMake on `PATH`; in Git Bash add `/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin`), LLVM with `LIBCLANG_PATH` at its `bin`, and for a release (`--features vulkan`) the Vulkan SDK in `VULKAN_SDK`. Evidence: `scripts/package-windows.ps1::VULKAN_SDK`
- R4. Package Windows with `scripts/package-windows.ps1 [-Phase all | stage | package]` → MSI and zip in `dist`; it builds in `ECHO_BUILD_DIR` (default `<system drive>\echo-build`) and stages in `ECHO_STAGE_DIR`, so signing runs between `stage` and `package`. Evidence: `scripts/package-windows.ps1::ECHO_BUILD_DIR`
- R5. Package macOS with `scripts/package-macos.sh` (signed, notarized app and dmg); `LOCAL=1` builds an ad-hoc signed `dist/Echo.app` for permission work on this Mac. Evidence: `scripts/package-macos.sh`
- R6. A dev build gets the libraries it loads at run time from `scripts/fetch-pdfium.sh <dest>` and `scripts/fetch-nemo-speech.sh <dest>` (Apple Silicon), pointed at by `ECHOCTL_PDFIUM_DIR` / `ECHOCTL_NEMO_DIR`. Evidence: `nemo::candidates`

## Run and debug

### Rules
- R1. A dev app runs beside the installed Echo only with its own folders: `ECHOCTL_CONFIG_DIR=<dir>` and `ECHOCTL_HOME=<dir>`, never the real ones. Evidence: `app::record_app_path`
- R2. Run your build with `target/debug/echoctl app run` (`--no-tray` without the icon), never `echoctl app start`; stop it with `echoctl app quit`. Evidence: `app::spawn_app`
- R3. After `LOCAL=1 scripts/package-macos.sh` macOS asks for every permission again; clear a stale one with `tccutil reset <service> ai.onetest.echo`. Evidence: `scripts/package-macos.sh::refuse_if_running`
- R4. Check permissions with `echoctl doctor` (`--probe`, `--request`, `--open <pane>`, `--json`) while the app runs. Evidence: `permissions::doctor`
- R5. `ECHOCTL_DEBUG=1` prints Echo's diagnostics on stderr, never an environment value. Evidence: `ai::terminal_command`
- R6. Review window changes in a browser with `echoctl window --serve <port>` on a dev data folder (`ECHOCTL_SERVE_IPC=1` to let clicks reach the app); `--export <dir>` writes it as static files; `--selftest` checks the real webview. Evidence: `window::serve`
- R7. Try a `plugin/` change in `claude --plugin-dir plugin` after `scripts/build-plugin.sh`. Evidence: `README.md::plugin-dir`
- R8. Look for failures in `<home>/.echo/app.log` (the app), `<call>/claude.log` and `<call>/notes-run.json` (a minutes run), `<call>/transcribe.error` (transcription). Evidence: `notes::LOG`

### Read first
| When you are | Exemplar |
|---|---|
| reviewing a window change against made-up data | `window::serve` |
| running a second Echo for development | `paths::config_dir` |
