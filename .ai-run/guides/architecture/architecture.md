# Architecture Guide

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: where code goes, how the app, CLI and plugin talk, background work, feature gates, per-OS files

## Where code goes

| Component | Place |
|---|---|
| App (watcher loop) | `watch.rs`, `app.rs` |
| Recording and capture | `recorder.rs`, `audio/`, `ax/`, `permissions/` |
| Transcription | `live.rs`, `transcribe.rs`, `nemo.rs`, `remote.rs` |
| CLI | `main.rs` (`Cmd` and its dispatch), `cli/<area>.rs` |
| Tray and window | `tray.rs`, `window/<area>.rs`, `terminal.rs`, `ui/` |
| AI jobs | `ai.rs`, `livenotes.rs`, `notes.rs`, `guard.rs`, `projectsync.rs` |
| Threads and ledger | `projects.rs`, `ledger.rs`, `people.rs`, `tasks.rs` |
| Recall (Brain) | `brain/` (feature `brain`) |
| Echo plugins | `plugins.rs`, `jshost.rs`, `claudejob.rs`, `share.rs` (feature `js`) |
| Claude Code plugin | `plugin/` |

## Structure and boundaries

### Rules
- R1. Code that needs an optional dependency (whisper-rs, rusqlite, rquickjs, tray-icon/tao/wry) sits behind `#[cfg(feature = "transcribe"|"brain"|"js"|"tray")]`, its `mod` line in `main.rs` included. Evidence: `crates/echoctl/Cargo.toml::features`
- R2. Each OS's implementation lives in `<area>/macos.rs` and `<area>/windows.rs`, `mod.rs` dispatching by `#[cfg]` with an `unsupported platform` arm. Evidence: `permissions::report`
- R3. Capture, accessibility and permission code goes in the per-OS files; elsewhere a platform API is called only inside a `#[cfg(target_os = …)]` block, except in `app.rs`, `tray.rs`, `frames.rs`, `ai.rs` and `main.rs`. Evidence: `app::FreeConsole`
- R4. Recording never depends on Claude Code, a model or a sign-in; AI jobs fail on their own with a reason. Evidence: `ai::pin`
- R5. Only a Claude Code run writes a call's `notes.md` and action list, applied through `calls::apply_actions`; the app writes transcripts and live drafts, never minutes. Evidence: `calls::apply_actions`
- R6. The Claude Code plugin reaches the app only through the CLI (`${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`) and the data folder's files. Evidence: `plugin/scripts/echoctl::ECHOCTL_BIN`
- R7. One app per user, held by the OS lock (`app.lock` through `app::FileLock`); `watch.json` is only a heartbeat for clients. Nothing else runs a loop of its own on recordings: act through the running app. Evidence: `app::try_lock`, `watch::running_watcher`
- R8. The window's page never reads or writes files: it goes through an `/api/...` route or an IPC message. Evidence: `window::handle`
- R9. A source file stays under 1,500 lines (Rust counted up to its `#[cfg(test)] mod tests`, tests last) and a function under 150; the PR that would cross a limit splits first, in a move-only commit. `scripts/size-check.sh` enforces files against `scripts/size-ceilings.txt` (a listed file may shrink, never grow; lower its ceiling when it does). Vendored and generated files are exempt. Evidence: `scripts/size-check.sh`
- R10. New behaviour goes into its area's own files (`ui/app/NN-<area>.js`, `ui/css/NN-<area>.css`, `window/<area>.rs`, `cli/<area>.rs`); a shared file (`window/mod.rs`, `cli/mod.rs`, `main.rs`, `00-core.js`, `90-render.js`) changes only to register a new area. Evidence: `build.rs::join_parts`
- R11. A split or move commit contains no edits: joined assets stay byte-identical, a CLI move keeps `echoctl help` identical; its PR names the check. Evidence: `build.rs::join_parts`

## Patterns

### Rules
- R1. The CLI makes the running app act by writing a control file in `paths::state_dir()` that the watcher takes (as `watch::stop` writes `stop.request`), and adds its name to the stale-request cleanup list at the top of `watch::run_loop`. Evidence: `watch::run_loop`
- R2. Deterministic work (matching, filtering, gates, preflight) is a CLI command that prints JSON; the skill keeps only judgment. Evidence: `calls::minutes_gate`
- R3. Clients learn what happened from `events.jsonl` (`watch::emit`), tailed, never replayed. Evidence: `watch::emit`
- R4. Long or periodic work runs on its own thread from a `start_scheduler`, never on the watcher's or the event loop's thread, and reports through a state file the window polls. Evidence: `notes::start_scheduler`
- R5. A per-call request from the CLI or the page is one named file (`notes-<call>.request`) the app takes in one handler. Evidence: `notes::handle_requests`
- R6. Window actions and tray items run the same code: a Settings flow is a tray `Action` run by name; IPC actions call the CLI's functions. Evidence: `tray::page_action`
- R7. The page polls a cheap revision (`/api/call/<name>/rev`) and fetches the call only when it changed. Evidence: `window::call::call_rev`
- R8. A CLI command is a variant of `Cmd` or of its area's sub-enum in `cli/<area>.rs`, with a doc comment; a dev command is `#[command(hide = true)]`. Evidence: `main::Cmd`
- R9. Echo plugins extend the window through registries and slots (`SLOTS`, `CALL_TABS`, `ACTIVITIES`) and the host through `ctx` services, never by patching Echo's views. Evidence: `SLOTS` (window script)

## Configuration

### Rules
- R1. Every Echo location comes from `paths` (`home`, `state_dir`, `calls_dir`, `config_dir`), never the working folder. Evidence: `paths::home`
- R2. Per-computer settings (AI profiles, sign-ins, models, window layout) live in the config dir; per-data-folder settings (`config.json`) in the state dir. Evidence: `paths::models_dir`

### Read first
| When you are | Exemplar |
|---|---|
| adding a CLI subcommand | `cli::plugin::PluginCmd` |
| adding an app request (CLI → running app) | `watch::stop` |
| adding a window action | `window::settings::handle` |
| adding a background job | `projectsync::start_scheduler` |
| adding a per-OS capability | `permissions::report` |
| reading or writing a setting | `config::Config` |
