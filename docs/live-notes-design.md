# Live notes in the Echo window — design

Status: v4, 2026-10-23, reviewed with a second model (OpenAI Codex) over four rounds; v2 added the
phase 0 measurements. Where the code differs from this text, see **As built** at the end.

## Goal

During a call, the Echo window shows what matters without a terminal: live notes that keep up with
the call, quick help on demand, and a reminder of what was left open last time. Which model writes
them is chosen per call, with a default, and always shown.

Today this exists only in the Claude Code terminal pane (the plugin's mod): a rolling summary every
90 s and two buttons, through `$.model.complete` with `config.live_model` on whatever provider and
account the open session uses (issue #5).

## Decisions so far

| Question | Decision |
|---|---|
| What v1 shows | Live notes, quick buttons + ask box, prep card from last time |
| Where | A **Live** tab in the Calls view (a floating mini window maybe later) |
| Language of the live notes | Always English, whatever the call's language |
| Model | A default **model profile**, overridable per call — at the start or mid-call |
| Scope of the per-call choice | The whole call: live notes and the minutes written after it |
| Not in v1 | "You were asked" alerts, live speaker names, floating window |

## Phase 0 results (measured on a Mac with 48 GB of memory, 2026-10-23)

Synthetic 10-minute transcript (60 lines, 4.2K chars), the live prompt below, Qwen 3.6 35B-A3B
(Q4_K_M) through Ollama, already loaded (cold load adds ~10 s):

| Path | Time | Output tokens |
|---|---|---|
| Claude Code `-p`, lean flags, Qwen through Ollama | 46–57 s | 2,400–3,600 (mostly thinking) |
| + `MAX_THINKING_TOKENS=0`, `CLAUDE_CODE_DISABLE_THINKING=1`, `alwaysThinkingEnabled: false`, `--safe-mode` | 52–54 s | 3,300–3,600 |
| Ollama's Anthropic API directly, `thinking: {"type": "disabled"}` | **6.9 s** | 411 |
| Ollama native API, `think: false` | 6.5 s | 261 |

- Claude Code never sends `thinking: disabled`; with thinking turned off it only omits the field, and
  Ollama then lets Qwen think (captured with a logging proxy). `/no_think` does nothing for Qwen 3.6.
  An Ollama variant with a non-thinking template failed: the GGUF's own Jinja template is used and
  rejects Claude Code's system messages. Ollama rejects `PARAMETER think`.
- The lean Claude Code run itself is cheap: `--system-prompt` + `--tools ""` → 1,838 input tokens;
  start-up ≈ 2 s on top of the API time.
- A project `.claude/settings.json` `env` **overrides** the environment Claude Code was started with
  (a bogus `ANTHROPIC_BASE_URL` in the project beat a working one in the process env; the run spent
  3 min retrying). `--setting-sources user` skips project settings.
- A fresh `CLAUDE_CONFIG_DIR` starts signed out (`claude auth status` → `loggedIn: false`): it does
  not see the default folder's claude.ai sign-in. Two folders signed in to two accounts side by
  side is not tested yet (needs two claude.ai accounts; see **As built**).
- Abandoning an Ollama request (client closes the socket) stops generation: a tiny request sent
  right after came back in 2.8 s instead of waiting ~45 s.
- 48 GB RAM: two 22 GB Qwen variants can't be resident together; using the same model for live
  (thinking off per request) and minutes (thinking on) avoids any swap.

## What the user sees

The Live tab is the default tab while a call records or is transcribed:

```
┌ Weekly delivery sync ─ ● Rec 23:41 ─ [Qwen · this Mac ▾] ─ updated 40 s ago ┐
│ Transcript                  │ ▸ Last time (15 Oct): 2 open actions, 3 highlights│
│ 23:02 Remote  …the L2 team  │ NOW  RFP resourcing plan                          │
│   can start in…             │ DECISIONS                                         │
│ 23:10 Me  Can we move the   │ • Go-live moves to 15 Jan                [21:40]  │
│   start to…                 │ ACTION ITEMS                                      │
│ 23:31 Remote  I'll send the │ • Remote — send the rate card            [23:31]  │
│   rate card…                │ OPEN QUESTIONS                                    │
│                             │ • Who signs off the SOW?                 [22:15]  │
│                             │ [What did I miss?] [Suggest a question]           │
│                             │ [ Ask about this call…                         ]  │
│                             │ ── You asked (23:35): what budget was mentioned?  │
│                             │    "€240k for phase 1" [18:02]                    │
└─────────────────────────────┴───────────────────────────────────────────────────┘
```

- **Header:** recording time; the call's profile (a menu: switch mid-call, from the next job on);
  the live-notes state: waiting for speech · updating · updated N s ago · paused · failed (the
  reason, e.g. "Claude · enterprise isn't signed in — Sign in…", "Qwen didn't answer in 120 s") ·
  waiting for another call's notes to finish · ended. When the shown draft came from another
  profile than the selected one: "draft by Qwen; next update by Claude · personal". A pause button
  stops further updates for this call.
- **Left:** the live transcript (the existing transcript view over `live.jsonl`).
- **Right, top to bottom:** the prep card (collapsible), Now, Decisions / Action items / Open
  questions (each bullet with a `[mm:ss]` jump), the buttons, the ask box, answers newest first.
- Narrow window: the right column goes under the transcript.
- After the call the Live tab stays (last draft + answers; the ask box keeps working). The default
  tab becomes Notes once the minutes run is `done` — never on the first appearance of `notes.md`.
  "Write notes" stays the explicit button it is today; it names the call's profile and offers the
  others for this run.

## Model profiles

A profile is a named way to reach a model. Stored per machine in `<config dir>/ai.json` (like
`graph.json`): sign-ins and local models belong to the computer, not the data folder.

```json
{ "default": "local-qwen", "live_notes": true,
  "profiles": [
    {"id": "local-qwen", "name": "Qwen · this Mac", "kind": "local",
     "base_url": "http://localhost:11434", "token": "ollama",
     "live_model": "qwen3.6:35b-a3b", "notes_model": "qwen3.6:35b-a3b"},
    {"id": "claude-personal", "name": "Claude · personal", "kind": "claude",
     "live_model": "haiku", "notes_model": "sonnet", "account": "you@example.com"},
    {"id": "claude-enterprise", "name": "Claude · enterprise", "kind": "claude",
     "live_model": "haiku", "notes_model": "sonnet", "account": null}
  ] }
```

| Kind | Live notes and asks | Minutes |
|---|---|---|
| `system` | Claude Code, lean run, `config.live_model` | today's run, unchanged (login shell, all setting sources, `config.notes_model`) |
| `local` | **Echo → the profile's Anthropic-compatible API directly** (`POST {base_url}/v1/messages`, `thinking: disabled`) | Claude Code with the profile's environment (thinking on) |
| `claude` | Claude Code, lean run, own config folder | Claude Code, own config folder |

- **`system`** is today's behaviour exactly and the default when `ai.json` doesn't exist, so an
  existing install changes nothing. Live notes are **off** unless `live_notes` is turned on (they
  would send call text to a model the user may never have chosen for that).
- **Why `local` live jobs skip Claude Code:** phase 0 — 7 s instead of ~50 s, because only a direct
  request can turn Qwen's thinking off. Live jobs are single completions without tools (what the
  mod's `$.model.complete` does today), not Claude sessions; minutes still always go through
  Claude Code and the skill. Sent with curl, as `graph.rs` does.
- **`claude` profiles** each get their own Claude Code config folder, `<config dir>/claude/<id>`,
  signed in once. Switching accounts with `/login` in the terminal changes nothing for Echo.
  `account` is recorded at the first successful check (`claude auth status`: email, organisation,
  subscription); if the folder later reports another account, AI jobs stop with "signed in as X,
  expected Y" until the user confirms in Settings.
- **Settings → AI for calls:** the default profile, live notes on/off, each profile with its account
  or **Sign in…** (opens a terminal with that config folder running `claude auth login`: macOS
  Terminal via `osascript`, Windows `cmd /k` — behind `#[cfg]`), add / remove a local model or a
  Claude account.
- CLI: `echoctl ai list | default <id> | add-local <id> --model … [--url …] | add-claude <id> |
  remove <id> | status [<id>] | login <id> | live on|off`; `echoctl arm --ai <id>`.
- The user's own Claude Code settings are never changed. The data folder's `.claude/settings.json`
  (pointing at Ollama, say) keeps working for interactive sessions there; Echo's runs for `local` and `claude`
  profiles don't load project settings (`--setting-sources user`).

### One resolver, one launcher (routing is authoritative)

- `ai::resolve(call)` → the profile pinned in the call's `meta.json`; for a new recording, the
  explicit choice or `default`, pinned at start. A missing profile, an unreadable `ai.json`, a
  `claude` profile not signed in or on the wrong account → **an error for AI jobs, never another
  profile**; recording and transcription continue. (`notes.rs`'s `Config::load().unwrap_or_default()`
  is not reused for routing.)
- `ai::claude_command(profile, args, cwd)` is the only way Echo starts Claude Code (minutes, live,
  auth status, login):
  - `system`: today's command (`notes.rs::command`) unchanged.
  - `local` / `claude`: on macOS/Linux still through the login shell (PATH, proxies), but the profile
    is applied **after** the shell's start-up files. Removal is by **family**, not a fixed list
    (Claude Code 2.1.309 reads dozens of these, e.g. `CLAUDE_CODE_USE_GATEWAY`, `_MANTLE`,
    `_ANTHROPIC_AWS`, `ANTHROPIC_UNIX_SOCKET`, identity tokens): every variable named `ANTHROPIC_*`,
    `CLAUDE_CODE_USE_*`, `CLAUDE_CODE_SKIP_*_AUTH`, `CLAUDE_CODE_OAUTH_*`, `CLAUDE_CODE_API_KEY_*` or
    `CLAUDE_CONFIG_DIR` is unset — the `-c` script lists the environment's names (`env | sed -n …`),
    unsets each match, then `export`s the profile's values (single-quoted by Echo) and
    `exec "$0" "$@"`. Windows: no shell; the same families removed from the `Command`'s
    environment (matched over the app's own environment) and the values set. One matcher
    (`ai::routing_var`) is used for both and unit-tested. Plus `--setting-sources user` (the
    profile's own, Echo-created config folder) and `--model`.
  - **Admin-managed policy:** Claude Code always applies it and its `env` beats the process
    environment. Before a `local` / `claude` job Echo inspects every device policy source Claude
    Code 2.1.309 reads:
    - `managed-settings.json` and `managed-settings.d/*.json` (macOS
      `/Library/Application Support/ClaudeCode/`, Linux `/etc/claude-code/`, Windows
      `C:\Program Files\ClaudeCode\`);
    - macOS managed preferences: `/Library/Managed Preferences/com.anthropic.claudecode.plist` and
      `/Library/Managed Preferences/<user>/com.anthropic.claudecode.plist` (read with
      `plutil -convert json -o -`);
    - Windows policy keys `HKLM\SOFTWARE\Policies\ClaudeCode` and `HKCU\SOFTWARE\Policies\ClaudeCode`.

    A routing variable in a policy's `env`, an `apiKeyHelper`, a `policyHelper` / `policyHelpers`
    (policy computed at run time: not inspectable), a policy source that exists but can't be read or parsed, or any
    Windows policy key (v1 doesn't parse the registry) → the job is **refused** ("your
    organisation's Claude Code policy decides where requests go — use the Claude Code (as
    configured) profile"). The absence of files is not taken on trust beyond these sources.
  - **Server-managed settings** come with an organisation's claude.ai account (Team / Enterprise)
    and are part of choosing that account. Claude Code caches them in the profile's own config
    folder (`remote-settings.json`), which Echo owns: the same check runs on that copy and refuses
    on a routing `env` / helper. A policy the organisation pushes for the first time, or changes, is
    fetched by Claude Code at start-up or on refresh and applies as that organisation's rule for its
    own account (residual, documented). `local` profiles never sign in to an organisation, so none applies to them.
  - `local` profiles get their own config folder too (`<config dir>/claude/<id>`), so nothing of the
    user's default folder (hooks, plugins, env) reaches the run.
- Every job records the profile id and model it ran with (`live-notes.json`, `live-asks.jsonl`,
  `notes-run.json`), shown in the window.

### Pinning the call's profile

- `Armed`, `MeetingContext` (scheduled meetings), the window's Record menu, the tray's Record
  submenu and `arm --ai` carry an explicit choice; at recording start the effective profile
  (explicit, else `default`) is written into `Meta` as `ai_profile`. Changing the default later
  never changes an existing call.
- Mid-call change: window → IPC → `ai-profile.request` (`{call, profile}`, the folder name) → the watcher updates the
  `Recording`'s `Meta` and rewrites `meta.json` (the recording owns the file until `finish`, which
  rewrites it whole). After the recording, "Write notes" with another profile writes `meta.json`
  itself (no owner any more) and runs with it.
- Each job freezes profile and model at launch.

## How it works

### The AI job scheduler (app side, `livenotes.rs`)

- One worker thread in the app runs **live jobs** for the current call: summaries and asks, one at
  a time; a pending ask goes before a pending summary.
- **Local model gate:** `local` **and `system`** jobs are gated — `system` may well be a local
  model too (a data folder's `.claude/settings.json` can point it at Ollama), so it is treated as possibly local;
  only `claude` profiles (Anthropic's servers) aren't. The gate is a **process-shared lease**: an
  exclusive lock on `<config dir>/local-model.lock` (`flock` / share-mode, like `app.lock`), one for
  all gated profiles, held for the whole job by whichever process runs it — the app's live worker,
  the app's minutes thread, or a `notes --wait` CLI process.
  - Live jobs try the lease without waiting; taken → state "waiting for another call's notes to
    finish", a summary retried in 10 s, a question in 5 s (it stays queued; other calls' jobs go first).
  - Minutes on a gated profile wait while a recording or a transcription is in progress (any call:
    the watcher's `recording`, a live `.transcribing` marker), then for the lease. Enforced in
    `notes::run`, so the window, `notes-<call>.request` and `notes --wait` all obey.
  - A gated minutes run already going when a recording starts finishes; live jobs wait for it.
  - Phase 2 measures the real case: repeated updates and questions during a replayed call being
    transcribed, and a minutes run that was already going.
- Live notes start with a recording when `live_notes` is on and the call's profile resolves.

### Minutes waiting their turn

- `notes-run.json` gets `state: "queued"` (with `message`: "waits for the call to end" / "waits for
  Qwen to finish another call's notes") and the **pid of the process holding the place** (the app,
  or the `notes --wait` process). `queued → running → done | skipped | failed | needs_login`.
- Admission is atomic across processes: a run takes its place by creating `notes-run.lock` in the
  call folder with `create_new` (holder pid inside; a lock whose pid is dead is taken over), then
  writes `queued`. `notes::check` refuses a second run while one is `queued` or `running`; the place
  keeps the call's `.claimed`.
- `notes::status`: `queued` or `running` with a dead pid → reported as interrupted ("Echo quit
  before the notes were written — Write notes again"), and the claim is released. Queued places are
  not restored after a restart.
- The window and the terminal pane treat `queued` like `running`: pending, the reason shown, polled
  until a final state.

### Live summaries

- **Cadence:** a job is queued at most every 90 s, with ≥ 400 new characters, never two at once.
- **Input cursor:** the number of complete lines of `live.jsonl` already summarised. Every newly
  arrived record is included whatever its timestamp (the mic and remote tracks are appended
  separately), sorted by start for the prompt, each as `[mm:ss] Me|Remote: text`. The cursor
  advances only after a validated result is persisted. If the backlog exceeds 12K characters (after
  failures, a pause, a slow model) the newest 12K are used and the skipped span is recorded as a
  gap ("live notes skip 31:10–38:40"), shown in the tab.
- **Prompt:** system prompt (below) + the previous live notes JSON + the new lines.
- **Output:** the model's text must hold one JSON object `{now, decisions: [{text, t}], actions:
  [{owner, task, t}], questions: [{text, t}]}` (English; `t` as `mm:ss`). Parsed (the first `{` … last
  `}`), validated (≤ 6 items a list, ≤ 300 chars an item, `t` within the call, anything else
  dropped). Invalid → the previous draft stays and the state says the update failed.
- `<call>/live-notes.json`, written atomically: `{profile, model, updated_at, cursor, covered_until,
  gaps, now, decisions, actions, questions}`.

### Quick buttons and the ask box

- Window → IPC (`live_ask {call, kind, text}`) → the scheduler. Input: the question, the live notes, and the
  transcript: last 5 min ("What did I miss?"), last 10 min ("Suggest a question"), or the whole call
  so far for a typed question (newest 24K characters; the live notes stand in for the rest).
- Answers → `<call>/live-asks.jsonl` `{asked_at, kind, question, answer, profile, model, error?}`.
  One question at a time; the button shows it is busy.
- System prompts: `MISSED_SYSTEM` / `QUESTION_SYSTEM` / `ASK_SYSTEM` in `livenotes.rs` (from the mod's former
  `QUICK`), always answering in English and citing `[mm:ss]`.

### Running a live job

- `local`: `POST {base_url}/v1/messages` (`x-api-key: token`, `anthropic-version`), `max_tokens`
  1,200 (a summary; 600 a question), `thinking: {"type": "disabled"}`, `system` + one user message; connect timeout 5 s, overall
  deadline 120 s. Response: the text blocks. Sent with **curl** as a child process (as `graph.rs`
  does: the config, token included, on stdin), in its own process group — so a cancelled request is
  a stopped process and a closed socket, the same mechanism as for Claude Code. `curl -q -sS -K -`:
  `-q` first, so the user's `.curlrc` (which could turn `location` on) is not read; no `-L`, so
  redirects are not followed. A loopback `base_url` (`localhost`, `127.0.0.0/8`, `::1`) gets
  `noproxy = "*"`, so an inherited `http_proxy` can't send "this Mac" traffic elsewhere.
- `claude` / `system`: `ai::claude_command` with `-p --output-format json --system-prompt <live
  prompt> --tools "" --strict-mcp-config --no-session-persistence --model <live model>`, the user
  message **on stdin** (then closed), cwd = Echo's state folder for `claude` (it loads no project
  settings anyway); `system` runs where its minutes run (`notes_in`, else the data folder), so that
  folder's `.claude/settings.json` applies — strictly: if it can't be used, the job fails. Env
  `ECHOCTL_HEADLESS=1`. Parse the result envelope (`is_error`, `result`) before the model's JSON;
  sign-in errors become "needs sign-in" (`notes.rs::needs_login`). Deadline 90 s.
- `--bare` is not used (it skips the keychain: no claude.ai sign-in); `--safe-mode` measured no gain.

### Cancellation and recovery

- Every job is a child process (curl or Claude Code) in its own process group (Unix
  `process_group(0)`): cancel = `SIGTERM` to the group, `SIGKILL` after 3 s, reaped on the job's
  thread; Windows `taskkill /T /F`. Closing curl's socket stops Ollama's generation (measured). The
  lease is released only after the child is reaped, so the next job can't overlap a dying one.
- **A call is identified by its stamp**, the `YYYY-MM-DD_HHMMSS` that starts its folder name:
  retitling (`echoctl title`, the rename at the end of a minutes run) changes only the slug after
  it. The scheduler keys its state by stamp, asks name their call, and a result resolves the
  folder by stamp when it is saved (`calls::by_stamp`), so an answer that finishes after a rename
  lands in the renamed folder; a write that fails because the folder moved in between is resolved
  and retried (up to 3 times). The window follows a selected call that was renamed (same stamp).
- Each result is bound to its call and a per-call generation number (bumped by pause and profile
  change); a result whose generation is stale is dropped.
- Recording stops: no new summaries; an in-flight one may finish; asks keep working.
- App quits: all jobs cancelled. App restarts mid-call: the recording is closed by the existing
  recovery; `live-notes.json` keeps the last draft. The scheduler's state lives in memory;
  `live-state.json` is only a mirror for the terminal pane (never read back by the app), and the
  app rewrites it as `ended` for calls it finds there at start.

### Prep card ("Last time")

- No model, deterministic: `previous_with_notes(subject, Some(this call))` once → that call's date,
  open action items (`actions.json`, any `status` but `done`) and up to three highlights (`recap_from_notes`,
  labelled "Highlights" — it may be decisions or summary lines). Exposed as `echoctl prep <call>`
  (JSON) and in `/api/call/<name>`. Matching is by meeting subject (as `context` does today).

### The window

- `/api/call/<name>` gains `live` (`live-notes.json` + scheduler state + `prep` + answers).
- The page polls the call while it records or transcribes, while a live job or ask is pending, and
  while the Live tab is open on a call with live data.
- The Live tab's right column re-renders on its own (live notes, answers) without replacing the ask
  box: the typed question survives refreshes, and the "skip rendering while an input has focus" rule
  doesn't freeze the notes.

### The terminal pane (mod) — in v1, so the profile is never bypassed

- The pane **never** calls `$.model.complete` any more, whatever the setting: it shows
  `live-notes.json` / `live-asks.jsonl` (and `live-state.json`: updating, failed: why), and its
  buttons call `echoctl live ask <call> --kind …`. Live notes off, or an app without them → the pane
  says so ("Live notes are off — menu → AI for calls") instead of summarising on its own.
- Its automatic processing (start-up catch-up and `transcribed`) calls
  `echoctl notes <call> --auto --for <session cwd>` instead of claiming and submitting
  `/echo:process-call`. The app owns the whole step: the same eligibility as `pending --for` (inside
  `notes_in`, transcribed, no notes, ≥ 3 min, < 48 h), the claim taken by the run itself, and
  idempotent (a call already `queued`, `running` or `done` is a no-op), so several open sessions
  can't start it twice. Typing `/echo:process-call` by hand in a session stays possible: that is the
  user's own choice of model.

### Safety

- No tools in live jobs; the window escapes everything it renders (as today).
- Live notes are a draft for the user during the call; they never feed the minutes. CLAUDE.md's
  "Notes/actions are written only by a Claude session" is scoped to the final `notes.md` /
  `actions.json`, with this exception written down: model-authored live drafts the app saves, which
  never feed the minutes.
- `live-notes.json` and `live-asks.jsonl` stay in the call folder, next to the transcript.

### Live system prompts

In `livenotes.rs`: `SUMMARY_SYSTEM` (the JSON notes, the whole call each time: every previous item
that still holds is kept), `MISSED_SYSTEM`, `QUESTION_SYSTEM`, `ASK_SYSTEM`. The prompt also gets a
call header (title, invited people); a typed question gets the live notes.

## Phases

0. Measure — done (above). Still open: jobs on two signed-in config folders side by side; live jobs while a
   recording is transcribed (checked in phase 2 with `echoctl live-replay`).
1. Profiles: `ai.json`, resolver, launcher, `meta.json.ai_profile`, Settings → AI for calls, the
   Record / tray / scheduled menus, minutes through the call's profile with the local gate.
2. Live summaries + Live tab + prep card; the pane reads them.
3. Quick buttons + ask box (window and pane).
4. Later: "You were asked" alerts; live speaker names from Teams' tile ring (`KNOWN_MARKERS`); a
   floating window; Windows testing (all shared code, untested there).

Phases 1–3 ship together.

## Tests

- Resolver: pinned → profile; missing / unreadable / not signed in → error, never another profile.
- Launcher: `routing_var` over the known names and families (and not `PATH`, `HOME`, `HTTPS_PROXY`);
  the env script (family removal, quoting of values with spaces and quotes); Windows env; a
  managed-settings `env` with a routing variable or `apiKeyHelper` → refused.
- Minutes queue: `queued` with a dead pid → interrupted + claim released; a second request while
  queued is refused; `--auto --for` outside `notes_in` → no-op.
- Stamps: a result for a call renamed meanwhile lands in the renamed folder.
- Local transport: loopback → `noproxy`; a slow fake server is stopped by pause / profile change and
  the lease is free afterwards.
- Cursor: out-of-order records, partial last line, advance only on success, backlog cap and gaps.
- Output parsing: malformed JSON, prose around the JSON, out-of-range `t`, oversized lists.
- Scheduler: asks before summaries; local gate vs a queued minutes run; stale generations dropped.
- `ai.json` load/save: atomic, never saved after a failed load; no `ai.json` = `system` only.
- The existing checks: `cargo clippy`, `cargo test`, the Windows `cargo check`, `scripts/check-mod.sh`.

## Open questions

- Two signed-in config folders on one Mac (keychain) — see **As built**.
- Qwen while the call is transcribed — measured in phase 2.
- Keep `live-notes.json` after the minutes exist (small, useful to compare), or remove it?

## As built (2026-10-23)

Where the code differs from the text above, after the implementation and its reviews
(`/pr-review-toolkit:review-pr`: code, silent failures, tests, comments, types):

- **Settings → AI for calls** is a tray menu ("AI for calls": live notes on/off, the default for new
  calls, each Claude account with its sign-in state, Sign in…, and Use <account> when it is signed in
  to another one). Adding and removing
  profiles is the CLI (`echoctl ai add-local | add-claude | remove | default | live | status | login |
  accept | set`). Sign-in (2026-10-23, design reviewed with a second model over 3 rounds): no
  terminal — Echo runs `claude auth login --claudeai [--email] [--sso]` itself (`signin.rs`), the
  browser calls Claude Code back on localhost, Echo notices the child's exit, checks the account once
  and resumes live notes at once; choosing a signed-out Claude profile in a menu starts it; the
  window shows the state with Sign in… / Cancel / Use X. A per-profile reader/writer lock keeps jobs
  and sign-ins apart across processes; `echoctl ai login` signs in in the terminal it was run from,
  starting Claude Code with `build_command` directly (no login shell, not detached), not through
  `claude_command`. See CLAUDE.md ("Signing in").
- The tray's Record submenu carries no profile (it records with the default); the window's Record
  menu, scheduled meetings and Join do.
- `ai::resolve` is `ai::for_call`; the old `notes.rs::command` is `ai::claude_command`.
- **Recording never depends on AI settings**: `arm` stores the chosen id unchecked (only a typed
  `arm --ai` is checked); an unreadable `ai.json` at recording start pins `!unreadable`, whose jobs say
  so. An unreadable `meta.json` is an error for AI jobs (never the default); all `meta.json` writes
  are atomic.
- The profile's values reach the login shell as `ECHOCTL_P_<name>` and are exported after the routing
  variables are removed (never in the script's text). Children get their own session (`setsid`):
  a process group alone made an interactive login shell started from a terminal stop on SIGTTOU.
- An account is compared by email · organisation (not the plan); the plan shows in "Signed in as …"
  right after a sign-in, and in `echoctl ai status`.
- Stopping: pause stops summaries only; a profile change stops the running job (a summary is
  dropped, a question asked again with the new profile); the worker checks before and while a child
  runs and stops it (TERM, KILL after 3 s).
- A stamp shared by two calls (same second) is never resolved by guessing; the scheduler keeps the
  folder it last saw.
- Automatic notes (`--auto`) run only for a call never tried (a skipped or failed run is left to
  Write notes) and answer `{requested, reason}`; the pane shows the notes run's state and reason.
- Size limits count characters, not bytes.
- After the second model's code reviews: the call's place is an OS lock on `notes-run.lock` (held by the owner
  for the whole run), the claim is taken under it (a session's fresh claim wins; a refusal changes
  nothing), quitting stops minutes runs' Claude Code and starts none after, a dead owner whose
  Claude Code still runs keeps the call, the run
  finishes in the folder as named after Claude Code, prompts are fed on a thread of their own, every
  `ai.json` change goes through `Settings::update` (lock + re-read), and `capture` pins like the
  watcher (`ai::pin`).
- After the review for upstream: `system` runs only where it is set up (`ai::system_folder`: an
  unreadable `config.json` or a missing `notes_in` folder fails the job, live or minutes, instead of
  running it elsewhere); a skipped minutes run is never read as a sign-in problem, and a `local`
  profile's refused token says how to set it (`notes::conclude`); request files (`live-*`, `notes-*`,
  `ai-profile`) find a retitled call by its stamp, and say in `app.log` when they can't; `notes --wait`
  stops its Claude Code on Ctrl-C; `ai.json` is 0600 from creation and a new default must name a
  profile; on Windows the routing variables are removed whatever their case; the recording's
  `finish` keeps a profile switch written straight to `meta.json`; a question put back says what it
  waits for (the model, or a sign-in).
- The window doesn't re-render while text is selected or a menu is open; the "Last time" card keeps
  the open state the user chose; after the minutes are `done` the call opens on Notes once.
- Tests that exist: routing (family matcher, the shell script in sh/zsh/bash, the no-shell command),
  policy refusal (routes, a cached server policy), strict resolution (missing / unpinned / unreadable),
  settings validation and 0600, the account check, the local transport (answers, errors, `-q` against a
  `.curlrc` that follows redirects, stopping a request), the summary end to end (success, failure keeps
  the draft and cursor, `{}` refused, the lease freed), the scheduler order, the `FileLock`, the cursor,
  stamps, prep, `--auto` refusals, the minutes place and interrupted runs. Not tested: the Windows
  runtime, jobs on two signed-in Claude folders side by side (checked on one Mac: a profile folder
  signed in keeps its own Keychain item, and the default folder's sign-in to another account stayed
  as it was), live jobs under real transcription load.
