# Architecture — reference

How Echo is built: where code lives, the mechanisms the rules rely on, their files and names. It changes with the code,
in the same PR. The rules are in `.ai-run/guides/architecture/architecture.md`.

**Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs.

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

## Feature gates

Optional dependencies sit behind `#[cfg(feature = …)]`, the module's `mod` line in `main.rs` included
(`crates/echoctl/Cargo.toml::features`):

| Feature | Brings in |
|---|---|
| `transcribe` | whisper-rs (whisper.cpp), ureq, libloading |
| `tray` | tray-icon, tao, wry, portable-pty |
| `brain` | rusqlite, zip, quick-xml, calamine, pdfium-render, regex |
| `js` | rquickjs |
| `vulkan` | `transcribe` + whisper.cpp on the GPU (Windows/Linux) |

Default features: `transcribe`, `tray`, `brain`, `js`.

## Per-OS files

- Each OS's implementation lives in `<area>/macos.rs` and `<area>/windows.rs`; `mod.rs` dispatches by `#[cfg]` with an
  `unsupported platform` arm (`permissions::report`).
- Capture, accessibility and permission code is in those files. Outside them a platform API is called only inside a
  `#[cfg(target_os = …)]` block, except in `app.rs`, `tray.rs`, `frames.rs`, `ai.rs` and `main.rs` (`app::FreeConsole`).

## One app per user

- The OS lock is `app.lock`, taken through `app::FileLock` (`app::try_lock`).
- `watch.json` is only a heartbeat for clients (`watch::running_watcher`).

## The Claude Code plugin's way in

The plugin calls the CLI through `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (`plugin/scripts/echoctl::ECHOCTL_BIN`) and
reads the data folder's files.

## Size limits

- A source file: under 1,500 lines; Rust counted up to its `#[cfg(test)] mod tests` (tests last), the window's parts
  (`ui/app/*.js`, `ui/css/*.css`) whole. A function: under 150 lines.
- `scripts/size-check.sh` (run by `scripts/check-mod.sh`) checks files against `scripts/size-ceilings.txt`: a file over
  the limit today is listed there with its size; it may shrink, never grow, and its ceiling is lowered when it shrinks.
- Vendored and generated files are exempt.

## Areas and shared files

- An area's own files: `ui/app/NN-<area>.js`, `ui/css/NN-<area>.css`, `window/<area>.rs`, `cli/<area>.rs`.
- Shared files, changed only to register a new area: `window/mod.rs`, `cli/mod.rs`, `main.rs`, `00-core.js`,
  `90-render.js` (`build.rs::join_parts`).
- A move's checks: joined assets stay byte-identical; a CLI move keeps `echoctl help` identical.

## App requests (CLI → running app)

- The CLI writes a control file in `paths::state_dir()` that the watcher takes, as `watch::stop` writes `stop.request`.
- Every control file's name is in the stale-request cleanup list at the top of `watch::run_loop`.
- A per-call request from the CLI or the page is one named file, `notes-<call>.request`, taken in one handler
  (`notes::handle_requests`).

## Events, background work, polling

- Clients learn what happened from `events.jsonl`, written by `watch::emit`, tailed and never replayed.
- Long or periodic work runs on its own thread started from a `start_scheduler` (`notes::start_scheduler`,
  `projectsync::start_scheduler`) and reports through a state file the window polls.
- The page polls a call's revision, `/api/call/<name>/rev` (`window::call::call_rev`), and fetches the call only when it
  changed.

## CLI layout

A command is a variant of `Cmd` (`main::Cmd`) or of its area's sub-enum in `cli/<area>.rs`, with a doc comment; a dev
command is `#[command(hide = true)]`.

## Shared code between window and tray

A Settings flow is a tray `Action` run by name; IPC actions call the CLI's functions (`tray::page_action`).

## Plugin extension points

The window's registries and slots: `SLOTS`, `CALL_TABS`, `ACTIVITIES` (window script); the host's: `ctx` services. See
`.ai-run/guides/plugins/echo-plugins.md`.

## Locations and settings

- Locations: `paths::home`, `paths::state_dir`, `paths::calls_dir`, `paths::config_dir`.
- The config dir (per computer): AI profiles, sign-ins, models, window layout (`paths::models_dir`).
- The state dir (per data folder): `config.json`.

## Exemplars

| When you are | Exemplar |
|---|---|
| adding a CLI subcommand | `cli::plugin::PluginCmd` |
| adding an app request (CLI → running app) | `watch::stop` |
| adding a window action | `window::settings::handle` |
| adding a background job | `projectsync::start_scheduler` |
| adding a per-OS capability | `permissions::report` |
| reading or writing a setting | `config::Config` |
