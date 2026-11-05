# AI profiles and minutes — design

Which model and account each AI job uses (profiles, sign-in, leases, strict routing), the live-notes worker, and how
the app writes a call's minutes headless with Claude Code under a guard — plus the minutes page, the About me profile
and the app's self-update.

Related:
- Rules: `.ai-run/guides/integrations/claude-code-runs.md` (starting Claude Code, routing, leases, the fake) and
  `.ai-run/guides/security/untrusted-text.md` (the guard, run allowlists, safe Markdown, path checks).
- Plan, profile kinds in detail, live-notes mechanics and measurements: `docs/live-notes-design.md`.

## Modules

| File | Role |
|---|---|
| `crates/echoctl/src/ai.rs` | Profiles (`ai.json`), `for_call` / `ready`, `claude_command`, routing env, policy and hooks checks, leases, `ai::fake` |
| `crates/echoctl/src/signin.rs` | Sign-in flow run by the app (`claude auth login`), `SignInState` for window and tray |
| `crates/echoctl/src/livenotes.rs` | The live worker: summaries, asks, lean short jobs (`complete_claude_with`), usage |
| `crates/echoctl/src/notes.rs` | Minutes runs: automatic scheduler, admission, the run, its state and outcome |
| `crates/echoctl/src/guard.rs` | `echoctl run-guard`: the `PreToolUse` hook that limits a headless run |
| `crates/echoctl/src/calls.rs` | Call files used by runs: `meta.json` locking, actions, gate, recap, `me.md` |
| `crates/echoctl/src/minutes.rs` | `notes.md` → HTML (window frame and the opt-in page) |
| `crates/echoctl/src/update.rs` | Release check, download, verify, stage, swap |

## How it works

### Profiles

- Stored per computer in `<config dir>/ai.json`: `default`, `live_notes` (off until turned on), `profiles`. Loading
  refuses invalid or duplicate ids; saving is atomic and 0600 from the start. Every change goes through
  `ai::Settings::update` (lock + re-read, so two writers never undo each other; a new default must name a profile).
- Kinds (detail in `docs/live-notes-design.md` → Model profiles):
  - `system` — built in, never stored: Claude Code as the user set it up (login shell, all settings,
    `config.notes_model` / `config.live_model`). No `ai.json` = only `system`, and live notes are off.
  - `local` — a Messages-API server at `base_url` (Ollama). Nothing checks it is local; the window names the host
    when it isn't loopback.
  - `claude` — a claude.ai account in its own Claude Code folder `<config dir>/claude/<id>`. The first account seen
    (email · organisation) is recorded; another account later stops jobs until accepted (Use X / `ai accept`).
- Minutes model: `Profile::notes_model` is the one answer for the run, menus, window and the minutes record
  (`system` = `config.notes_model`, default sonnet; `claude` its own, default sonnet; `local` its own, else its live
  model).

### Pinning a call to a profile

1. At recording start `ai::pin` picks the explicit choice (`arm --ai`, Record menu, a scheduled / joined meeting's
   `MeetingContext.ai_profile`) else the default; `!unreadable` when `ai.json` can't be read. The watcher and
   `capture` share this rule. It is written to `meta.json` `ai_profile`.
2. Recording never depends on AI: `arm` doesn't check the profile (only a typed `arm --ai` is checked by the CLI).
3. Mid-call change → `ai-profile.request` → the watcher (it owns `meta.json` until `finish`) →
   `livenotes::profile_changed`. After the call `ai::set_call_profile` writes it. A request names the call; one
   retitled since is found by its stamp (`calls::by_stamp`).
4. Every `meta.json` write takes the call's `.meta.lock` (`calls::update_meta` / `lock_meta`) and is atomic
   (`paths::write_atomic`, a temporary file of its own per write).

### Routing (mechanism; rules in the claude-code-runs guide)

- `ai::for_call` resolves the pinned profile; a missing profile, unreadable `ai.json` / `meta.json`, signed-out or
  other account → the job fails with the reason. Never another profile.
- `ai::claude_command` is how Echo runs Claude Code on call text, a sign-in check or a sign-in:
  - `system`: the user's command as is.
  - `local` / `claude`: after the login shell's startup files, every routing variable family is unset
    (`ai::routing_var` for the process env, `UNSET_ROUTING` as the shell's `sed` filter; on Windows any case), the
    profile's values exported from `ECHOCTL_P_<name>` (never in the script text), `NO_PROXY` for a loopback `local`,
    and `--setting-sources user` (not for `claude auth …`, which rejects it). A project `.claude/settings.json` `env`
    beats the process environment (measured) — hence the flag.
  - Children get their own session (`ai::detach`, `setsid`): a bare process group stopped an interactive login shell
    started from a terminal on SIGTTOU. `echoctl ai login`'s child is the exception: it stays in the terminal's
    foreground (paste-code fallback, Ctrl-C).
- Admin policy that could route (managed settings + `.d/`, macOS managed preferences, any Windows policy key, the
  profile's cached `remote-settings.json`: a routing `env`, `apiKeyHelper`, `policyHelper(s)`, an unreadable source)
  → `local` / `claude` Claude Code runs are refused (`ai::policy_conflict`). `local` live jobs use curl, not Claude
  Code, so they aren't affected.

### Sign-in and leases

1. Trigger: window / tray Sign in…, `echoctl ai login <id>`, or choosing a signed-out `claude` profile in a window
   menu (Live header, Record, Write minutes). For a menu choice only an explicit `loggedIn: false` opens the browser,
   and only if the choice is still the latest for its purpose both when the check ends and right before the browser
   opens.
2. The app runs `claude auth login --claudeai [--email] [--sso]` through `claude_command`, no terminal. The browser
   calls Claude Code back on `http://localhost:<port>/callback`; the child exits. 10 min at most; Cancel; quit stops it.
3. One `auth status` judged against the settings as they are now (`ai::settle`) → "Signed in as …";
   `livenotes::signed_in` resumes live jobs that waited.
- Lease: a reader/writer lock per profile, `<config dir>/claude/<id>.lock` (`app::FileLock`). Jobs and status checks
  hold it shared (`ai::lease`; the `Ready` from `for_call` keeps it for the job); a sign-in holds it exclusively
  (`ai::sign_in_lease`).
- During a sign-in `for_call` fails with `ai::SigningIn` and never waits: live jobs are held back by
  `livenotes::defer` (so they can't grab the lock first), a minutes run waits up to 12 min.
- The window's Claude sessions on the profile hold its lease; a sign-in closes them first
  (`terminal::close_profile`, they resume by id). Still held after 10 s → the error names the session that didn't stop
  (`terminal::holds_profile`).
- The lock file's text is a token rewritten by each sign-in; each process's readiness cache is keyed by it.
- Window and tray state (`signin::SignInState`, in `ai::summary`) never runs a process. Background checks: app start,
  window open, and whenever nothing current is known (none yet, changed token, a model said signed out), at most every
  30 s. Errors are typed (`ai::SignInNeeded`) → `needs_sign_in` / run `needs_login` with the profile → Sign in… / Use X.

### Live notes worker (`livenotes.rs`)

Mechanics (cadence, cursor, prompts, asks, cancellation, the local-model gate) are in `docs/live-notes-design.md` →
How it works. What this area adds:
- Calls are keyed by folder stamp (`calls::stamp_of` / `by_stamp`); two calls in the same second share one, and
  `by_stamp` then refuses to guess — the worker keeps the folder it last saw.
- The JSON result must carry at least one of `now`, `decisions`, `actions`, `questions`; otherwise it is refused
  (saving it would wipe the draft).
- Every short job (live notes, asks, the Brain's readers) is lean: `--settings <config dir>/lean-job-settings.json`
  (`claudeMdExcludes: ["**"]`) and `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`. The user's prompt text goes on stdin from a
  thread of its own, so the deadline and stopping cover it.
- `system` live jobs run in `notes_in`, else the data folder, strictly (`ai::system_folder`: an unreadable
  `config.json` or a `notes_in` that isn't a folder fails the job).
- Stopping: `ai::stop_child` / `stop_group` — TERM to the group, KILL to what's left after 3 s; the leader exiting
  first doesn't end the watch, and output still held by a leftover gets 2 s. Turning live notes off stops the running
  job and drops queued questions with that reason.
- Usage from Claude Code's result envelope is summed into `<call>/live-usage.json` (`livenotes::Usage`) and shown in the
  Transcript tab. After the call the live notes are appended to `transcript.md` as a `## Live notes` draft
  (`markdown_section`, `refresh_transcript`).
- Live notes and asks get `livenotes::call_header` (who I am / people / terms from About me, ≤ 1500 chars).

### Minutes waiting for the local model

`local` and `system` jobs share `<config dir>/local-model.lock` (live jobs, the app's minutes, `notes --wait`).
Minutes on them wait while any call records or is transcribed; the run's message names what it waits for
(`notes-run.json` `wait: own | call | model`; its own call is checked first and never skipped). On Claude Code as
configured — not routed by its folder's settings to a local model — the window offers **Write now** while it waits for
another call (`notes-now.request`, read by the waiting run, used up when it starts; never for a local model or its own
call). The Inbox lists each run waiting for something it names (`minutes-wait:<stamp>`).

### Automatic minutes (`notes::auto_due`, `auto_tick` every 15 s)

A call is due when all hold:
- `config.auto_minutes` on and the call ended after `config.auto_minutes_since` (set at first start with the switch on
  and at each off → on, so no backlog is sent);
- closed, transcribed, not being transcribed, `my-notes.md` readable;
- never tried: no `notes.md`, no `notes-run.json` (except an automatic run whose owner died before writing — still
  `queued`, or `running` with its Claude Code gone), no session's fresh claim;
- past `calls::minutes_gate` (shared with `context`);
- now ≥ max(end, `my-notes.md` mtime) + 15 min, unless held by **Not yet** (`notes::set_later` →
  `minutes-later.json`: `{until}` or `{manual: true}`).

Flow:
1. `admit(dir, Intent::Auto)` re-checks `auto_due` under the call's place and `.meta.lock`.
2. Right before Claude Code starts, `auto_due` once more (its own record exempt).
3. Not due any more, the app quitting while it waits, or Claude Code stopped by quit before writing → **withdrawn**
   (record and claim removed; due again later).
4. An admission that fails is logged once per reason and recorded as the call's failure after 4 ticks in a row, with a
   notification (missing Claude Code / plugin: at once). One that can't even be recorded is left until Echo restarts
   (`notes::GAVE_UP`).

The window gets `auto = {due: now|at|off|before|not_ready|tried|held|gate|missing|error, at, moved, reason, held_by}`;
`missing` (`notes::auto_missing`) means no "minutes at" is promised. The Minutes tab draws the countdown ring with
Write now and Not yet ▾; the calls list shows ⏱ for calls ended in the last 2 hours (`minutes_at` in `/api/calls`).

### Write minutes (explicit) and the run

- `Intent::Explicit` (window, `echoctl notes <call>`): its prompt overrides the skill's short-call gate, and it takes
  over a session's fresh claim on a call with no minutes. The CLI hands the run to the app (`notes-<call>.request`)
  unless `--wait` (Ctrl-C stops its Claude Code too).
- The run: `/echo:process-call` headless, `claude -p … --output-format stream-json` via `claude_command` on the
  call's profile; cwd = `notes_in` or the data folder (`system`: `ai::system_folder`); `--plugin-dir` = the plugin
  shipped in the app (dev: repo `plugin/`; else the installed one); model = the profile's minutes model.
- A run whose `init` event lacks `claude_code_version` ≥ `guard::MIN_CLAUDE` is stopped before the model's first turn.
- A local notes model on `system`: the cwd's `.claude/settings.json` `env` points Claude Code at Ollama and
  `notes_model` names a model with `num_ctx` ≥ 64K (Claude Code's prompt + an hour's transcript). Claude Code's
  `unrecognized_model` warning counts only when the run failed (`model_rejected`).

### The guard (mechanism; rules in the untrusted-text guide)

- `echoctl run-guard`: a `PreToolUse` command hook with `onFailure: block`, in the run's own settings file
  (`<config dir>/minutes-run/<run>.json`, removed after; `disableAllHooks: false`, `claudeMdExcludes` for `calls/**`).
- Allows only: Bash = `<cli> <notes::RUN_COMMANDS> …` as plain words (no `$`, backtick, newline, or
  `;&|<>(){}*?[]~#!` outside quotes); Read in the run's call folder (canonical) and the previous minutes of the same
  meeting; Glob/Grep with a path in the call folder (no absolute, `~`, `..` or brace patterns); Edit/Write of only
  `notes.md` and `actions.new*.json` at the folder's top, never through a link; Skill `echo:process-call`. Anything
  else, or anything it can't judge (a panic too), exits 2.
- Scope only from the environment: `ECHOCTL_RUN_CALL` (a folder with `meta.json`), `ECHOCTL_RUN_PREVIOUS`,
  `ECHOCTL_RUN_CLIS` (required).
- Second layer: `--tools Bash,Read,Glob,Grep,Edit,Write,Skill`, `--strict-mcp-config`, `--permission-mode dontAsk`,
  allow rules for the call folder, the previous minutes and `Bash(<cli> <cmd>:*)`, no `--add-dir`, deny `Edit` of any
  call's `my-notes.md` / `actions.json`.
- Admin policy that switches settings hooks off (`disableAllHooks`, `allowManagedHooksOnly`, a `policyHelper(s)`) in
  any source — for `system` in every config folder its Claude Code may use (the login shell's `CLAUDE_CONFIG_DIR`,
  asked with a 15 s limit, Echo's own, `~/.claude`) — refuses the run (`ai::hooks_conflict` returns the folders it
  checked, for the log).
- Inside a run (`ECHOCTL_HEADLESS=1`) the CLI works only on the run's call (`calls::resolve_call` →
  `guard::own_call`, canonical path); `notes::refused_in_run` refuses `vocab add --always`, `transcribe --keep-audio` /
  `--model`; the mod does nothing and `pending` / `claim` refuse.
- `title` inside a run sets the title, marks `title_from_run`, and leaves the folder rename to the run's end
  (`calls::rename_to_title`); a terminal `echoctl title` on a call with a run waits too (`notes::run_active`).
- By design a run still changes two things outside its call: learned vocabulary for this meeting and pruned frames
  moved to the trash (purged by the app after the run, `calls::purge_trash`, not by `frames --keep`).

### Action items

- Ids are stable; `calls::actions` gives ids once to an older list before any reader or run sees it.
- The skill writes a new list and runs `echoctl actions <call> --apply <file>` (`calls::apply_actions`): only
  `actions.new*.json` in the call's folder; keeps ids and each known item's current status (ticks made meanwhile
  survive); records the writer in `.actions-generation` from `ECHOCTL_RUN_ID`.
- An app run's prompt names its own file `actions.new-<run>.json`; if the model left it unapplied the app applies it as
  that run's (`notes::finish_actions`) — never the generic `actions.new.json`, which could be a session's.
- A current list that isn't a JSON list is set aside as `actions.unreadable-<HHMMSS>.json` on admission or apply.
- Ticks: `calls::set_action_status(id, status)`.

### One run per call, claims, outcome

1. `notes::admit`: the call's place first — an OS lock on `notes-run.lock` held by the owner for the whole run (freed
   by the OS if it dies, moves with a renamed folder, dropped before the final rename for Windows) — then the claim,
   then `queued`. A refusal changes nothing.
2. Claims are live: `echoctl claim <call> --session <id>` records the session and its Claude Code process; it holds
   only while that process lives and works on it (claim < 15 min old, or minutes / actions / log changed since). A
   claim without `--session` is refused. A session's claim fails only while Echo's run is on or another session's fresh
   claim holds a call without minutes; a finished run's claim is taken over.
3. States in `notes-run.json`: `queued → running → done | skipped | failed | needs_login` (`notes::conclude`; a `local`
   profile's refused token is `failed` with how to set it). `pid` = owner, `child_pid` = Claude Code; a dead owner =
   interrupted unless its Claude Code still runs.
4. `done` only with `notes.md` written during the run and this run's `.actions-generation`; then
   `minutes: {model, at, sha256}` names the model on the page while `notes.md` keeps that hash.
5. The run finishes in the folder as named after Claude Code (a rename made outside meanwhile is followed by stamp).
   Quit stops minutes runs and starts no more (`notes::shutdown`).
6. Notifications: "Transcript ready — minutes at HH:MM…" after transcription; "Minutes ready" once per run.
- Older Echo plugin installs (`app::stale_plugins`) appear in Health and Needs you with Update it
  (`app::update_plugins`).
- `calls::recap_from_notes` reads minutes in other shapes (frontmatter skipped, Decision table cells, placeholders
  like `TBD` dropped); `calls::recap` gives the deterministic "last time" bullets for the Record dialog.

### Minutes page (`minutes.rs`)

- The window's Minutes tab always renders fresh (`minutes::render_for_window`, `with_theme`) whatever the setting.
- The file is opt-in (`config.minutes_page`): `notes.md` → `<call>/<title>-minutes.html`, self-contained — header block
  from `> **Label:** value` lines and frontmatter, tables, `[mm:ss]` timestamps, frames embedded only where cited, each
  "On screen" line given its nearest kept frame within 30 s (`attach_screens`), `ECHO_CSS` light and dark (the
  window's frame adds `EMBED_CSS`), mermaid inlined only when drawn (`securityLevel: 'strict'`).
- Written after `done` by an app run, by `echoctl html <call> --auto` (only when on) from a session, or any time by
  `echoctl html <call> [--open]`. Our own writer escapes everything (`minutes::markdown_html` rules in the guide).
- A renamed call's old page (found by its generator meta) is replaced, never another file.

### About me (`me.md`)

`calls::me_profile` → `context.me_profile`. `calls::me_parts` reads it in blocks by heading or "Label:" line —
identity, people, terms, patterns ("How I work"), style — and each consumer takes only what it needs:

| Consumer | Gets |
|---|---|
| Thread sync (`project ledger <id> context`, `people-context`, `verify-prep` → `me_patterns`) | all; patterns as defaults, the words win |
| Claude in the window (`SessionStart` hook, `<about_me>`; on resume only when changed) | all |
| Live notes and asks (`livenotes::call_header`) | identity, people, terms, ≤ 1500 chars |
| The Brain's quarantine reader | people and terms only, for spelling; its cache key carries their hash |

### Updates (`update.rs`)

1. `update::check` at start and every 6 h (public GitHub API, else `gh`).
2. `update::prepare` downloads, verifies the asset's `digest` when the API gives one, and stages (macOS: mounts the
   dmg, requires our team id signature).
3. `update::commit` schedules the swap after exit — only once the window's notes are saved and Echo is stopping
   (`app update` too, after `app quit`), and from the tray never while a call records (checked again in `commit`).
- Windows: the release's MSI, offered only to an Echo in `%LOCALAPPDATA%\Programs\echo`, the digest required (builds
  are unsigned), run by `msiexec /qn` (per-user; the MSI quits Echo, replaces files, starts it).

## Decisions

| Decision | Why | Where |
|---|---|---|
| Strict routing: a job fails rather than use another profile | call text must reach only the model the user chose for that call | `ai::for_call`, `ai::call_profile_id` |
| Profile pinned in `meta.json` at recording start | changing the default later never re-routes an existing call | `ai::pin` |
| Recording never checks the AI profile | AI problems must not cost a recording | `arm` (CLI), `ai::pin` |
| `--setting-sources user` for `local` / `claude` | a project `env` beats the process env and would re-route | `ai::claude_command` |
| Routing env removed by family, in two matchers kept in step | Claude Code reads many routing vars; shell and process env both need it | `ai::routing_var`, `UNSET_ROUTING` |
| Refuse `local` / `claude` runs under routing admin policy | managed `env` / helpers override Echo's routing | `ai::policy_conflict` |
| Children in their own session, not just a process group | a process group stopped an interactive login shell on SIGTTOU | `ai::detach` |
| Sign-in exclusive, jobs shared, on one lock per profile | a sign-in must not race a job using the same folder | `ai::lease`, `ai::sign_in_lease` |
| `for_call` never waits on a sign-in; live jobs deferred | deferred jobs can't grab the lock before the sign-in | `ai::SigningIn`, `livenotes::defer` |
| Lean short jobs (no CLAUDE.md, no auto memory) | in the data folder each 90 s update was ~46K input tokens instead of ~2.5K | `livenotes::complete_claude_with` |
| Live result with none of the four fields is refused | it would wipe the draft | `livenotes.rs` |
| `local` live jobs by curl with thinking disabled | Claude Code can't turn Qwen's thinking off: ~50 s vs ~7 s an update | `livenotes.rs` |
| Guard hook with `onFailure: block` | without it a guard that can't start lets the tool call through | `guard.rs`, run settings |
| Guard scope only from env, never tool input | the model controls tool input | `guard::RUN_CALL` |
| Guard refuses writes to `meta.json`, `my-notes.md`, instruction files | `meta.json` would re-pin the profile; `@imports` load files with no tool call | `guard::own_call` |
| No `--add-dir`; Write governed by the `Edit` rule | allow rules reach outside cwd; a path-scoped `Write(…)` rule never matched | `notes.rs` |
| Run refused when policy can switch hooks off | the guard would not run | `ai::hooks_conflict` |
| Folder rename only at the run's end; `title_from_run` ignored for previous minutes | a run could pick which other call's minutes a later run reads; Windows can't rename an open folder | `calls::rename_to_title`, `calls::previous_subject` |
| App applies only its own `actions.new-<run>.json` | weaker models stop one step short; the generic file may be a session's | `notes::finish_actions` |
| Apply keeps each item's current status | ticks made during the run survive | `calls::apply_actions` |
| Explicit Write minutes overrides a session's fresh claim; automatic yields | an old plugin's claim blocked Write minutes for 30 min with nobody writing | `notes::admit` |
| Claims tied to a live session process | a bare-time claim outlived its session | `echoctl claim --session` |
| `auto_minutes_since` reset at each off → on | turning automatic minutes on must not send a backlog | `config.rs`, `notes::auto_due` |
| Automatic run re-checks `auto_due` right before start; withdrawn if not due | the user may have written minutes or edited notes while it waited | `notes::admit`, `Intent::Auto` |
| Windows updates require the release digest | builds are unsigned | `update::verify_digest` |
| No swap while recording or before notes are saved | never lose a call or the user's notes | `update::commit` |

## Files and state

| Path | What | Who writes |
|---|---|---|
| `<config dir>/ai.json` | profiles, default, `live_notes` (0600) | `ai::Settings::update` |
| `<config dir>/claude/<id>/` | a `claude` / `local` profile's Claude Code folder | Claude Code |
| `<config dir>/claude/<id>.lock` | per-profile lease; text = sign-in token | `ai::lease`, `ai::sign_in_lease` |
| `<config dir>/local-model.lock` | gate shared by `local` / `system` jobs | live worker, minutes runs |
| `<config dir>/lean-job-settings.json` | settings for short jobs | `livenotes.rs` |
| `<config dir>/minutes-run/<run>.json` | a run's settings with the guard hook; removed after | `notes.rs` |
| `<call>/meta.json` `ai_profile` | the call's pinned profile | watcher / `ai::set_call_profile` |
| `<call>/ai-profile.request` | mid-call profile change | window → watcher |
| `<call>/live-notes.json`, `live-asks.jsonl`, `live-usage.json` | live drafts, answers, usage | live worker |
| `<call>/notes-run.json`, `notes-run.lock` | run state and the one-run lock | `notes.rs` |
| `<call>/.claimed` | session / Echo claim | `echoctl claim`, `notes.rs` |
| `<call>/minutes-later.json` | Not yet hold | `notes::set_later` |
| `<call>/notes-now.request` | Write now for a waiting run | window |
| `<call>/notes.md`, `actions.json`, `.actions-generation` | minutes, actions, last writer | run / `calls::apply_actions` |
| `<call>/claude.log` | readable run log | `notes.rs` |
| `<call>/<title>-minutes.html` | opt-in minutes page | `minutes.rs` |

## Known gaps

- Server-managed policy that Claude Code fetches fresh at startup is seen only from the next run.
- Not verified: jobs on two signed-in Claude folders side by side (separate Keychain items were checked), live jobs
  under real transcription load, live notes on Windows.
- Updates have no publisher check beyond the digest until builds are code-signed.
