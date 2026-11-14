# Untrusted Text

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: headless-run guard, run allowlists, safe Markdown, path checks

## Runs, rendering and paths

### Rules
- R1. A CLI command a minutes run, project sync or the window's session on a call must call has to be in that mode's allowlist (`notes::RUN_COMMANDS`, `guard::PROJECT_STEPS`, `guard::ASK_COMMANDS`); the guard refuses any other Bash call. A routine's or Yolo's run has no run guard: it runs as the folder session (R20). Evidence: `guard::PROJECT_STEPS`
- R2. A CLI command that changes settings, recording, audio retention or anything outside the run's call refuses inside a run Echo started: it checks `notes::run_limited()` (every run but a routine acting on its own, R21), as `memo start` does; the always-on vocabulary and keeping a call's audio (`notes::refused_in_run`) and Echo plugin installs and updates (`cli::plugin::refuse_in_run`) check `notes::headless()`, refused in every run — except a mail draft (`echoctl compose`, `m365 mail draft`). Evidence: `notes::run_limited`, `notes::refused_in_run`
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
- R20. Give the folder session (`terminal::start_folder`), a routine (`ctx.claude.run`) and a Yolo run their context and permissions from one builder, `folder_args::session` (a routine and Yolo add only `-p`, the prompt and the model, and start in the data folder), and let the user's one choice, "Claude acts on its own" (`Config::folder_skip_permissions`), set all three. A run with `tools: "none"` (Won't do, a plugin's analysis of what it read) keeps no tools whatever the choice; minutes, project sync, the window's session on a call and live notes keep their own runs and guards. Evidence: `folder_args::session`, `claudejob::run`
- R21. Acting on its own (`notes::run_acts`): no permission prompts; a routine's run has the CLI's limits for runs lifted, as the folder session has none (`notes::run_limited`), and a routine or Yolo may send; Yolo keeps its own task's and thread's limits (R9); Echo plugin installs stay refused in every run (R2). Evidence: `notes::run_limited`, `m365::not_in_run`
- R22. Preparing and asking (the choice off): Echo's reads and its own data's edits run without a prompt (`folder_args::FREE_COMMANDS`, each a fixed command and its own arguments, `project work` among them); a Microsoft 365 send is refused, pointing to a draft (`compose --body`); a plugin command's host half is read-only and only `readonly` scripts run (`guard::RUN_MODE=routine`: `jshost::read_only_here`, `plugins::scripts::run`); a thread's work refuses `delete`, `merge`, `xref`, `unxref`, `undo`, `accept`, `reject`, `state: cancelled` and a task's `mode`, `prompt` or `skill` (`cli::project::refused_when_asking`); anything else that would ask isn't done, and the run says why. Evidence: `folder_args::FREE_COMMANDS`, `cli::project::refused_when_asking`
- R23. Note every change and send a routine's or Yolo's run makes with Echo's CLI or a plugin (`notes::note_action`, `notes::note`) in its `actions.jsonl`, and give them in its outcome: its log, `ctx.claude.run`'s `actions`, Yolo's changes (`notes::run_actions`). Record its edits of a thread's work `by: "run"` through `ledger::human_event`, applied as the user's would be (`ledger::by_hand`), and show a task's `mode`, `prompt` or `skill` a run set as the run's (`Node::by_run`, `taskmodes::of`) until the user sets it. A Claude run started inside one drops its markers (`ai::PARENT_SESSION`) unless `claudejob` sets them; a plugin script inside one inherits them (`plugins::scripts::run_markers`). Evidence: `notes::run_actions`, `Node::by_run`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Let a minutes run write Echo's own call files (`meta.json`, `actions.json`, `my-notes.md`) or instruction files | Write only `notes.md` and `actions.new*.json`, applied through `echoctl actions --apply` | `guard::own_call` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| changing what a headless run may do | `guard::own_call` | `guard::the_cli_works_only_on_the_runs_call` |
