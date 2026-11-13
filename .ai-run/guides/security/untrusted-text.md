# Untrusted Text

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: headless-run guard, run allowlists, safe Markdown, path checks

## Runs, rendering and paths

### Rules
- R1. A CLI command a minutes run, project sync or routine must call has to be in that mode's allowlist (`notes::RUN_COMMANDS`, `guard::PROJECT_STEPS`, `guard::ROUTINE_COMMANDS`, `guard::ASK_COMMANDS`; a routine also the plugin commands Echo passes in `guard::RUN_PLUGIN_COMMANDS` and the plugin scripts marked `readonly` in `guard::RUN_PLUGIN_SCRIPTS`, each by its full path with plain words); the guard refuses any other Bash call. Evidence: `guard::ROUTINE_COMMANDS`
- R2. A CLI command that changes settings, recording, audio retention or anything outside the run's call checks `notes::headless()` and refuses inside a run Echo started (as `memo start` and `notes::refused_in_run` do) — except a mail draft (`echoctl compose`, `m365 mail draft`): never sent, the user sends it. Evidence: `notes::refused_in_run`
- R3. Turn Markdown into HTML with `minutes::markdown_html` (raw HTML shown as text, https links only, no images); the crate has no `pulldown_cmark::html::push_html` call. Evidence: `minutes::markdown_html`
- R4. A path that comes from a run, a plugin or the page is checked as relative with only normal components before it is joined, then canonicalised and kept inside its root (as `grants::resolve` does: no `..`, links resolved and re-checked, Windows device names refused). Evidence: `grants::resolve`
- R5. Read a run's scope (its call, previous minutes, CLI names) only from the environment Echo set (`ECHOCTL_RUN_CALL`, `ECHOCTL_RUN_PREVIOUS`, `ECHOCTL_RUN_CLIS`), never from the tool call's input; without that environment the guard allows nothing. Evidence: `guard::RUN_CALL`
- R6. The run guard is a `PreToolUse` hook with `onFailure: block` that refuses whatever it can't judge, a panic too; refuse the run when any settings source could switch hooks off. Evidence: `ai::hooks_conflict`
- R7. A minutes run starts with `--tools` `RUN_TOOLS`, `--strict-mcp-config`, `--permission-mode dontAsk` and never `--add-dir`; Write is governed by the `Edit` rule. Evidence: `notes::args`
- R8. A run reads saved `tool-results` only of its own session (the hook's `transcript_path` + `session_id`), never another session's. Evidence: `guard::own_results`
- R9. A Yolo run (a task's, or an item of Needs you now's) reads only its own thread, and nothing of it changes but its task's state: `echoctl read echo://call/…` gives a call of that thread pinned to the profile the run started on (`guard::RUN_PROFILE`; without it, no call), and the CLI gives its brief, people and work read-only (`guard::run_thread`); never another thread. Evidence: `links::thread_call`, `guard::run_thread`
- R10. Rename a run's call folder only at the run's end, and never use a `title_from_run` title to find previous minutes. Evidence: `calls::rename_to_title`, `calls::previous_subject`
- R11. Initialise mermaid on a minutes page only with `securityLevel: 'strict'`. Evidence: `minutes::MERMAID_INIT`
- R12. A minutes run changes nothing outside its call except learned vocabulary for its meeting and pruned frames moved to the trash; the app, not the run, clears trash entries older than 30 days. Evidence: `calls::purge_trash`
- R13. A run on a Claude Code older than `guard::MIN_CLAUDE` is stopped before it does anything. Evidence: `guard::new_enough`
- R14. Inside a minutes run, the CLI reaches only the run's own call. Evidence: `guard::own_call`
- R15. The minutes render in an `<iframe sandbox="allow-scripts">`; the window is served with `window::reply`'s CSP (`'self'`/`echo:`), and the minutes page with its own hashed CSP through `window::page_reply`. Evidence: `minutes::render`, `window::page_reply`
- R16. `echoctl window --serve` answers only on 127.0.0.1, and its `POST /ipc` (with `ECHOCTL_SERVE_IPC=1`) only with that run's SameSite=Strict cookie and its own Origin. Evidence: `window::serve`
- R17. Secrets (`ai.json`, `plugins/secrets/<id>.json`) are written 0600; a profile's values reach Claude Code only as `ECHOCTL_P_<name>` variables, never in a script's text. Evidence: `ai::PASS`
- R18. An update is installed only verified: on Windows against the release's digest, on macOS with Echo's team signature; never while a call records or before the window's notes are saved. Evidence: `update::commit`
- R19. `<echo_view>` goes only to Echo's own terminals (`ECHOCTL_TERMINAL`), never to runs, plugins or other sessions. Evidence: `view::SESSION_VAR`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Let a minutes run write Echo's own call files (`meta.json`, `actions.json`, `my-notes.md`) or instruction files | Write only `notes.md` and `actions.new*.json`, applied through `echoctl actions --apply` | `guard::own_call` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| changing what a headless run may do | `guard::own_call` | `guard::the_cli_works_only_on_the_runs_call` |
