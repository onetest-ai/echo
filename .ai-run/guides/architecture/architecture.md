# Architecture Guide

**Project**: Echo · **Scope**: where code goes, how the app, CLI and plugin talk, background work, feature gates, per-OS files

**Reference**: `.ai-run/reference/architecture/architecture.md` — the code map, file names, limits and mechanisms these
rules rely on (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Structure and boundaries

### Rules
- R1. Code that needs an optional dependency sits behind its feature gate, its `mod` line included. Evidence: `crates/echoctl/Cargo.toml::features` (ref: Feature gates)
- R2. Each OS's implementation lives in its own per-OS file, dispatched by `#[cfg]` with an unsupported-platform arm. Evidence: `permissions::report` (ref: Per-OS files)
- R3. Platform APIs are called only from the per-OS files or inside a `#[cfg(target_os = …)]` block, outside the few listed exceptions. Evidence: `app::FreeConsole` (ref: Per-OS files)
- R4. Recording never depends on Claude Code, a model or a sign-in; AI jobs fail on their own with a reason. Evidence: `ai::pin`
- R5. Only a Claude Code run writes a call's minutes and action list, applied through `calls::apply_actions`; the app writes transcripts and live drafts, never minutes. Evidence: `calls::apply_actions`
- R6. The Claude Code plugin reaches the app only through the CLI and the data folder's files. Evidence: `plugin/scripts/echoctl::ECHOCTL_BIN` (ref: The Claude Code plugin's way in)
- R7. One app per user, held by an OS lock; nothing else runs a loop of its own on recordings: act through the running app. Evidence: `app::try_lock`, `watch::running_watcher` (ref: One app per user)
- R8. The window's page never reads or writes files: it goes through an `/api/...` route or an IPC message. Evidence: `window::handle`
- R9. A source file and a function stay under their size limits; the PR that would cross a limit splits first, in a move-only commit, and a file over the limit today never grows. Evidence: `scripts/size-check.sh` (ref: Size limits)
- R10. New behaviour goes into its area's own files; a shared file changes only to register a new area. Evidence: `build.rs::join_parts` (ref: Areas and shared files)
- R11. A split or move commit contains no edits, and its PR names the check that shows it. Evidence: `build.rs::join_parts` (ref: Areas and shared files)

## Patterns

### Rules
- R1. The CLI makes the running app act only by a control file the watcher takes, and every control file is cleaned up when stale. Evidence: `watch::run_loop` (ref: App requests)
- R2. Deterministic work (matching, filtering, gates, preflight) is a CLI command that prints JSON; the skill keeps only judgment. Evidence: `calls::minutes_gate`
- R3. Clients learn what happened from the event log, tailed, never replayed. Evidence: `watch::emit` (ref: Events, background work, polling)
- R4. Long or periodic work runs on its own thread, never on the watcher's or the event loop's thread, and reports through state the window polls. Evidence: `notes::start_scheduler` (ref: Events, background work, polling)
- R5. A per-call request from the CLI or the page is one named file the app takes in one handler. Evidence: `notes::handle_requests` (ref: App requests)
- R6. Window actions and tray items run the same code, never copies of it. Evidence: `tray::page_action` (ref: Shared code between window and tray)
- R7. The page polls a cheap revision and fetches a call only when it changed. Evidence: `window::call::call_rev` (ref: Events, background work, polling)
- R9. Echo plugins extend the window and the host only through registries, slots and `ctx` services, never by patching Echo's views. Evidence: `SLOTS` (window script) (ref: Plugin extension points)

## Configuration

### Rules
- R1. Every Echo location comes from `paths`, never the working folder. Evidence: `paths::home` (ref: Locations and settings)
- R2. Per-computer settings live in the config dir, per-data-folder settings in the state dir. Evidence: `paths::models_dir` (ref: Locations and settings)
