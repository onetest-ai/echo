# Echo plugins

Status: **v3, 2026-11-03** — the plan we build from. v2 (2026-10-29, below under *v2: exploration*) is kept for
its UI decisions; where they disagree, v3 wins. Since v2 the terminal, the task tracker (Threads' work ledger),
Review (→ Needs you / Inbox) and the ⌘K palette became core, so its two motivating examples are gone.

## What changed in v3

- **A plugin is a declarative package, no code of its own.** Everything that runs is Echo's (Rust): plugins combine
  Echo's **capabilities** (below) and bring content — skills, MCP servers, settings, UI entries. No WASM,
  no plugin pages, no second origin in this round. A capability Echo lacks is added to Echo, then any plugin uses it.
- **Marketplaces in Settings.** A marketplace is a catalog of plugins. Echo ships ours (public, built in); the user
  adds others in Settings → Plugins → Marketplaces — an enterprise's own catalog is one more marketplace. Which plugins
  a marketplace offers and promotes is its maintainers' business, not Echo's.
- **The first three plugins**: Scheduler (routines), Telegram, Obsidian vault — in our public marketplace, each
  one a manifest over capabilities Echo gains for them.
- Integration with other systems keeps the 2026-10-30 rule: **an MCP connection + Echo's ingestion / egress points
  + a skill for judgment**.

## Invariants (unchanged)

| Invariant | For plugins |
|---|---|
| Recording is opt-in per meeting | No plugin arms, starts or stops a recording. |
| Nothing leaves by default | A plugin that sends anything names where, in words, at install; routines deliver to the user (their Telegram, their Teams chat with themselves, their own mailbox) — and mail to the addresses the user sets on a routine (2026-11-03), never ones a model picked. |
| Strict profile routing | Every Claude run a plugin causes goes through the user's AI profiles (`ai::claude_command`, `ai::lease`) — never a plugin's own key or account. |
| Others' words stay data | Transcripts, chat, mail reach a run with tools only under a guard; mail and posts reach the Brain only as checked facts (the quarantine reader). |
| `notes.md` / `actions.json` are the minutes run's | Plugins read them; an export copies them, never edits them. |
| macOS **and** Windows | A package has no per-OS parts. |

## Plugin package

```
echo-plugin.json      manifest
skills/               Claude Code skills (loaded with --plugin-dir into the sessions / runs the manifest names)
templates/            optional: note templates, export templates
README.md             shown in the marketplace (Echo's escaping Markdown writer)
```

```json
{
  "id": "obsidian",
  "name": "Obsidian vault",
  "version": "0.1.0",
  "echo": ">=0.7.0",
  "summary": "Use your Obsidian vault as Echo's second brain, and keep minutes and people in it",
  "uses": {
    "brain_source": { "kind": "vault" },
    "export": { "kind": "vault", "what": ["minutes", "people"] },
    "sessions": { "read": ["vault"], "skills": ["obsidian"] }
  },
  "settings": [
    { "key": "vault", "label": "Vault", "type": "folder" },
    { "key": "map", "label": "Folders → clients / threads", "type": "scope_map" },
    { "key": "export_folder", "label": "Echo writes into", "type": "string", "default": "Echo" }
  ]
}
```

`uses` names capabilities, each with its own small schema checked by Echo; an unknown capability or a newer
`echo` refuses the install with the reason in words. The permission sheet is written from `uses`, never from the
plugin's own text.

## Capabilities (what Echo implements)

| Capability | What it is | Built on |
|---|---|---|
| `routines` | Scheduled headless Claude runs (below) | `ai::claude_command`, the notes run's machinery, `guard.rs` |
| `deliver` | Send text to the user: `telegram`, `teams` (chat with yourself, `48:notes`), `mail` (to yourself) | Bot API `sendMessage`; Graph `POST /chats/{id}/messages`, `/me/sendMail` (the sign-in already carries `ChatMessage`/`Chat.ReadWrite`, `Mail.Send`) |
| `channel` | A Claude Code channel plugin attached to the **Echo-folder session** | `terminal.rs` folder session + `--channels` |
| `brain_source` | A new kind of Brain source | `brain/files` (registry, readers, sync) |
| `export` | Echo writes copies of its own records into a folder the user picked | `paths::write_atomic`, generator marks |
| `sessions` | Skills / MCP servers / read folders added to Claude in the window | `terminal::brain_mcp_config`, session settings |
| `ui` | ⌘K actions, Inbox items, a Settings section, a call's ⋯ entry | the workbench registries |

Not offered: recording control, writes to call files, network beyond `deliver` and named MCP servers, arbitrary
processes.

## Marketplaces

A marketplace is a git repository (GitHub `owner/repo`, or any https git URL — an enterprise's GitLab / Azure
DevOps) with `echo-marketplace.json` at its root:

```json
{
  "name": "onetest",
  "owner": "onetest-ai",
  "plugins": [
    { "id": "scheduler", "source": "./plugins/scheduler", "categories": ["automation"] },
    { "id": "telegram",  "source": "./plugins/telegram",  "categories": ["chat"] },
    { "id": "obsidian",  "source": "./plugins/obsidian",  "categories": ["knowledge"] }
  ]
}
```

- **Built in**: `onetest-ai/echo-plugins` (public; all of ours live there, each in `plugins/<id>/`; a plugin may also point to its own repo at a ref). Can be hidden, not removed.
- **Added by the user**: Settings → Plugins → Marketplaces → Add (`owner/repo`, a git URL, or a local folder for
  authors); `echoctl plugin marketplace add|remove|list|update`.
- Any number of marketplaces; the same plugin id in two is shown with each one's name, and the user picks which
  to install from. A marketplace's `featured` list orders Discover (promotion is the maintainers' job). No
  managed / locked configuration in this round.
- Fetching is the app's (never the page's): a shallow fetch of the repo at its default branch, or a pinned ref;
  every plugin's files are recorded with the **commit SHA** and a hash of the extracted files; an update is a new
  SHA. Private repos: the user's `git` / `gh` credentials (enterprises already have them), never stored by Echo.
- Installed plugins: `<config dir>/plugins/<id>/<sha>/`, list in `<config dir>/plugins.json` (lock + re-read, as
  `ai.json`); their settings and state per data folder in `<home>/.echo/plugins/<id>/`.
- Install = the permission sheet (from `uses`), Install / Cancel (Cancel the default outside the built-in
  marketplace). An update asking for more capabilities waits for the user; within the built-in marketplace and with
  nothing new asked, updates install themselves with Echo's own update check.

### Settings → Plugins

```
Plugins                                          [ Search ]
Installed (3)   Discover   Marketplaces (2)

Scheduler      onetest · v0.1.0   On ●   3 routines · last ran 09:00 ✓        ⋯
Telegram       onetest · v0.1.0   On ●   @my_echo_bot · paired with you       ⋯
Obsidian vault onetest · v0.1.0   Off    Needs: a vault folder                ⋯

Marketplaces:  onetest (built in) · acme-corp · + Add marketplace
```

A plugin's row opens its section: what it uses (in words), its settings, its state, Remove (keep or delete its data).

## The first three plugins

### Scheduler — routines

A routine = a prompt Claude Code runs headless on a schedule, its answer delivered to the user.

```yaml
# <home>/.echo/routines/<id>.md   — written by the user (or by Claude for them); plugins ship no routines
---
name: Monday asks
schedule: "mon 09:00"            # daily HH:MM · weekdays HH:MM · <day> HH:MM · every 2h · cron
profile: default                 # an AI profile id; model: its minutes model unless set
tools: read                      # the user picks: read (default) · folder (the Echo-folder session's permissions) · skip (no prompts)
deliver: [telegram, teams]       # telegram · teams · mail — always to the user
skip_empty: true                 # nothing to say → nothing sent
---
What do I owe this week and what is waiting for others? Use the Brain and `echoctl project tasks`.
```

- Runs in the data folder (as the window's sessions), `claude -p … --output-format stream-json` through
  `ai::claude_command` on the routine's profile under its lease; Echo's Brain MCP attached when the Brain is on and
  narrowed to that profile (`brain/access.rs`).
- `tools: read` = a guard (`RUN_MODE=routine`): Read / Glob / Grep in the data folder, Bash only read-only `echoctl`
  commands, `mcp__echo-brain__*`; no Edit / Write. `tools: folder` = the Echo-folder session's settings (incl.
  `folder_skip_permissions` when on); `tools: skip` = `--dangerously-skip-permissions`. The routine's editor says
  what each one can do ("can change files", "asks nothing"); a headless run can't ask, so in `folder` a tool that
  would need a prompt is refused and the run says which.
- Missed runs (asleep, Echo off) run once when Echo is back; never two of one routine at a time; local-model
  profiles share the `local-model.lock` and wait while a call records, as minutes do.
- Each run: `<home>/.echo/routines/runs/<id>/<stamp>/` (prompt, log, answer, deliveries). A failed run or delivery →
  Inbox ("Routine ‘Monday asks’ failed: …", Run again). Delivery formats: Telegram HTML subset (split at 4096),
  Teams HTML, mail HTML — all from Echo's safe Markdown writer.
- UI: Settings → Plugins → Scheduler (list, New routine, Run now, last runs) + ⌘K "Run routine…"; making one in
  words is a skill (`scheduler:new`) in the Echo-folder session that runs `echoctl routine add`.
- CLI: `echoctl routine list|add|edit|remove|run <id>|runs <id>`. The code is `routines.rs` (`schedule.rs` is
  "Record when it starts").

### Telegram — the Echo-folder session from the phone

One user, one session: Telegram talks to **the same Claude · Echo folder session** the window shows — no second
Claude, no Echo-side bridge.

- The session starts with `--channels plugin:telegram@claude-plugins-official`, its profile = the folder session's
  (Settings: which profile). Echo installs the channel plugin into that profile's Claude Code folder
  (`claude plugin install telegram@claude-plugins-official` through `ai::claude_command`).
- The bot token: Settings → Plugins → Telegram, stored by Echo (0600) and given as `TELEGRAM_BOT_TOKEN`; the
  channel's state in `TELEGRAM_STATE_DIR=<config dir>/telegram/` (not the profile's folder). Pairing: Echo shows
  "send /start to @bot" and the code the bot answers; Echo runs the pairing and sets the allowlist policy — only
  the user's Telegram id.
- Each person configures **their own bot**; it answers only while their Claude · Echo folder session is open in
  Echo (the channel lives in that process). Closed session → the bot is silent; Telegram keeps the messages ~24 h
  and the session reads them when it opens again (to check in the spike). Nothing marks a message "from Telegram"
  in the window: it is the same conversation.
- Delivery for routines uses the same bot (`sendMessage` to the paired id, any time — sending needs no open
  session) — no Claude involved.
- Limits we inherit, said in the sheet: a research preview; needs Bun; a claude.ai login or Console key (not
  Bedrock / Vertex / a gateway); on Team / Enterprise an admin must switch channels on. Echo checks what it can
  (Bun present, profile kind, the `channelsEnabled` managed setting) and says which one blocks it.
- To spike first: `--channels` with `--resume`; whether the channel plugin declares permission relay (prompts on the
  phone); `access.json` written by Echo vs the `/telegram:access` command.

### Obsidian vault — the user's second brain, kept

Read (needs the Brain on):
- A Brain source of kind `vault` (personal, not a thread's): Markdown with frontmatter, `[[wikilinks]]` (and
  `#heading`, `|alias`), `#tags` / nested tags, embeds as links; `.obsidian/`, `.trash/`, templates, attachments and
  Echo's own export folder skipped (globs in Settings). A note → a document, its headings → sections; wikilinks →
  `related` edges; frontmatter `tags` / `date` / `aliases` kept (dates as `exact_date`, aliases as names).
- Each section links back with `obsidian://open?vault=<name>&file=<path>` (the window opens it in Obsidian).
- Scope: the vault belongs to the default profile; a folder or tag map (`Clients/Acme/**` → Acme, `#proj/alpha` →
  thread Alpha) gives sections a client / thread, so they appear in those scopes. Unmapped = the practice.
- Claude in the window: the session may Read / Grep the vault (an allow rule for its path) and loads the plugin's
  `obsidian` skill (navigating wikilinks, tags, MOCs).

Write (export, opt-in per kind):
- Into `<vault>/<export_folder>/` (default `Echo/`): `Calls/<date> <title>.md` — the minutes as written by the
  minutes run + frontmatter (date, attendees as `[[people]]`, threads, client, `echo://call/<name>` link);
  `People/<Name>.md` — role, organisation, threads, facts with their sources.
- Echo owns only files it wrote (a generator field in their frontmatter, `echo: <id>`): it rewrites those, never a
  file the user made or edited since (its hash differs) — that one is kept and the new version written beside it.
- Written after minutes are written / a person changes; `echoctl vault export [--all]`.

## Phases

| Phase | Scope |
|---|---|
| 1 (built 2026-11-03) | Plugin registry: manifest + `uses` validation, `plugins.json`, marketplaces (built in + any number added), install / update / remove from a local folder and git, Settings → Plugins (Installed, Discover, Marketplaces), `echoctl plugin …`. The three plugins' manifests in `onetest-ai/echo-plugins`. |
| 2 | `deliver` (Telegram Bot API, Teams self-chat, mail) + `routines` → **Scheduler** — built 2026-11-03 (the bot's token is Echo's own delivery setting, shared with phase 3) |
| 3 | `channel` on the folder session (while it is open) → **Telegram** (spike items first) |
| 4 | `brain_source: vault` + `sessions` read access → **Obsidian** read; then `export: vault` |
| later | Plugin pages / code (v2's WASM and second origin), plugin call tabs and panels, signatures |

## Decided after review (2026-11-03)

1. Telegram: one bot per person, their own; it answers only while their Echo-folder session is open. No marking.
2. Routines are the user's own (no templates from plugins), so the user picks their permissions — the prompt isn't
   someone else's text, the injection risk is what the routine reads (as in any session).
3. Enterprise managed configuration is out of scope: several marketplaces, promotion inside a marketplace is its
   maintainers' job.

---

# v2: exploration (2026-10-29)

Kept for its window / UI decisions. Its WASM host, plugin pages and second origin are deferred (v3 → *later*).

## Goal

Grow Echo with packages installed from GitHub instead of building every feature into the core.
Two examples set the requirements:

1. **Terminal in the Echo window.** A real terminal (Claude Code inside it) as a panel, so Echo
   never builds its own chat UI, onboarding wizards or "ask about this call" boxes: anything
   conversational opens a terminal running `claude` in the right folder with the right skill.
2. **Task tracker** as a main menu item — a port of taska (`onetest-ai/applied_skills`,
   `bundles/taska`, used as a reference only): an Epic → Story → Task tree kept current from calls,
   chats and (later) files and pull requests; every change cites a verbatim quote, a second agent
   verifies it, and a workbench shows the tree, the board, the history and the asks.

Neither one fits the "sandboxed exporter" plugin of v0. The terminal needs a PTY and a child
process; the tracker needs its own page, a store, scheduled jobs, model runs, and the call data.
So the plugin system is designed around **host primitives**, which Echo provides and plugins
combine, with plugin code sandboxed.

## What must not change

| Invariant | What it means for plugins |
|---|---|
| Recording is opt-in per meeting | No plugin can arm, start or stop a recording. |
| On-device by default; no cloud service by default | No network for plugin code unless the plugin declares hosts and the user grants them. Model runs go only through the user's AI profiles (`ai::for_call`), never a plugin's own key. |
| Transcripts and chat are others' words | Model runs a plugin starts are guarded like minutes runs: only the tools and CLI commands the plugin declared, only the data it was granted. |
| macOS **and** Windows | One package runs on both: no per-OS binaries, no runtime the user must install (no Python, Node). |
| Hardened runtime, library validation | No native plugin code loaded into Echo. |
| The minutes are Claude Code's, `notes.md` / `actions.json` written only by the minutes run | Plugins read them, never write them. |

## Plugin anatomy

```
echo-plugin.json        manifest: id, version, permissions, what it contributes
ui/                     the plugin's pages (HTML/JS/CSS, vendored, no remote scripts)
plugin.wasm             optional: deterministic logic (a WebAssembly component)
skills/                 optional: Claude Code skills for its judgment steps (EXTRACT, VERIFY …)
templates/, vocabulary.json, …   optional content
```

Division of labour, the same rule Echo already follows: **deterministic → plugin code (WASM)**,
**judgment → its skills, run by Echo through Claude Code**, **interaction → a terminal**,
**display → its pages**.

## Host primitives

What Echo gives plugins. Each one is declared in the manifest and granted at install.

| Primitive | What it is | Built on |
|---|---|---|
| **Nav item** | A main entry in the window's sidebar (beside Calls / Today / Calendar) and optionally in the tray menu (opens the window there) | `window.rs` routes, `tray.rs` menu rebuild |
| **Page** | The plugin's `ui/` in an isolated frame, its own origin; talks to the host by postMessage only | The Minutes frame pattern (`EMBED_JS`, accepted only from that frame's `contentWindow`) |
| **Terminal** | A PTY running a declared command (default `claude`), shown with a vendored xterm.js, in a page, a call tab or its own panel | New: `portable-pty` (macOS PTY, Windows ConPTY); output to the page in batches through `evaluate_script`, input over IPC |
| **Store** | The plugin's own folder `<home>/.echo/plugins/<id>/` (per data folder: the tracker's tree belongs with the calls), atomic writes, an append-only log helper | `paths::write_atomic`, `.meta.lock`-style locks |
| **Call data** (read) | List calls, read meta / transcript / segments / notes / actions / chat / my-notes, per granted part | `calls.rs`, `echoctl context` |
| **Events** | `recording_started`, `transcribed`, `minutes_written`, `actions_changed`, `call_retitled`, `call_deleted`, `chat_updated` (the last five are new) | `watch::emit` → `events.jsonl` |
| **Schedule** | "Every day at 18:30", "15 min after minutes", with catch-up after sleep | The notes scheduler (`notes::start_scheduler`) generalised |
| **Claude jobs** | A headless Claude Code run of one of the plugin's skills, with the plugin's skills as `--plugin-dir`, through an AI profile, under a guard built from the plugin's declared tools and commands | `notes.rs` run machinery, `ai::claude_command`, `guard.rs` |
| **Plugin CLI** | `echoctl plugin call <id> <op> [args]` runs a WASM export and prints JSON; this is how a Claude job calls the plugin's deterministic steps | `main.rs`, the guard's `RUN_COMMANDS` pattern |
| **Microsoft 365** | Read-only Graph calls within scopes Echo already has (`Calendars.Read`, `Chat.Read`) | `graph.rs` |
| **Notify, settings, secrets** | Notifications as Echo; a settings form from the manifest; secrets in the OS keychain | `app::notify`, Settings window |

Not offered: recording control, writes to Echo's own call files, running arbitrary processes,
arbitrary file access, plugin code inside the minutes run.

### The terminal, in detail

- `contributes.terminals`: `{id, label, command, args, cwd}` where `cwd` is `data` (the data
  folder), `call` (the open call's folder), or `plugin` (the plugin's store), and `command` is
  `claude` or `shell`. A plugin can only start a terminal the user opens, or one opened by a
  button the user clicks on its page (`host.openTerminal(id, {prompt: "/taska:onboard"})`). It
  never types into a terminal by itself.
- `claude` starts through `ai::claude_command` for the chosen profile (the call's profile in a call
  tab, the default elsewhere; a menu to switch), with Echo's plugin and the plugin's skills
  (`--plugin-dir`). This is an interactive session in a folder the user opened, so it has the
  user's own permissions and modes. The data is the user's own. It is not a guarded run.
- One PTY per terminal panel, kept while the window is closed (the session continues),
  ended when Echo quits (`tray::Leaving` asks first if one is busy).
- The bundled **Terminal** plugin is just a manifest: a nav item "Terminal" (data folder) and a
  call tab "Ask Claude" (call folder). The "What did I miss" buttons stay as they are; anything
  longer goes to the terminal instead of a new chat UI.

## Example: the task tracker as an Echo plugin

What taska does, mapped to the primitives (ported to Rust → WASM; nothing in Python):

| Taska piece | In Echo |
|---|---|
| `taska.toml` + `/taska:onboard` | A **terminal** opened from the tracker's page running the plugin's `onboard` skill; the skill writes config through `echoctl plugin call tasks config …` |
| Sources: meetings folder, msgraph (Teams transcripts, calendar), files, code host | **Call data** (Echo's own calls are the meetings source: transcript, notes, actions, chat, frames' captions); **Microsoft 365** for chats outside calls; files / pull requests later (needs `fs` or `net` permissions) |
| Evidence items, dedup, `events.jsonl`, the fold, level rules, quote check, staleness, asks, prep pack | **Plugin code (WASM)**: `ingest`, `pending`, `apply`, `verify-apply`, `status`, `asks`, `pack`, operating on the **store** |
| `/taska:run` with EXTRACT / DEPS / VERIFY subagents | A **Claude job** of the plugin's `run` skill, triggered by **events** (`minutes_written`) and a **schedule** (nightly); its guard allows only `echoctl plugin call tasks <op>` and reads of granted calls; the subagents call the same commands |
| `workbench.html` + `serve --port 8766` | The **page**: the tree / board / playback / asks UI talks to its WASM by postMessage → host → `panel_request`. No local HTTP server and no port, so the origin checks taska needed are gone |
| `--share` export | `export` writes a self-contained read-only HTML file (as `minutes.rs` does) |
| `/taska:prep` | A page button: a Claude job writes the pack; opening it in the terminal lets the user refine it |
| Main menu item | **Nav item** "Tasks" in the sidebar + tray → Tasks (counts of asks / overdue in the label) |

Echo-side additions this needs: `minutes_written` / `actions_changed` / `chat_updated` events,
plugin Claude jobs (the minutes run generalised: a guard policy per job, not one fixed list), the
scheduler, and the store. The tracker is the reason to build the jobs and the guard policy right;
the terminal is the reason to build the PTY.

## Window layout: a workbench, like VS Code

The window becomes a fixed set of **areas** that the built-in views and plugins fill through one
registry. Built-ins use the same registry as plugins (Calls, Calendar, the call tabs, Open notes).
If the registry is enough for Echo's own views, it is enough for plugins.

```
┌────┬──────────────────────────────────────────────┬──────────────────┐
│ ▣  │  Primary view (the activity's layout)        │  Right panel     │
│Calls│  Calls: list │ call (tabs: Live Notes …)    │  [Terminal|Tasks]│
│ ▦  │  Calendar: one column                         │                  │
│Cal │                                               │                  │
│ ✓  ├──────────────────────────────────────────────┴──────────────────┤
│Tasks│  Bottom panel                      [Terminal | Transcript | …]  │
│ ›_ │                                                                   │
│ ⋯  │                                                                   │
│ ⚙  ├───────────────────────────────────────────────────────────────────┤
│    │  status: ● Recording "Weekly sync" 12:40 · Stop                    │
└────┴───────────────────────────────────────────────────────────────────┘
 activity bar
```

| Area | What it holds | Today |
|---|---|---|
| **Activity bar** | Icons (with labels on hover, badges): Calls, Calendar, then plugins' activities; Plugins and Settings at the bottom | The sidebar's text nav |
| **Primary view** | The selected activity. Each activity picks its **layout**: `list-detail` (Calls: list + call) or `single` (Calendar, Tasks, Plugins) | `shell` / `shell no-list` in `app.js` |
| **Right panel** | One or more views as tabs; one visible at a time; resizable, collapsible | — |
| **Bottom panel** | Same, below the primary view (and the right panel when it is full width) | — |
| **Status bar** | Recording / armed state + Stop, the profile, a plugin's short status item | The sidebar's status card |

Kept lean: the areas are fixed. There's no free docking, no split editors and no drag-and-drop in
v1: a view moves with "Move to right panel / bottom panel" from its tab's menu. Tabs can be
reordered by dragging later.

### Extension points (contributions)

| Contribution | Where it shows | Context it gets | Examples |
|---|---|---|---|
| `activities` | An icon in the activity bar → a primary view (`single` or `list-detail`) | — | Tasks, Plugins |
| `call_tabs` | A tab on a call, after the built-in ones | the call | Tasks for this call |
| `panels` | A view in the right or bottom panel (`default_area`, user can move it) | follows the selection: the open call, the selected calendar event, or none | Terminal, Tasks for the selection |
| `actions` | A button or menu item on an item: calendar event (the popover: next to **Open notes**), call (its header menu, the list's context menu), action item, transcript selection | the item | "Prep in Claude" on an event, "Track in Tasks" on an action item, "Ask Claude about this" on a transcript selection |
| `status_items` | A short text + click in the status bar | — | "Tasks: 3 asks" |
| `settings` | The plugin's Settings tab (Plugins view) and Settings → Plugins → <name> | — | Tasks' project, schedule |

An action says where it applies and when:

```json
{ "id": "prep", "label": "Prep in Claude", "on": "calendar_event",
  "when": ["future", "teams"], "run": { "terminal": "claude", "prompt": "/tasks:prep {{event.subject}}" } }
```

`when` is a short list of flags Echo computes per item (`recorded`, `scheduled`, `future`, `past`,
`teams`, `recording`, `has_minutes`, `has_actions`), not an expression language. `run` is one of:
`{terminal, prompt}` (opens a terminal with the prompt, which the user then sends), `{panel}` (shows
a panel with the item as context), `{op}` (calls the plugin's WASM export, then shows the
answer as a toast or opens a panel). The calendar popover shows at most two plugin actions inline and
the rest under "More…".

Built-in examples of the same points: **Open notes** is a calendar-event action (`when: recorded`),
**Record when it starts** is one (`when: future, teams`), Live / Notes / Minutes / Transcript are
call tabs, and the live pane could become a right panel during a recording.

### Panels: how they run

- A plugin's panel is its page in an isolated frame (see Pages: isolation). Its context comes by
  postMessage when the selection changes: `{echo: 'context', call, event}`. Only the item and the
  data the plugin was granted are sent, never more.
- Hidden panels stay alive (`display: none`, never detached: detaching reloads a frame, as the
  Minutes frame taught us). That matters for the terminal and for panels with state. At most N
  live frames per plugin (e.g. 4); the oldest hidden one is unloaded.
- The **terminal** is a host view (xterm.js + PTY), registered like any panel. Terminals are
  instances: "New terminal" in the panel's tab menu; each one remembers its folder (data / call) and
  profile.
- `render()` already replaces regions only when their HTML changed. The areas become regions
  (`r-activity`, `r-primary`, `r-right`, `r-bottom`, `r-status`), and plugin frames are never
  re-rendered, only shown or hidden.

### Layout settings

Per computer, in `<config dir>/window-layout.json` (written by the app over IPC, not in the
webview's storage, so it survives a data folder change and `echoctl` can reset it):

```json
{
  "activities": ["calls", "calendar", "tasks:tasks"],
  "hidden_activities": [],
  "right":  { "open": true,  "size": 380, "views": ["tasks:selection"], "active": "tasks:selection" },
  "bottom": { "open": false, "size": 260, "views": ["terminal"],        "active": "terminal" },
  "views":  { "terminal": { "area": "bottom" } }
}
```

- Each view's tab menu: Move to right panel / bottom panel, Hide. Each activity icon's menu: Hide,
  Move up / down.
- Settings → Layout lists every view with a right / bottom / hidden choice. That is where the user
  picks "terminal at the bottom". A plugin's own Settings tab shows the same choice for its views.
- A new plugin's views go to their `default_area`. Removing a plugin removes its entries.
- Narrow window (< 900 px): the right panel turns into an overlay over the primary view. Below
  600 px the bottom panel does too.
- Keyboard: ⌘J / Ctrl+J toggles the bottom panel, ⌘⌥B / Ctrl+Alt+B the right panel, ⌘1…9 the
  activities, as in VS Code.

### Migration from today's window

1. Sidebar text nav → activity bar (Calls, Calendar), and the status card → status bar.
2. The view registry in `app.js`: built-in activities, call tabs and the calendar's event actions
   as entries, with `render()` driven by it. No change in behaviour.
3. Right and bottom panels + `window-layout.json` + the terminal as the first panel.
4. Plugin contributions plug into the same registry (phase 1 below).

## Plugin code: WebAssembly components

With Python dropped, the deterministic core of a plugin needs a portable, sandboxed form. A
WebAssembly component (wasmtime, interface in WIT) is one file for both platforms. It is
sandboxed (no files, clock or network except through the host's functions) and needs no signing.
The tracker's core (fold, validation, staleness) is pure logic over JSON, which suits it well.
Authors write Rust (cargo-component); JS and Go also compile to components.

```wit
package echo:plugin@1.0.0;

interface host {
  list-calls: func(since: option<string>) -> list<string>;
  call-part: func(call: string, part: part) -> result<string, string>;   // granted parts only
  enum part { meta, transcript, segments, notes, actions, chat, my-notes, frames-index }
  store-read: func(name: string) -> result<list<u8>, string>;
  store-write: func(name: string, bytes: list<u8>) -> result<_, string>;
  store-append: func(name: string, line: string) -> result<_, string>;
  graph-get: func(path: string) -> result<string, string>;               // granted scopes only
  http: func(req: request) -> result<response, string>;                  // granted hosts, https
  now: func() -> string;
  notify: func(title: string, body: string);
  log: func(level: u8, msg: string);
}

world plugin {
  import host;
  export call: func(op: string, args: string) -> result<string, string>;    // echoctl plugin call
  export on-event: func(event: string, payload: string) -> result<_, string>;
  export panel-request: func(path: string, body: string) -> result<string, string>;
}
```

Time and memory limits per call (epoch interruption, a memory cap). One instance per plugin, with
calls serialised, so the store needs no locking of its own. To measure in the spike: what it adds to
the binary (expect ~10–20 MB) and how long a cold start takes.

## Pages: isolation

A page is served from its own origin: a second custom protocol (`echo-plugin://<id>/`; on Windows
`http://echo-plugin.localhost/<id>/`, to be checked against wry's mapping). It never shares the
window's `echo://` origin, so it can't read `/api/*` or reach the window's IPC (`from_page` already
checks the URL). Strict CSP: `default-src 'self'`, `connect-src 'self'` (its own origin only,
answered by its WASM). The host API is a small `window.echoHost` shim that posts messages:
`call(op, args)`, `openTerminal(id, opts)`, `openCall(name)`, `subscribe(events)`. Everything the
page renders from calls is others' text and must be escaped. The plugin's README says so, and the
starter template does it.

## Manifest

```json
{
  "id": "tasks",
  "name": "Task tracker",
  "version": "0.1.0",
  "echo": ">=0.3.0",
  "api": 1,
  "source": "onetest-ai/echo-tasks",
  "permissions": {
    "calls": ["meta", "transcript", "notes", "actions", "chat"],
    "graph": ["Chat.Read", "Calendars.Read"],
    "net": [],
    "claude_jobs": true
  },
  "contributes": {
    "activities": [{ "id": "tasks", "label": "Tasks", "icon": "ui/icon.svg", "layout": "single",
                     "page": "ui/index.html", "tray": true }],
    "call_tabs": [{ "id": "tasks", "label": "Tasks", "page": "ui/call.html" }],
    "panels": [{ "id": "selection", "label": "Tasks", "page": "ui/panel.html", "default_area": "right" }],
    "actions": [
      { "id": "track", "label": "Track in Tasks", "on": "action_item", "run": { "op": "track" } },
      { "id": "prep", "label": "Prep in Claude", "on": "calendar_event", "when": ["future"],
        "run": { "terminal": "claude", "prompt": "/tasks:prep {{event.subject}}" } }
    ],
    "status_items": [{ "id": "asks", "op": "status_text" }],
    "terminals": [{ "id": "onboard", "label": "Set up the tracker", "command": "claude", "cwd": "plugin" }],
    "jobs": [{
      "id": "run", "skill": "tasks:run", "args": "--due",
      "on": ["minutes_written"], "schedule": "daily 18:30",
      "model": "profile",
      "allow": { "cli": ["plugin call tasks"], "read": ["calls"], "tools": ["Bash", "Read", "Task"] }
    }],
    "settings": [{ "key": "project", "label": "Project name", "type": "string" }]
  }
}
```

## Installing from GitHub

```
echoctl plugin install owner/repo[/path][@ref]
echoctl plugin list | update [<id>] | disable <id> | remove <id> | info <id>
```

- **Source**: a release asset `<id>-<version>.echoplugin` (zip) with the API's SHA-256 `digest`
  required (the `update.rs` path: public API, else `gh`), or a repository path at a ref resolved to
  a **commit SHA** (the tarball for that SHA, only `path/` extracted). That covers monorepos like
  `applied_skills`. The SHA and a hash of the extracted files are recorded, and reinstalling
  checks both.
- Extracted into a fresh empty folder. Path checks: no absolute paths, no `..`, no links. Size caps.
  The manifest is validated, then `echo` / `api` are checked.
- A native dialog lists the permissions in words ("Reads your calls' transcripts, notes and chat ·
  Reads your Teams chats · Runs Claude Code on them with your AI profile, nightly and after each
  call · Opens a Claude Code terminal"). The grant is stored.
- Installed in `<config dir>/plugins/<id>/<sha>/` (code is per computer) and recorded in
  `<config dir>/plugins.json` (atomic, lock + re-read like `ai.json`). Its **store** is in the data
  folder, so the tree moves with the calls.
- Updates are checked with Echo's own (every 6 h). A version asking for more permissions waits for
  the user's approval. Disabled plugins keep their store.
- The window's **Plugins** view (below) does all of this without the CLI.

Trust: the digest or SHA proves which code was installed, not that it is good. Protection is the
sandbox (WASM, the page's origin), the guard on its Claude jobs, and the permission dialog. The
one unsandboxed surface is the terminal, which only the user opens. Later, signatures (GitHub
artifact attestations) can show "built by this repo's CI".

## Marketplace UI

### Catalogs

A **catalog** is a GitHub repo with `echo-plugins.json`, the same idea as Claude Code's
`marketplace.json`:

```json
{
  "name": "onetest",
  "plugins": [{
    "id": "tasks", "name": "Task tracker", "summary": "Epics, stories and tasks kept current from your calls and chats",
    "source": "onetest-ai/echo-tasks", "categories": ["productivity"],
    "icon": "icon.svg", "screenshots": ["shots/board.png", "shots/tree.png"]
  }]
}
```

`onetest-ai/echo-plugins` is built in. The user can add others (a team's own) by `owner/repo`
(`echoctl plugin catalog add|remove|list`). Each entry points to a plugin source. The **plugin's
own manifest** (fetched at the ref the catalog names) gives the version, the permissions and the
README, so a catalog can't promise fewer permissions than the plugin asks for. Plugins from the
built-in catalog get a "onetest" badge. Plugins from other catalogs show which catalog listed them.
Plugins installed by `owner/repo` show "Not in a catalog".

Fetching runs in the app, never in the page. Catalogs are refreshed at window open and every 6 h,
cached in `<config dir>/plugins/catalogs/`, and served by the async `echo://` handler as
`/api/plugins/catalog` (as `/api/calendar` is). Icons, screenshots and READMEs are downloaded by
the app, size-capped, served from `echo://` (the window's CSP allows no remote images). READMEs go
through `minutes.rs`'s escaping Markdown writer (raw HTML shown as text, https links only). No
catalog reachable → the cached copy, with a note saying when it was fetched.

### Where it lives

**Plugins** is a built-in activity at the bottom of the activity bar (above Settings), with a
badge when updates are waiting. Its layout is `single`.

Tray: Settings → Plugins… opens the window there. `echoctl app open --plugins [<id>]` does the same.

### The Plugins view

Three tabs over one list, a search box and category chips:

```
Plugins                                       [ Search plugins…        ]
[ Discover ] [ Installed 2 ] [ Updates 1 ]    All · Productivity · Export · Call apps · Developer

┌───────────────────────────────────────────────────────────────────────┐
│ [icon] Task tracker                     onetest       v0.3.0  [Install]│
│        Epics, stories and tasks kept current from your calls and chats │
│        Reads calls · Teams chats · Runs Claude Code                    │
├───────────────────────────────────────────────────────────────────────┤
│ [icon] Terminal                         built in      v1.0.0  [On ●]   │
│        Claude Code inside Echo — data folder and per call              │
├───────────────────────────────────────────────────────────────────────┤
│ [icon] Obsidian export                  team-catalog  v1.1.0  [Update] │
│        Minutes and actions into your vault        +1 permission        │
└───────────────────────────────────────────────────────────────────────┘
                                 Install from GitHub: [ owner/repo[/path][@ref] ] [Look up]
```

- Each row shows the plugin's permissions in short form, so they are visible before anyone clicks.
- **Installed** shows each plugin's on/off switch, version, catalog, last error (a job that failed,
  a paused plugin) and storage size.
- **Updates** lists newer versions with their release notes. A version that asks for more
  permissions says so on the row ("+1 permission") and can only be updated from its detail page.
  "Update all" skips those.

### Detail page

Clicking a row opens the plugin's page in the same view (`#plugins/<id>`):

```
← Plugins
[icon]  Task tracker                                         [ Install ]
        onetest-ai/echo-tasks · v0.3.0 · commit 4f2a9c1 · listed by onetest
[ Overview ] [ Permissions ] [ Versions ] [ Settings ]*  [ Data ]*      * installed only

Overview:     screenshots (click to enlarge), the README
Permissions:  every permission in words, grouped:
                What it reads   — your calls' transcripts, notes, action items and chat;
                                  your Teams chats (Microsoft 365, read-only)
                What it runs    — Claude Code with your default AI profile, after each call's
                                  minutes and every day at 18:30
                What it adds    — "Tasks" in the sidebar and the tray, a Tasks tab on each call,
                                  a terminal to set it up
                Network         — none
Versions:     tags with dates and notes; install a specific one (pinned: no auto-update)
Settings:     the manifest's settings form, the AI profile for its jobs, schedules on/off
Data:         where its store is, its size, Reveal, "Remove with its data" vs "Remove, keep data"
```

### Install flow

1. **Install** → a review sheet over the page: the Permissions tab's text, the source and the commit,
   and for plugins outside the built-in catalog a line saying who listed it and that Echo hasn't
   reviewed it. Buttons: **Install** (default) / Cancel.
2. Progress in the row: Downloading → Checking (digest / SHA) → Installing. On failure, the reason,
   in words ("the release has no checksum", "needs Echo 0.4 or later", "the manifest asks for an
   unknown permission").
3. Done → a toast "Task tracker installed", its nav item appears, and if it has a setup terminal,
   a **Set it up** button opens it.
4. Updates with more permissions go through the same review sheet, showing only what is new.

The page only asks: every step runs in the app over IPC (`plugin_install {source, ref}`,
`plugin_update`, `plugin_toggle`, `plugin_remove {keep_data}`), and the answer refreshes the
view. The CLI's `plugin install` asks the same question with a native dialog (`app::yes_no`). A
plugin page can't open the marketplace review sheet or install anything.

### States worth designing

- No catalog reachable and nothing cached: Discover shows "Can't reach GitHub", the
  install-by-source field still works if GitHub is reachable later.
- A plugin needs a newer Echo: its row says so, and the Install button is replaced by **Update Echo…**
  when an Echo update is available.
- A plugin is paused after repeated failures: an amber row in Installed, with the last error and
  **Resume**.
- A recording in progress: installing and updating still work. Enabling or disabling a plugin that
  has a job running on the current call waits until that job ends.

## Phases

| Phase | Scope | Proves |
|---|---|---|
| 0 | Spikes: a PTY + xterm.js panel in the window (macOS, then a Windows check); wasmtime component "hello" called from `echoctl plugin call`; a second origin for pages under wry on both platforms | The three risky pieces |
| 1 | Workbench layout (activity bar, right / bottom panels, status bar, view registry with the built-ins on it, `window-layout.json`), then plugin loader + manifest + contributions + pages + **Terminal** (bundled) | Layout, contributions and isolation, no GitHub yet |
| 2 | GitHub install (release asset and repo path @ SHA), `plugins.json`, catalogs, the **Plugins view** (Discover / Installed / Updates, detail page, review sheet) | Distribution and the marketplace |
| 3 | WASM host API + store + events (`minutes_written` etc.) + plugin CLI | Plugin code |
| 4 | Plugin Claude jobs: the minutes run generalised, guard policy per job, scheduler | Judgment steps |
| 5 | **Task tracker** port: core in Rust → WASM, skills, workbench page; sources = Echo calls + Teams chats | The whole model on a real plugin |
| later | Files / pull-request sources (`fs` / `net`), call apps as data, signatures | — |

## Decisions from the design fight (rounds 1–3, 2026-10-29)

Three rounds, each one a mockup followed by an independent critic. Where the text above disagrees,
this section wins. The mockups are on the Echo Workbench canvas (10 screens, drawn at 1180×760,
Echo's default window size, and at 800 px).

**Core flow first**
- Record / Stop is a **pill at the top centre of the window**, the same on every screen. On macOS it
  sits in the transparent title bar (full-size content view), so it takes no space from the views,
  and the window drags from either side of it. On Windows it sits in the same strip under a custom
  title bar. (Round 3 put it at the top of the activity bar; the user rejected that position.)
  - Idle: "● Record next call | English · Work ▾".
  - In a Teams call: "Record this call".
  - Armed: time left + Cancel.
  - Meeting soon: "15:00 Acme QBR in 3 min · Record it".
  - Recording: red dot, elapsed time, the call's title, mic state (follows Teams' mute), ■ Stop.
    Stop asks first; ⇧⌘R / Ctrl+Shift+R.
  - After the call: "Transcript ready · minutes at 13:05 · Add your notes", back to idle after 2 min.
  - In an 800 px window the title drops and only the clock and Stop remain.
  
  The ▾ menu holds what to record (the next call, join a meeting, a meeting when it starts), the
  language (English / Detect / Only …) and the AI profile with its sign-in state. Recording never
  waits for a sign-in. Plugins never get a state here.
- Recording state shows once, in the pill (plus the call list's dot). The status bar keeps only mic
  sync and the profile.
- The activity bar shows labels under its icons. Badges appear only for counts that need the user.

**Review inbox (core) and moments**
- **Review** is a built-in activity. Plugins (and Claude jobs) never change anything directly; they
  *propose* items there: a diff, the source plugin, the verifier's verdict, and one or more
  **moments**.
  - A moment is a `call@mm:ss` anchor, and the host renders it the same way everywhere: the quote
    ±10 s, the speaker, and a jump to the transcript.
  - Keys: j/k to move, y to accept, n to reject, e to edit.
  - An accepted item is pinned: later evidence never overwrites it.
  - Anything that would leave Echo over the network goes through Review first and is marked
    "leaves Echo → host". That makes Review the one trust gate for plugins that send data.
  - Its count is the only plugin-driven badge.
- Host API additions: `host.propose({title, diff, moments, verdict})` and `host.cite(call, t)` →
  a moment. This replaces plugins' own accept/reject buttons (the Tasks panel lost its own).
- Live-notes actions have no ids and are rewritten every 90 s. "Track in Tasks" on one of them is
  queued and resolved after the minutes give it an `actions.json` id. The chip says "Tasks: after
  minutes".

**Claude sessions (the terminal)**
- Exactly two scopes. There are no "new terminal" instances.
  - **Claude · <call name>**, guarded. It runs under the minutes run's guard (`guard.rs`): a
    `PreToolUse` hook with `onFailure: block` and `ECHOCTL_RUN_CALL` set to the call.
    - Read, Glob and Grep are allowed inside the call folder and in earlier minutes of the same
      meeting, without prompting.
    - Edit and Write are denied.
    - Bash is limited to a read-only subset of `RUN_COMMANDS`.
    - The call's pinned profile is used.
    - A banner says so. During a recording it says "live transcript so far".
  - **Claude · Echo folder**: the user's own session and modes.
- The call session is pinned to its call. Selecting another call shows a strip: "On Northwind… ·
  Open on Pricing review". There is one call session at a time; the previous one is suspended and
  can be resumed. The guard resolves the call by folder stamp, so the minutes run's rename doesn't
  break it.
- Every session starts with `--session-id <uuid>` and is resumed with `--resume <uuid>`, never
  `--continue`. The ids are kept in `<home>/.echo/sessions.json`, keyed by call stamp.
  Processes always start in the data folder (on Windows a working folder blocks renames).
- Plugins never drive the folder session. A plugin action only pre-fills its prompt and never
  submits it. Plugin skills load only into guarded sessions. A plugin's setup session runs with
  `--permission-mode default`, opt-in, unchecked by default.
- Keys: while Claude has focus it gets every key except ⌘Esc / Ctrl+Esc. The palette is ⌘K on
  macOS and Ctrl+Shift+P on Windows; Ctrl+K and Ctrl+J are Claude Code's own. Claude toggles with
  ⌘` / Ctrl+`.

**Layout**
- **Phase layouts:** Before (prep), During (from `recording_started`) and After (from `transcribed`)
  each keep their own placement of views and their own window mode. During is narrow, beside Teams.
  "Move to …" in a view's tab menu changes the current phase only. Settings → Layout shows a
  view × phase table. The layout is per computer; Claude session ids and plugin stores are per data
  folder.
- The right panel is closed by default. Below 900 px it opens as an overlay over the page. Below
  600 px of height the bottom panel collapses to a strip. In a narrow window the call list becomes
  a "Calls · Today ▾" dropdown.
- Plugin call tabs stay (a requirement) but sit behind "More ▾". Status items are off by default.
- **⌘K palette:** plugins put their actions here instead of buttons everywhere.
  - A target chip ("On: <call> ▾") can be switched to a call, an event, or nothing (global
    actions only).
  - Unavailable actions show greyed, with the reason.
  - Jumps to calls and people are in the same list, so there is no second search.
  - The calendar popover shows at most two plugin actions; the rest are in ⌘K.
- Calendar actions get the event's id, never its text (organisers are strangers). The built-in
  deterministic "Prep from last time" (`echoctl prep`) comes first.

**Plugins and the marketplace**
- Origins: a fixed pool of protocol slots (`p0`…`p15`) registered when the webview is created.
  Plugins are assigned to slots, so the webview is never recreated (that would kill the terminal,
  the notes editor and frames). A slot's storage is cleared before it is reused. To spike: whether
  WebView2's `WebResourceRequested` fires for cross-origin iframe requests.
- `echoctl plugin call` is forwarded to the running app, which owns the single WASM instance and
  enforces the run's `ECHOCTL_RUN_CALL` scope. With no app running, it uses a `FileLock` on the
  store.
- Profiles: each call is processed with its own pinned profile. Plugin stores are partitioned by
  profile, so evidence from a call on one profile never reaches a job on another. A job with no
  call (chats) runs with a profile the plugin's settings name, shown at install.
- Permissions:
  - Data plus network is shown as an orange warning line ("Can send your calls' action items to
    api.linear.app").
  - There is an `fs` permission: a folder the user picks, write-only.
  - Call apps (Zoom) are core, not plugins: they decide what gets recorded.
- Install sheet:
  - "Its setup runs in a Claude session you open, with your permissions."
  - Cancel is the default button outside the built-in catalog.
  - Updates install automatically only within the built-in catalog and only without new
    permissions. "What changed" lists the commits and the changed skills and prompts.
- Uninstall:
  - stops the plugin's jobs;
  - offers to keep or delete its store;
  - lists what disappears (chips, tabs, panels, actions);
  - removes its layout entries and clears its slot;
  - restarts guarded sessions that had loaded its skills.
- Notifications: `host.notify` is prefixed with the plugin's name, rate-limited, can be muted per
  plugin, and is silent while recording. A plugin paused after failures shows an amber row and one
  status item.
- Performance:
  - Live frames: only the visible ones plus one per area.
  - Terminal output is kept in a 2 MB ring buffer in Rust and pushed only while the panel is
    visible, coalesced (about 16 ms / 64 KB) through a streamed `echo://pty/<id>?from=<seq>`, not
    through `evaluate_script`.
  - To profile: a 1 h call + 3 frames + the terminal on a CPU-only Windows machine with live
    transcription running.

**Action items vs tasks**

An action item is *what was said in one call*. A task is *the work tracked across calls*. A tracked
action item is evidence for a task; it doesn't turn into one.

| | Live action | Action item | Task |
|---|---|---|---|
| Owner | Echo, live notes (`live-notes.json`) | Echo core, written by the minutes run (`actions.json`) | The Tasks plugin's store |
| When | During the call, rewritten every 90 s | After the minutes | Across calls and chats, for weeks |
| Identity | None (a draft) | Stable id, `t`, owner, todo/done | Its own id, in Epic → Story → Task |
| Holds | Text | One commitment from one call | Status, due date, acceptance criteria, dependencies, waiting on, history |
| Without the plugin | Unchanged | Unchanged | — |

Links run many-to-one: the same commitment said in three calls is three action items and one task.
- Many action items never become tasks (small one-offs).
- Some tasks have no action item: they come from the discussion or from chats.
- An item someone else owns becomes **waiting on <person>** on a task, or an **ask** for the next
  meeting with them, not a task of the user's.

The flow:
1. **During the call.** Live actions are drafts. "Track in Tasks" (the item's menu or ⌘K) only
   queues the intent. The chip says "Tasks: after minutes".
2. **Minutes.** The minutes run writes `actions.json` with ids. Echo's own job ends here, with or
   without plugins.
3. **The Tasks job** runs on `minutes_written`. It reads the new action items, the transcript and
   the chat, and proposes one of these in **Review** for each item, with its moment:
   - **New task:** the user owns it, and there is a deliverable or a date.
   - **Same as task X:** a link, and maybe a moved due date.
   - **Waiting on / ask:** someone else owns it.
   - Nothing, for items too small to track: they stay only action items.
   
   It also proposes changes that came from the discussion: status ("it's live in staging" → done?),
   acceptance criteria, slipped dates. Its verifier checks each proposal against the quote first.
   Queued intents from step 1 are proposed even if the model wouldn't have picked them.
4. **Accept** (y / n / e). The plugin stores the link `{call stamp, action id} → task` in its store.
   The action item shows the chip "✓ in Tasks · <task>" (`host.annotate`).
5. **Later calls** add evidence the same way. A repeated commitment links to the existing task
   instead of creating a duplicate. Items no call has touched for N days are flagged as quiet, and
   open asks feed "Prep from last time".

**Done status: shown, never written (decided 2026-10-29).**
- The user ticks an action item done: Echo writes `actions.json` as today and emits
  `actions_changed`. The plugin marks the linked task done at once, without Review: it was the
  user's own act.
- The user marks a task done in the plugin's page: the plugin records it in its store and annotates
  the linked action items. Echo shows them as **"done · via Tasks"**, derived from the annotation.
  `actions.json` is not touched: plugins never write call files, with no exceptions.
- Echo's own readers (the "last time" recap, `context`, open items in prep) take the annotation
  into account when one exists, so a task closed in the plugin doesn't come back as an open item.
  Without the plugin they read `actions.json` alone, as today.
- A change a model infers ("Ravi said he got it") is always a proposal in Review, never applied
  directly.

Other rules:
- The minutes never read plugin state. Action items stay the record of what was said.
- The plugin never changes an action item's text, owner or date. If it thinks the minutes got one
  wrong, that is a note on the task.
- Unlinking is possible from either side and deletes nothing.
- Uninstalling the plugin removes its links, chips and "done · via Tasks" annotations. Every call's
  `actions.json` is exactly as it was.

**The Tasks plugin's own view**
- Its page shows:
  - Tree / Board / Timeline, with status, due dates and acceptance-criteria progress;
  - a source filter (calls / chats);
  - items that have gone quiet ("quiet 16 days");
  - asks by person, which feed "Prep from last time";
  - the last run with its log.
- Its proposals live in Review, not in its own page.

## Open questions

1. Terminal: only `claude`, or a plain shell too? A shell in Echo's window is the user's own, but it
   widens what "Echo" does.
2. Should the Terminal plugin be bundled and on by default, or something the user installs?
3. Tracker scope: one project per data folder, or several trees (one per client / engagement)?
4. Tracker jobs: the default profile, or a profile the plugin settings choose? Calls already pin
   their own profile, but a nightly run spans many calls.
5. Who else will write plugins: only us (a curated index is enough), or third parties soon
   (signatures early)?
