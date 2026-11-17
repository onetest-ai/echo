# Untrusted Text — reference

How the guards, allowlists, safe writers and path checks are built: their lists, flags, variables and functions. It
changes with the code, in the same PR. The rules are in `.ai-run/guides/security/untrusted-text.md`.

## Run allowlists

The CLI commands each guarded mode may call (the guard refuses any other Bash call):

| Mode | Allowlist | Commands (on main) |
|---|---|---|
| minutes run | `notes::RUN_COMMANDS` | `context`, `actions`, `speakers`, `title`, `type`, `vocab add`, `frames`, `transcribe`, `memo threads` |
| project sync | `guard::PROJECT_STEPS` | `apply`, `done`, `people-apply` (what it reads is in its prompt; its verify run has no tools) |
| the window's session on a call (ask) | `guard::ASK_COMMANDS` | `context` |

A routine's or Yolo's run has no run guard: it runs as the folder session (One builder, below).

## Refusing inside a run

- `notes::run_limited()`: true in every run Echo started but a routine acting on its own. A CLI command that changes
  settings, recording, audio retention or anything outside the run's call checks it, as `memo start` does.
- `notes::headless()`: true in every run. Checked by the always-on vocabulary and keeping a call's audio
  (`notes::refused_in_run`) and by every `echoctl plugin` command that changes plugins (`cli::plugin::refuse_in_run`):
  install, update, remove, enable, disable, `set` with a value, `marketplace add / remove / hide / show`.
- A mail draft is not refused: `echoctl compose`, `m365 mail draft`.

## Safe Markdown

`minutes::markdown_html` turns Markdown into HTML: raw HTML shown as text, https links only, no images. The crate has no
`pulldown_cmark::html::push_html` call.

A plugin's vault page (`ctx.markdown(md, opts)`) goes through the same writer, `minutes::page_html`, its links and
images as the plugin resolved them and `plugins::markdown::render` allows: a link to `https://`, an `echo://` link Echo
knows or the plugin's own `#<id>:` or `#brain/<id>:` route; an image only from that plugin's folder route `plugin/<id>/folder/…` (no dot
segment, encoded or not), never `data:` or another host; nothing with whitespace, control characters or a backslash.
Anything else is text; a page that isn't there is a link with no href (`data-md-missing`); a mermaid fence is code.

## Path checks

As `grants::resolve` does: the path is checked as relative with only normal components before it is joined (no `..`),
then canonicalised, links resolved and re-checked, kept inside its root; Windows device names refused.

## Run scope variables

A run's scope is read from the environment Echo set: `ECHOCTL_RUN_CALL` (its call), `ECHOCTL_RUN_PREVIOUS` (previous
minutes), `ECHOCTL_RUN_CLIS` (the CLI's names) (`guard::RUN_CALL`). A sync run also gets `ECHOCTL_RUN_LEDGER`,
`ECHOCTL_RUN_DIR`, `ECHOCTL_RUN_PROJECT` and `ECHOCTL_RUN_ITEMS` (the items it may mark done; the guard and the CLI
refuse `done` and `apply --done` for any other). A sync run reads nothing with a tool (the guard refuses Read, Glob, Grep
and Edit): Echo gives it its text, each item in an evidence block whose tag is random for the run.

## The run guard

- A `PreToolUse` hook with `onFailure: block`; it refuses whatever it can't judge, a panic too.
- `ai::hooks_conflict` refuses the run when any settings source could switch hooks off.
- `guard::MIN_CLAUDE` (2.1.295 on main) is the oldest Claude Code whose hooks block when they fail; `guard::new_enough`
  stops an older one.
- A run reads saved `tool-results` only of its own session: the hook's `transcript_path` + `session_id`
  (`guard::own_results`).
- Hooks the user installed, directly or through a wrapper, are the user's code and can change what a tool call runs
  after the guard allowed it; the guard judges what Claude asks for. (Claude Code runs matching hooks in parallel, the
  strictest decision wins, and another hook's `updatedInput` replaces the input the tool runs with.)
- Through a wrapper profile, the run proves Echo's settings are in force before anything acts on its text
  (`ai::wrapper`; claude-code-runs reference, Wrapper profiles): the gate holds every tool call until Echo has checked
  the run's start event. Its marks are in `.plugins/.proof/`, where the sessions' guards refuse an Edit or Write and,
  best effort, a Bash line that writes there; each is named by its run's value, which is in that run's own settings
  file (owner-only, named by another id) and Echo's relay. `echoctl proof hello <value> --mark` (a terminal session's
  hook) marks any value it is given: a process that knows a run's value can mark that run.
- A skill's load-time `!` commands: the guard lets the Skill tool load only the run's own skills; Echo's skills carry
  none, and a plugin's skill with one is refused (`plugins::check_skill_md`, `grants::set_session_skills`).

## Minutes run flags

`notes::args`: `--tools` `RUN_TOOLS` (`Bash,Read,Glob,Grep,Edit,Write,Skill`), `--strict-mcp-config`,
`--permission-mode dontAsk`, never `--add-dir`. Write is governed by the `Edit` rule. Deny rules keep `my-notes.md` and
`actions.json` unedited in every call folder.

## A minutes run's own call

- The CLI inside a minutes run reaches only the run's call (`guard::own_call`).
- The run writes only `notes.md` and `actions.new*.json`; the action items are applied through `echoctl actions --apply`.
  It never writes `meta.json`, `actions.json`, `my-notes.md` or instruction files.
- A call folder is renamed at the run's end (`calls::rename_to_title`); `calls::previous_subject` finds previous minutes
  without a `title_from_run` title.
- Outside its call it changes only learned vocabulary for its meeting and moves pruned frames to the trash; the app clears
  trash entries older than 30 days (`calls::purge_trash`).

## Yolo scope

- `echoctl read echo://call/…` gives a call of the run's thread pinned to the profile the run started on
  (`guard::RUN_PROFILE`; without it, no call) (`links::thread_call`).
- The CLI gives the thread's brief, people and work read-only (`guard::run_thread`).

## Rendering and content security

- Mermaid on a minutes page: `securityLevel: 'strict'` (`minutes::MERMAID_INIT`).
- The minutes render in an `<iframe sandbox="allow-scripts">` (`minutes::render`).
- The window is served with `window::window_csp`: `default-src 'self' echo: http://echo.localhost`, `img-src` the same
  and `data:`, `style-src` the same and `'unsafe-inline'`, `script-src` the same; `connect-src` and `media-src` the
  same plus the addresses the user listed for plugins that are on (`plugins::window_sources`: each listed address as its
  scheme and the matching ws/wss with its port; an IPv4 pattern with a `*` number left out). The builder lets in only a
  plain source expression (`plugins::addresses::csp_source_ok`: `http|https|ws|wss://[*.]host:port|*`, lowercase, no
  quote, `;`, space, keyword or bare `*`), whatever reaches it. Only the page (`/`) carries that list; every other
  answer the list-less policy. The page knows its list by a key (`<meta name="echo-net">`) and loads again when
  `/api/state`'s `net` differs (`netReload`, window script).
- The minutes page with its own hashed CSP through `window::page_reply`.
- The served window (`window --serve`) sends a page's policy in its header too.

## Served window

`echoctl window --serve` answers only on 127.0.0.1; its `POST /ipc` and any other POST — a plugin's call
(`/api/plugin/<id>/call/<command>`, `/api/plugin/<id>/markdown`) — (with `ECHOCTL_SERVE_IPC=1`) only with that run's
SameSite=Strict cookie and its own Origin (`window::serve_conn`); a plugin's call over 4 MB is refused (413) before it
is read in.

## Secrets

`ai.json` and `plugins/secrets/<id>.json` are written 0600; a profile's values reach Claude Code as `ECHOCTL_P_<name>`
variables (`ai::PASS`).

## Updates

`update::commit`: on Windows checked against the release's digest, on macOS against Echo's team signature.

## The view tag

`<echo_view>` goes only to terminals marked `ECHOCTL_TERMINAL` (`view::SESSION_VAR`).

## One builder for the folder session, routines and Yolo

- `folder_args::session` builds the context and permissions of the folder session (`terminal::start_folder`), a routine
  (`ctx.claude.run`, `claudejob::run`) and a Yolo run. A routine and Yolo add only `-p`, the prompt and the model, and
  start in the data folder.
- The user's choice "Claude acts on its own": `Config::folder_skip_permissions`; `notes::run_acts` tells a run acting on
  its own.
- A run with `tools: "none"` (Won't do, a plugin's analysis of what it read) keeps no tools.

## Preparing and asking (the choice off)

- Free without a prompt: `folder_args::FREE_COMMANDS`, each a fixed command and its own arguments — `context`,
  `attention`, `prep`, `previous`, `status`, `calendar events`, `read`, `project list`, `project show`, `project of`,
  `project tasks`, `project people`, `client list`, `org list`, `project work`, `compose`.
- A Microsoft 365 send is refused, pointing to a draft (`compose --body`) (`m365::not_in_run`).
- A plugin command's host half is read-only and only `readonly` scripts run (`guard::RUN_MODE=routine`:
  `jshost::read_only_here`, `plugins::scripts::run`).
- A thread's work refuses `delete`, `merge`, `xref`, `unxref`, `undo`, `accept`, `reject`, `state: cancelled` and a
  task's `mode`, `prompt` or `skill` (`cli::project::refused_when_asking`).

## Run actions and markers

- Every change and send a routine's or Yolo's run makes with Echo's CLI or a plugin is noted (`notes::note_action`,
  `notes::note`) in its `actions.jsonl`, and given in its outcome: its log, `ctx.claude.run`'s `actions`, Yolo's changes
  (`notes::run_actions`).
- Its edits of a thread's work are recorded `by: "run"` through `ledger::human_event`, applied as the user's would be
  (`ledger::by_hand`); a task's `mode`, `prompt` or `skill` a run set is shown as the run's (`Node::by_run`,
  `taskmodes::of`) until the user sets it.
- A Claude run started inside one drops its markers (`ai::PARENT_SESSION`) unless `claudejob` sets them; a plugin script
  inside one inherits them (`plugins::scripts::run_markers`).

## Exemplars

| When you are | Exemplar | Tests |
|---|---|---|
| changing what a headless run may do | `guard::own_call` | `guard::the_cli_works_only_on_the_runs_call` |
