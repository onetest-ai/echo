# Architecture Guide

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the system map, where code goes: paths, control files, CLI vs skill, background work, feature gates, per-OS files

## System overview

Echo records the Teams calls the user opts in to (their mic + Teams' audio, two tracks), transcribes them on-device,
and has Claude Code write the minutes. One Rust binary, `echoctl`, is both the app and the CLI.

| Component | Where | Role | Talks to others by |
|---|---|---|---|
| App (watcher loop) | `crates/echoctl/src/watch.rs`, `app.rs` | detects calls, records, transcribes, schedules jobs; one per user | takes control files in `<home>/.echo/`, writes `watch.json`, appends `events.jsonl` |
| Recorder + capture | `recorder.rs`, `audio/`, `ax/`, `permissions/` | two `audio::Track`s, Teams UI reading, OS permissions | WAVs and `*.jsonl` in the call folder |
| Transcription | `live.rs`, `transcribe.rs`, `nemo.rs`, `remote.rs` | live pass during the call, final transcript after | reads the growing WAVs, writes `live.jsonl`, `transcript.md`, `segments.json` |
| CLI | `main.rs` (`Cmd` and its dispatch), `cli/<area>.rs` (an area's sub-enums and what they run) | every user / plugin / skill entry point; prints JSON | request files for the running app; direct reads of the data folder |
| Tray + window | `tray.rs`, `window/` (one file per area), `terminal.rs`, `ui/` | menu-bar icon; a wry webview served over `echo://`; Claude in a PTY | HTTP-like `/api/...` routes (`window::respond`), IPC messages (`window::handle`) |
| AI jobs | `ai.rs`, `livenotes.rs`, `notes.rs`, `guard.rs`, `projectsync.rs` | live notes, minutes runs, thread syncs on the call's AI profile | headless Claude Code under the `run-guard` hook; state files per call / thread |
| Threads and ledger | `projects.rs`, `ledger.rs`, `people.rs`, `tasks.rs` | clients, threads (projects), work ledger, people | `<home>/projects/` files |
| Recall (Brain) | `brain/` (feature `brain`) | SQLite index of calls, mail, posts, folders; MCP server | `<home>/recall/`, `echoctl brain mcp` |
| Echo plugins | `plugins.rs`, `jshost.rs`, `claudejob.rs`, `share.rs` (feature `js`) | third-party packages with a host half (QuickJS) and a client half (window) | `ctx` services, slots and registries |
| Claude Code plugin | `plugin/` | status-line mod + skills (`process-call`, `manage`, `record`, ...) | only `plugin/scripts/echoctl` and the data folder's files |

Process and ownership:
- One app per user: `app.lock` in the config dir, plus the `watch.json` heartbeat that clients read to know it runs.
- The CLI is a client: commands that need the app write a request file; the app's loop takes it within a second.
- The Claude Code plugin never links or calls the app directly; it runs the CLI and reads the data folder.
- Claude Code is started by Echo only headless (minutes, syncs, routines) under `guard.rs`, or in the window's PTY.
- The window is a webview over the `echo://` protocol (`http://echo.localhost` on Windows): compiled-in `ui/` files and JSON routes; it never touches files itself.

Main flow:
1. The agenda (`agenda.json`) or a detected call offers a meeting; `arm` writes `armed.json` (opt-in, single use).
2. The watcher sees Teams holding the mic with an arm present → `recorder::Recording::start`: `mic.wav` + `remote.wav`, aligned on one clock.
3. `live::start` decodes finished utterances as the WAVs grow → `live.jsonl`; `livenotes` drafts `live-notes.json`.
4. Stop / call end → `Recording::finish` → `transcribe::write_transcript` (order, echo filter, names; audio deleted).
5. `notes::start_scheduler` admits a minutes run when `notes::auto_due` says so → headless Claude Code runs `/echo:process-call` → `notes.md`, `actions.json`.
6. `projectsync::start_scheduler` syncs each open thread's ledger from new calls, chats and mail; `brain::sync` indexes them for Recall.

Evidence: `watch::run_loop`, `recorder::Recording`, `transcribe::write_transcript`

## Data layout

| Place | Resolved by | Key files | Written by |
|---|---|---|---|
| Config dir (fixed per OS) | `paths::config_dir` | `home.json` (the chosen data folder), `models/`, `app.lock`, `app-path`, `ai.json`, `graph.json`, `claude/<profile>/`, `window-layout.json`, `plugins.json`, `plugins/` | app and CLI; secrets 0600 |
| Data folder (home) | `paths::home` (`ECHOCTL_HOME` → `home.json` → config dir) | `calls/`, `.echo/`, `projects/`, `recall/` | app and CLI |
| State dir | `paths::state_dir` = `<home>/.echo` | `config.json`, `armed.json`, `*.request`, `quit.asking`, `health.json`, `events.jsonl`, `watch.json`, `agenda.json`, `calendar-failed.json` (the last failed sync, until a good one), `scheduled.json`, `app.log` | CLI writes requests; the watcher takes them and writes state |
| A call folder | `paths::calls_dir` + `<ts>-<slug>` (`calls::stamp_of` keeps its identity across retitles) | `meta.json`, `meeting.json`, `mic.wav` / `remote.wav` (until transcribed), `live.jsonl`, `live-notes.json`, `transcript.md`, `segments.json`, `my-notes.md`, `chat.json`, `frames/` | recorder and transcription |
| ... its minutes | same folder | `notes.md`, `actions.json`, `notes-run.json`, `claude.log` | only a Claude Code run (`notes.md`, actions via `echoctl actions --apply`); `notes-run.json` by `notes.rs` |

Evidence: `paths::state_dir`, `paths::calls_dir`, `paths::config_dir`

## Structure and boundaries

### Rules
- R1. Put code that needs an optional dependency (whisper-rs, rusqlite, rquickjs, tray-icon/tao/wry) behind `#[cfg(feature = "transcribe"|"brain"|"js"|"tray")]`, its `mod` line in `main.rs` included (the Windows cross-check builds `--no-default-features`, so ungated use fails it). Evidence: `crates/echoctl/Cargo.toml::features`
- R2. Keep each OS's implementation in `<area>/macos.rs` and `<area>/windows.rs` with `mod.rs` dispatching by `#[cfg]` and an `unsupported platform` arm for other targets, as `permissions::report` does. Evidence: `permissions::report`
- R3. Capture, accessibility and permission code goes in the per-OS files (`<area>/macos.rs`, `<area>/windows.rs`); other modules may call a platform API only inside a `#[cfg(target_os = …)]` block. Allowed exceptions: `crates/echoctl/src/app.rs`, `crates/echoctl/src/tray.rs`, `crates/echoctl/src/frames.rs`, `crates/echoctl/src/ai.rs`, `crates/echoctl/src/main.rs`. Evidence: `app::FreeConsole`
- R4. Recording never depends on Claude Code, a model or a sign-in: the app asks with native dialogs and records whatever the AI profile's state; AI jobs fail on their own with a reason. Evidence: `ai::pin`
- R5. Only a Claude Code run writes a call's `notes.md` and action list (the app writes transcripts and live drafts, never minutes); a run applies its list through `calls::apply_actions`. Evidence: `calls::apply_actions`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Make the Claude Code plugin call the app any way but the CLI and the data folder's files | Run `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl <command>` | the plugin ships no binary and must work with any installed Echo | `plugin/scripts/echoctl::ECHOCTL_BIN` |
| Start a second app or a loop of your own to act on recordings | Act through the running app (request file) and check `watch::running_watcher` | one app per user owns the recording and the call's `meta.json` | `watch::running_watcher` |
| Let the window's page read or write files | Add an `/api/...` route or an IPC message in its area's `route` / `handle` (`src/window/<area>.rs`) | the page is untrusted-text territory under a strict CSP | `window::handle` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a per-OS capability | `permissions::report` | macOS / Windows / unsupported arms, implementations in sibling files | no covering test |
| following a recording end to end | `recorder::Recording` | start, memo start and finish of both tracks and the live worker | no covering test |

## Patterns in use

### Rules
- R1. To make the running app act from the CLI, write a control file in `paths::state_dir()` that the watcher loop takes (as `watch::stop` writes `stop.request`), and add its name to the stale-request cleanup list at the top of `watch::run_loop` (a request left by a previous run must not act on this one). Evidence: `watch::run_loop`
- R2. Put deterministic work (matching, filtering, gates, preflight) in a CLI command that prints JSON, and leave only judgment to the skill that calls it (the minutes gate is computed once in `calls::minutes_gate` and shared by `context` and the scheduler). Evidence: `calls::minutes_gate`
- R3. Keep the single instance on the OS lock (`app.lock` through `app::FileLock`), not on the heartbeat alone; the heartbeat (`watch.json`) only tells clients the app is alive. Evidence: `app::try_lock`
- R4. Tell clients what happened by appending an event to `events.jsonl` with `watch::emit`; clients (the status-line mod, `events --follow`) tail it and never replay. Evidence: `watch::emit`
- R5. Run long or periodic work on its own thread started by a `start_scheduler` (minutes, thread syncs, Recall), never on the watcher's or the event loop's thread, and report progress through a state file the window polls. Evidence: `notes::start_scheduler`
- R6. A per-call request from the CLI or the page is a named file (`notes-<call>.request`) the app picks up in one handler, so the CLI and the window start the same run. Evidence: `notes::handle_requests`
- R7. Window actions and tray menu items run the same code: a Settings flow is a tray `Action` run by name, and IPC actions call the CLI's own functions. Evidence: `tray::page_action`
- R8. The page learns of changes by polling a cheap revision (`/api/call/<name>/rev` over every file of the call) and fetches the call only when it changed. Evidence: `window::call::call_rev`
- R9. Shared JSON files change by lock + re-read + atomic write (`calls::update_meta` for `meta.json`, `paths::write_atomic`); the details live in `development/rust-conventions.md`. Evidence: `calls::update_meta`
- R10. Call text goes only to the AI profile pinned for that call (`ai::for_call`; never another profile as a fallback); how runs start is in `integrations/claude-code-runs.md`. Evidence: `ai::for_call`
- R11. A headless run may use only the CLI commands its mode allows (`notes::RUN_COMMANDS`, `guard::PROJECT_STEPS`, `guard::ROUTINE_COMMANDS`); transcripts, mail and chat are others' words — see `security/untrusted-text.md`. Evidence: `notes::RUN_COMMANDS`
- R12. Echo plugins extend the window through registries and slots (`SLOTS`, `CALL_TABS`, `ACTIVITIES`) and the host through `ctx` services, never by patching Echo's views; see `plugins/echo-plugins.md`. Evidence: `SLOTS` (window script)
- R13. **Size limits.** A source file stays under 1,500 lines (Rust counted up to its `#[cfg(test)] mod tests`, so tests go at the end and no production code after them) and a function under 150: the PR that would cross a limit first splits the file or function in a separate move-only commit. `scripts/size-check.sh` (run by `scripts/check-mod.sh`) enforces the file limit (the function limit is a review rule); a file already over it is listed in `scripts/size-ceilings.txt` with its size then, and may shrink but never grow (lower its ceiling when it shrinks). Vendored and generated files are exempt. Evidence: `scripts/size-check.sh`
- R14. **One feature, its own files.** New behaviour goes into the files of its area, not into a shared file: the window's script and styles in `ui/app/NN-<area>.js` and `ui/css/NN-<area>.css`, page routes and IPC commands in `src/window/<area>.rs` (its `route`/`handle`), CLI subcommands in `src/cli/<area>.rs` (its enum and `run`). A shared file (`window/mod.rs`, `cli/mod.rs`, `main.rs`, `00-core.js`, `90-render.js`) changes only to register a new area. Evidence: the hotspot study in #126 (47 of 62 PRs touched the one `app.js`)
- R15. **Move-only commits are provable.** A split or move commit contains no edits; joined assets stay byte-identical to the file they replace, and a CLI move keeps `echoctl help` identical for every subcommand. Its PR names that check. Evidence: `crates/echoctl/build.rs::join_parts`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Put a new CLI command anywhere but a variant of `Cmd` in `main.rs` or of its area's sub-enum in `cli/<area>.rs` | Add the variant with a doc comment; hide dev ones with `#[command(hide = true)]`; a body longer than a line goes in the area's `cli/` module | clap's help and the skills' references read from it | `main::Cmd` |
| Add a new app request without a cleanup entry | Add it to the list at the top of `watch::run_loop` | a stale request would act on the next start | `watch::run_loop` |
| Give a headless run a new command by widening Bash | Add it to the mode's list (`RUN_COMMANDS`, `PROJECT_STEPS`) and check it in the CLI too | the guard allows only listed commands as plain words | `guard::PROJECT_STEPS` |
| Put model judgment in Rust or deterministic steps in a skill | A JSON CLI command for facts, the skill step for judgment | runs stay cheap, testable and guarded | `calls::minutes_gate` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a CLI subcommand | `cli::plugin::PluginCmd` | clap derive enum with doc comments as help, hidden ones `#[command(hide = true)]` | no covering test |
| adding an app request (CLI → running app) | `watch::stop` | writes `stop.request`, the loop takes and removes it | no covering test |
| adding a window action | `window::settings::handle` | IPC message → shared function → `{refresh, reply}` | no covering test |
| adding a background job | `projectsync::start_scheduler` | own thread, due check, state file, failure backoff | no covering test |
| adding a state file the window shows | `notes::RUN` | `notes-run.json`: one writer, read by `/api/call` and the Inbox | no covering test |
| adding a skill step | `plugin/skills/process-call/SKILL.md` | each step calls one JSON CLI command, then judges | no covering test |
| adding a per-OS capability | `permissions::report` | see Structure and boundaries | no covering test |

## Configuration

### Rules
- R1. Resolve every Echo location through `paths` (`paths::home()`, `paths::state_dir()`, `paths::calls_dir()`, `paths::config_dir()`), never the process's working folder (the CLI also runs from Claude Code sessions whose cwd is a workspace). Evidence: `paths::home`
- R2. Per-computer settings (AI profiles, sign-ins, models, window layout) live in the config dir; per-data-folder settings (`config.json`) in the state dir. Evidence: `paths::models_dir`

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| reading or writing a setting | `config::Config` | atomic `save`, nothing saved after a failed `load` | no covering test |

Related guides: `capture/capture-and-transcription.md`, `window/window-ui.md`, `data/recall-store.md`,
`plugins/plugin-authoring.md`, `development/dev-workflow.md`.

