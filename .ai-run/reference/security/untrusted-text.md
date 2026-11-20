# Untrusted Text — reference

How the guards, allowlists, safe writers and path checks are built: their lists, flags, variables and functions. It
changes with the code, in the same PR. The rules are in `.ai-run/guides/security/untrusted-text.md`.

## Run allowlists

The CLI commands each guarded mode may call (the guard refuses any other Bash call):

| Mode | Allowlist | Commands (on main) |
|---|---|---|
| minutes run | `notes::RUN_COMMANDS` | `context`, `actions`, `speakers`, `title`, `type`, `vocab add`, `frames`, `transcribe`, `memo threads` |
| project sync | `guard::PROJECT_STEPS` | `apply`, `done`, `people-apply`; the sync's own runs have no tools (Echo applies their answers), so the list holds for a run started with the project mode's variables |
| the window's session on a call (ask) | `guard::ASK_COMMANDS` | `context` |

A routine's or Yolo's run has no run guard: it runs as the folder session (One builder, below).

## Refusing inside a run

- `notes::run_limited()`: true in every run Echo started but a routine acting on its own. A CLI command that changes
  settings, recording, audio retention or anything outside the run's call checks it, as `memo start` does.
- `notes::headless()`: true in every run. Checked by the always-on vocabulary and keeping a call's audio
  (`notes::refused_in_run`) and by every `echoctl plugin` command that changes plugins (`cli::plugin::refuse_in_run`):
  install, update, remove, enable, disable, `set` with a value, `marketplace add / remove / hide / show`; and by
  `echoctl config --folder-permissions / --routine-permissions / --yolo-permissions` (`cli::app::config`); and by
  `echoctl jobs set` (`jobs::set`: Echo's jobs and the switch and tuning of a plugin's routine) and `echoctl jobs run`
  (`plugins::routines::run`: a plugin's routine runs only on its schedule or when the user runs it).
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

A folder at or inside a hidden one (any name in its real path starting with `.`: `~/.claude`, `vault/.claude/skills`,
`.git`) is never granted (`grants::grant`): there nothing is hidden relative to the folder given. On purpose this takes
in every such folder, e.g. `<repo>/.claude/worktrees/x` or `~/.dotfiles`, not a bug. One granted by an older Echo is
still read but never written (`grants::writable`, `writable_folder`).

A write into a plugin's granted folder (`grants::writable`, `writable_folder`) also refuses hidden names, the path as
given and as it really is (a link inside the root, a Windows short name), but for the exact names `grants::hidden_ok`
takes: `.gitignore`, `.obsidian/<name>.json` (in any folder that isn't hidden, an ASCII name), `.claude/skills/<name>/<file>.md` (folders: `.obsidian`, `.claude`,
`.claude/skills`, `.claude/skills/<name>`) — never Claude Code's settings, hooks, agents or commands, `.git`,
`.mcp.json` or Obsidian's plugins, themes and the lists that switch its plugins on (`community-plugins.json`, `core-plugins.json`, `core-plugins-migration.json`, in any case) (plugin-api.md § Folders and files).

A skill is code Claude Code runs as well as text: its frontmatter can hold hooks, pre-approved tools, a shell or a model,
and `` !`cmd` `` or a code block opened with `!` runs when it loads. So whatever a plugin puts in a skill — in a granted
folder's `.claude/skills/**` (`grants::in_skills`, the path as given and as it really is, in any case) or in the user's
own Claude Code, a package skill (`plugins::check_skill_md`, then only `name` and a one-line `description`) and a
runtime skill's text (`grants::set_session_skills`) — passes `plugins::claude_skills::skill_md_ok`: frontmatter (also after a BOM or blank space) only
`name`, `description`, `when_to_use`, `argument-hint`, `disable-model-invocation`, `user-invocable`, plain values, closed;
no `` !` ``, no `!` code block, no `!$`, no CR alone; refused, never stripped. Its frontmatter ends at the next line
`---` alone (`claude_skills::frontmatter`, the one split for both checks). There it is only written whole (`ctx.fs.write`): `append`,
`copyIn` and a `move` into it are refused, and `ctx.git.commit` refuses a file there that fails the check.

A plugin's skill in the user's own Claude Code (`plugins::claude_skills`) is written only on the user's click, as
`<Claude Code folder>/skills/<name>/SKILL.md` alone (`<name>` 1–64 of `a-z`, `0-9`, `-`; 256 KB at most; `skill_md_ok`;
never through a link), never over a skill folder the plugin didn't add (Echo's record, `<config dir>/plugins/claude-skills.json`);
on test data only under the test's own `CLAUDE_CONFIG_DIR`.

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
(`/api/plugin/<id>/call/<command>`, `/api/plugin/<id>/markdown`, `/api/plugin/<id>/reveal`, `/api/plugin/<id>/install-skill`, `/api/plugin/<id>/remove-skill`) — (with `ECHOCTL_SERVE_IPC=1`) only with that run's
SameSite=Strict cookie and its own Origin (`window::serve_conn`); a plugin's call over 4 MB is refused (413) before it
is read in. A body it doesn't take (over 4 MB, or sent with another method) is read and dropped, up to 64 MB, before
the answer, so the client gets the answer rather than a reset connection.

## Secrets

`ai.json`, `plugins/secrets/<id>.json` and `telemetry-export.json` (with the files `telemetry::export` hands curl) are written 0600; a profile's values reach Claude Code as `ECHOCTL_P_<name>`
variables (`ai::PASS`).

## Updates

`update::commit`: on Windows checked against the release's digest, on macOS against Echo's team signature.

## The view tag

`<echo_view>` goes only to terminals marked `ECHOCTL_TERMINAL` (`view::SESSION_VAR`).

## One builder for the folder session, routines and Yolo

- `folder_args::session` builds the context and permissions of the folder session (`terminal::start_folder`), a routine
  (`ctx.claude.run`, a plugin's own routine `plugins::routines::run`, both `claudejob::run`) and a Yolo run. A routine and Yolo add only `-p`, the prompt and the model, and
  start in the data folder.
- The user's permission settings (Settings → AI & minutes → Claude's permissions; `echoctl config --folder-permissions
  ask|auto|skip`, `--routine-permissions auto|skip`, `--yolo-permissions auto|skip`, refused in every run Echo started:
  `notes::headless`), `config::Permissions`, each Auto when unset:
  - `Config::folder_permissions` — the folder session, and a plugin's session in a folder it was given
    (`terminal::start_plugin`, `ctx.claude.session`): `ask` (Claude Code's prompts, no flag), `auto` or `skip`
    (`--dangerously-skip-permissions`) (`folder_args::folder_permissions`). Each setting becomes flags only through
    `folder_args::permission_args`: `auto` is `--permission-mode auto` where the profile's model has auto mode, else
    `acceptEdits` (`dontAsk` read-only) (claude-code-runs reference, Starting Claude Code).
  - A routine and Yolo always act on their own, whatever the folder session's setting. A routine's own choice
    (`claudejob::Tools`, `ctx.claude.run`'s `tools`: `auto`, or `skip` when its plugin's sheet has `claude.skip`
    (`plugins::may_run_claude`); `read` and `folder` of before are `auto`) comes first; else
    `folder_args::run_permissions(yolo)`: `Config::routine_permissions` (routines, plugins' runs, Echo's own jobs as
    routines) or `Config::yolo_permissions` (`auto` or `skip`; an `ask` written by hand reads as `auto`).
    `claudejob::Spec::yolo` says which.
  - Its record (`permissions.json`: `asked` — `auto`, `skip`, `setting` or `none`; `used`) and its log's first line name
    the mode: `acts on its own (auto mode: Claude Code decides each action)`, `acts on its own (edits accepted: this
    profile's model has no auto mode)` or `acts on its own (every permission check skipped)`; `mode`, when Claude Code started in another permission mode
    than asked, says which. A record of before may
    say `prepares and asks (…)` (Yolo runs shows it as "Prepared and asked").
  - The switch before (`folder_skip_permissions`) is read once by `Config::load`: on becomes `folder_permissions: skip`,
    off stays unset (Auto); it is never written again.
- A plugin's run or session in a folder (`claudejob::in_folder`, `in_folder_session`; plugin-api reference, Claude
  runs) uses the same builder with a place (`folder_args::Place`): no folder plugins offer sessions; in `read` only
  Echo's reading commands and `folder_args::read_only_args` (`dontAsk`) whatever the setting; else the plugin's `tools`
  (`skip` only with `claude.skip`), else the routines' setting (a session: the folder session's).
- `notes::run_acts` tells a routine's or Yolo's run (`ECHOCTL_RUN_SESSION=acts`), whatever its mode.
- A run with `tools: "none"` (Won't do, a plugin's analysis of what it read) keeps no tools.

## Allowed by name

- Every session in the data folder gets Echo's reads and its own data's edits allowed by name (`--allowedTools`):
  `folder_args::FREE_COMMANDS`, each a fixed command and its own arguments — `context`, `attention`, `prep`,
  `previous`, `status`, `calendar events`, `read`, `project list`, `project show`, `project of`, `project tasks`,
  `project people`, `client list`, `org list`, `project work`, `compose` — and every plugin's declared scripts and
  session commands. What else a run does is its permission mode's to decide.
- Preparing and asking (a run with the user's one switch off, a read-only routine) is gone: no run sets
  `RUN_MODE=routine` or `ECHOCTL_RUN_SESSION=asks`, and the guard, the CLI and the plugin host have no limits for it.

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
