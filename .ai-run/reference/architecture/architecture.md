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
| Recall (Intake and search) | `intake/` (feature `intake`; `brain` is its alias) |
| Echo plugins | `plugins.rs`, `jshost.rs`, `web/` (a page read as Markdown, the public-address check, the render gate proxy), `netif/` (this computer's own addresses), `claudejob.rs`, `share.rs` (feature `js`) |
| Claude Code plugin | `plugin/` |

## Feature gates

Optional dependencies sit behind `#[cfg(feature = …)]`, the module's `mod` line in `main.rs` included
(`crates/echoctl/Cargo.toml::features`):

| Feature | Brings in |
|---|---|
| `transcribe` | whisper-rs (whisper.cpp), ureq, libloading |
| `tray` | tray-icon, tao, wry, portable-pty |
| `intake` | rusqlite, zip, quick-xml, calamine, pdfium-render, regex |
| `js` | rquickjs |
| `vulkan` | `transcribe` + whisper.cpp on the GPU (Windows/Linux) |

Default features: `transcribe`, `tray`, `intake`, `js`.

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
- Echo's own background jobs are listed in `jobs::JOBS`: Needs you now (`needs`), the thread sync (`thread_sync`), the
  threads' mail and chat fetch (`fetch`), minutes (`minutes`), meeting prep with meeting types and task modes (`prep`),
  Recall's indexing with its mail reader (`brain_reader`) and live notes (`live_notes`). Each has fields, a number
  (with its range and default), `night` (paused in `jobs::night_window`, 22:30–07:00; on by default for Needs you now,
  the thread sync and the fetch, off for prep and Recall) and `model` (haiku, sonnet or opus, which each profile maps;
  empty: the job's own default). A job reads its settings on every tick (`jobs::number`, `jobs::paused`;
  `jobs::model_for` for its model — never on a local profile that maps no model to that name — through
  `livenotes::job_model` for prep, Recall, meeting types and task modes, `needs::needs_model`,
  `livenotes::live_model_of`; minutes and the thread sync after the thread's own model) and a timed one (Needs
  you now, prep, Recall) says when it looks next (`jobs::after_pass`, `<config dir>/jobs-next.json`). A live notes
  session keeps the model it started with until the next call.
- A job's runs are kept as a routine's: `jobs::begin` … `Run::done` / `failed` on the job's thread, its model calls
  counted by `jobs::add_usage` (from `claude_session::complete_claude_with` and `livenotes::complete_local_body`), or
  `jobs::record` for a run elsewhere (the thread sync's passes, a failed sync, the fetch, a live notes session); a
  minutes run is `begin` at Claude Code's start … `Run::minutes`. A look that found nothing and called no model leaves
  no run (`Run::done_if_used`). One line a run, appended, in `<config dir>/jobs-runs/<job>.jsonl` (`started`, `ended`,
  `duration_ms`, `ok`, `summary` or `error`, `model`, `calls`, tokens, `cost_usd`), rewritten once its first run is
  older than 31 days or it grows long, keeping the last 31 days and at most 5000. A run's words are Echo's own and its
  counts: no call, thread or person named, no model text (plugins read them). `jobs::list` gives each job `to` (where it shows), `last`, `next_run` (a time) or
  `next` (in words, for a job that runs on events), `month` (`runs`, `failed`, `cost`, `daily` × 30) and today's
  counts, plus the night and `night_default`; `echoctl jobs runs <job>` its runs. `jobs::routines` /
  `jobs::routine_runs` give the same as routines, the one shape the Scheduler draws every routine from
  (`plugin-api.md` § Routines; `echoctl jobs routines`). The settings are
  `<config dir>/jobs.json`, changed only by `jobs::set` (`echoctl jobs set <job> key=value …`, `key=` its default;
  `night from=HH:MM to=HH:MM`) under `.jobs.lock`: all checked, then written at once, never inside a run Echo started.
  `echoctl jobs` lists them all.
- A plugin's own routines (`plugins::routines`, `plugin-api.md` § Routines) are kept beside Echo's jobs by their id
  `<plugin>.<routine>`: their settings (`on`, `every`, `night`, `model`) in `jobs.json` (`jobs::routine_fields`), their
  runs in `jobs-runs/<id>.jsonl` and each run's folder in `jobs-runs/<id>/<run>/` (the newest 20), its lock
  `jobs-runs/.<id>.lock`. `jobs::routines` lists them after Echo's jobs (`jobs::list` only Echo's); the app runs those
  due every 30 s (`plugins::routines::start_scheduler`, `jobs::routine_next`); `echoctl jobs run <id>` runs one now and
  `echoctl jobs outcome <id> [--run <run>]` gives its answer, changes and sends and log.
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
