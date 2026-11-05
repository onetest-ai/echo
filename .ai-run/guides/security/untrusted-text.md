# Untrusted Text

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: headless-run guard, run allowlists, safe Markdown, path checks

**Design**: `.ai-run/guides/design/ai-and-minutes.md` — how it works and why; the rules here win.

## Runs, rendering and paths

### Rules
- R1. A CLI command a minutes run, project sync or routine must call has to be in that mode's allowlist (`notes::RUN_COMMANDS`, `guard::PROJECT_STEPS`, `guard::ROUTINE_COMMANDS`, `guard::ASK_COMMANDS`); the guard refuses any other Bash call, so a skill step using a new command fails until it is added. Evidence: `crates/echoctl/src/guard.rs::ROUTINE_COMMANDS`
- R2. A CLI command that changes settings, recording, audio retention or anything outside the run's call checks `notes::headless()` and refuses inside a run Echo started (as `memo start` and `notes::refused_in_run` do). Evidence: `crates/echoctl/src/notes.rs::refused_in_run`
- R3. Turn Markdown into HTML with `minutes::markdown_html` (raw HTML shown as text, https links only, no images); the crate has no `pulldown_cmark::html::push_html` call. Evidence: `crates/echoctl/src/minutes.rs::markdown_html`
- R4. A path that comes from a run, a plugin or the page is checked as relative with only normal components before it is joined, then canonicalised and kept inside its root (as `grants::resolve` does: no `..`, links resolved and re-checked, Windows device names refused). Evidence: `crates/echoctl/src/grants.rs::resolve`
- R5. Read a run's scope (its call, previous minutes, CLI names) only from the environment Echo set (`ECHOCTL_RUN_CALL`, `ECHOCTL_RUN_PREVIOUS`, `ECHOCTL_RUN_CLIS`), never from the tool call's input; without that environment the guard allows nothing. Evidence: `crates/echoctl/src/guard.rs::RUN_CALL`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Let a minutes run write Echo's own call files (`meta.json`, `actions.json`, `my-notes.md`) or instruction files | Write only `notes.md` and `actions.new*.json`, applied through `echoctl actions --apply` | `meta.json` would pin the call to another AI profile; `CLAUDE.md` imports load files with no tool call | `crates/echoctl/src/guard.rs::own_call` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| changing what a headless run may do | `crates/echoctl/src/guard.rs::own_call` | resolves the run's call canonically so `other/../own` can't escape | `crates/echoctl/src/guard.rs::the_cli_works_only_on_the_runs_call` |
