# Echo plugins — design

Echo plugins are packages that extend Echo itself with their own code: a host half that runs inside the app (QuickJS)
and a client half that runs in the Echo window's page. They are not Claude Code plugins (see
`.ai-run/guides/design/claude-code.md`). Echo keeps only primitives; a plugin's features live in its package.

Related: rules in `.ai-run/guides/plugins/echo-plugins.md` (the host, capabilities, grants) and
`.ai-run/guides/plugins/plugin-authoring.md` (manifest, the `ctx` API of both halves, publishing). The full plan,
its history and the first three plugins: `docs/plugins-design.md` (v4 is current; where it names `plugins/host.rs`,
the code is `jshost.rs`). Obsidian's plan: `docs/obsidian-design.md`.

## Modules

| File | Role |
|---|---|
| `crates/echoctl/src/plugins.rs` | Manifest, capabilities and the permission sheet, marketplaces (fetch, check, catalog), install / update / remove, settings and secrets, Settings → Plugins ops |
| `crates/echoctl/src/jshost.rs` | Host half: one QuickJS thread, one context per enabled plugin, `PRELUDE` (the `ctx` object), services, commands, timers, events, channels, delivery providers (`js` feature) |
| `crates/echoctl/src/claudejob.rs` | `ctx.claude.run`: a headless Claude Code run on one of the user's AI profiles |
| `crates/echoctl/src/grants.rs` | What a plugin may reach beyond its store: picked folders, session skills / reads, `<echo_view>` hints |
| `crates/echoctl/src/share.rs` | Echo's data for plugins (`ctx.echo.read`, `ctx.echo.dictionary.add`) and the events it sends |
| `crates/echoctl/src/deliver.rs` | Sending to the user: Teams, mail, and plugins' ways |
| `crates/echoctl/ui/app/33-plugins-client.js` (`loadPluginClients`, `pluginCtx`, `unloadPlugin`, `SLOTS`) | Client half: imported from `/plugin/<id>/<file>` (`window::plugin_file`), its contributions registered and torn down by plugin id |

## How it works

### Package and trust
- A package is `echo-plugin.json` + `main` (host half) + `client` (client half) + any files (skills, templates).
- **Install = trust its code.** `plugins::sheet` puts "Runs its own code in Echo / the window, with your rights" first,
  then one line per `uses` entry, in Echo's words (never the plugin's). The user agrees to every line; an update that
  asks for more waits for a new agreement.
- A `uses` kind this Echo doesn't know is named on the sheet and granted nothing.

### Host half
- Loaded at app start and after install / switch / remove (`jshost::sync`); its `apply(ctx)` gets Echo's services over
  two Rust functions (`__host` sync, `__hostAsync` → a Promise settled from a worker thread).
- Commands are reached from the window (`GET /api/plugin/<id>/call/<cmd>`), Inbox buttons (IPC `plugin_call`) and
  `echoctl plugin call`, which loads the host halves without timers (`jshost::no_timers`).
- Limits: 64 MB for the runtime, 5 s of JavaScript per slice (`jshost::SLICE`); bulk reads go through async services.
- `ctx.claude.run` (`claudejob.rs`) runs on the user's AI profiles only, with the Brain narrowed to that profile.
  `tools: read` = the guard's routine mode (data folder read-only, `guard::ROUTINE_COMMANDS`; the CLI's
  `routine_reads` lets it read projects, clients, orgs and a ledger tree); `none`, `folder`, `skip` as named.
  Usage comes from `projectsync::finish_run_result`.

### Client half
- Each enabled plugin's `client` is imported as a module into the window's own page (trusted). Everything it adds —
  slots, panels, activities, Settings sections, palette entries, listeners, styles — is registered under its id, so
  `unloadPlugin` takes it all out on switch-off, removal or update.

### Marketplaces
- A git repository (or folder) with `echo-marketplace.json`. Ours, **`onetest-ai/echo-plugins`**, is built in, hidden
  and never removed (`plugins::built_in_source`; `ECHOCTL_PLUGIN_MARKET` points it elsewhere). Users add any number.
- `plugins::refresh` fetches (never the page): GitHub `owner/repo[@ref]` → its commit → tarball via `tar`; a git URL
  → `git clone --depth 1`; a folder read in place. Re-read when the Plugins page asks and the last read is 30 min old
  or by another Echo version (`Catalog.echo_version`). Every tree goes through `check_tree`.

### What a plugin reaches beyond its store
- **Echo's data** (`uses.echo`, `share.rs`): read kinds `clients`, `threads`, `people`, `dictionary`, `sources`,
  `source`; a source carries the AI `profile` its text may go to. Write: only adding to the dictionary
  (`taxonomy::dictionary_add`, merges, never renames or removes).
- **Echo's events** (`uses.events.on`, #59): `plugins::EVENTS` — a meeting soon, a Teams call starting / ending, a
  recording starting / stopping, a transcript ready, minutes written, a thread synced, sources changed — each with
  the sheet's words ("Knows when …"). Only the plugins that named one hear it (`jshost` `Plugin.hears`); `ctx.on` for
  one not named is refused. Payloads are ids and titles (a call's folder name, a meeting's id and subject, a
  thread's id; `watch::for_plugins`), never the words — `echo.read` reads those; the sheet's words name the text that
  comes (a subject, a title). A plugin reading sources (`echo.read` source / sources) hears `minutes_written` /
  `sources_changed` without naming them (they came with it before `events` existed); others no longer hear them.
  One grant per loaded plugin (`jshost::hears`, from the manifest it loaded with): `ctx.on` checks it, delivery
  filters by it. From the app only (a CLI process has no host), as they happen, no replay; no `brain_synced`
  (`sources_changed` says a Recall sync changed something).
- **Folders** the user picked in Echo's own picker (`grants.rs`), read-only, links not followed.
- **Echo's sessions** (`uses.sessions`): skills as text — Echo writes the Claude Code plugin itself
  (`<config dir>/plugins/session/<id>/`, `echo-<id>`; `grants::set_session_skills` keeps only name and description,
  so no hooks / MCP / agents ride along) — given to the Echo-folder session and `claudejob` runs via `--plugin-dir`,
  never the user's own Claude Code; granted folders via `--add-dir`; hints as one plain line in `<echo_view>` when
  that client or thread is open (`grants::hints_for`).
- **Channels** (`uses.channel`, `jshost::channels`): the Echo-folder session starts with `--channels plugin:<spec>`
  and the env given, minus any `CLAUDE*` / `ANTHROPIC*` keys.
- All of it only for enabled plugins; a reload drops what the old version set (`grants::drop_plugin`); removal
  deletes its grants, hints and session plugin.

### Delivery (`deliver.rs`)
- Only to the user (plus addresses the user set on a routine, `mail_to`); never an address a model picked.
- Echo's own ways: **Teams** = `POST /chats/48:notes/messages` (the chat with yourself); **mail** = `POST /me/sendMail`
  to `/me`'s own address, not saved in Sent (mail to others is kept in Sent, its foot says who sent it).
- A mail is Echo's own design (`mail_html`, `MAIL_CSS`): one plain card, Echo's ink for every text (Outlook recolours
  after the CSS), saturated fills only, report-style tables with zebra rows marked in the HTML; checked in Outlook on
  the web, light and dark.
- Anything a model wrote carries `ai_notice` in every channel.
- Other ways are plugins': `ctx.deliver.provide(kind, …)` (`jshost::providers`); `deliver::send` hands the message to
  that plugin (`jshost::provider_send` → its `__deliver` command). Settings → Sends to you (`#settings/deliver`,
  `/api/deliver`); `echoctl deliver status|test|send`.

### First-party plugins (in `onetest-ai/echo-plugins/plugins/`)
| Plugin | What it does |
|---|---|
| `scheduler` | Routines: the user's prompts on a schedule (`routines/<id>.md` in its store, templates in its package), runs with tokens and cost, its own Settings section |
| `telegram` | A channel into the Echo-folder session (`access.json` in its store: allowlist of the paired chat id, `TELEGRAM_ACCESS_MODE=static`) and a delivery provider (bot token as a secret, `api.telegram.org` only) |
| `obsidian` | The user's own vaults: survey → one no-tools Claude run → a description the user confirms → one `obsidian-vaults` skill for sessions; an activity browsing a vault as Obsidian does. Never indexed into the Brain |

## Decisions

| Decision | Why | Where |
|---|---|---|
| Plugins carry their own code; Echo keeps primitives | Echo is extended by plugins, not by growing features inside Echo | `jshost.rs`, `plugins.rs` |
| Install = trust; the sheet's words are Echo's | The client half runs in Echo's own page; the user must see what it touches in words a plugin can't rewrite | `plugins::sheet`, `plugins::CAPABILITIES` |
| Session skills written by Echo with name + description only, `!` commands refused | A plugin must not slip hooks, MCP servers or agents into Claude sessions | `grants::set_session_skills` |
| Plugin skills go only to Echo's sessions and runs, never the user's Claude Code | Keeps a plugin's reach to what Echo starts | `grants.rs`, `claudejob.rs` |
| Every plugin Claude run goes through the user's AI profiles | Strict profile routing: never a plugin's own key or account | `claudejob.rs` |
| Delivery only to the user (and routine-set addresses) | Nothing leaves by default; a model never picks a recipient | `deliver.rs` |
| Channel env stripped of `CLAUDE*` / `ANTHROPIC*` | A plugin must not reroute the session's model or account | `jshost::channels` |
| Everything registered under the plugin id and removed on unload | Else an update doubles every listener | `ui/app/33-plugins-client.js::unloadPlugin`, `grants::drop_plugin` |
| Obsidian vaults never enter the Brain | Vault and Brain stay apart (the user's call) | the `obsidian` package |

## Files and state

| Path | What | Who writes |
|---|---|---|
| `<config dir>/plugins.json` | Marketplaces added, plugins installed | `plugins.rs` |
| `<config dir>/plugins/markets/<key>/` | Fetched marketplace trees | `plugins::refresh` |
| `<config dir>/plugins/installed/<id>/<version>/` | Installed package files | `plugins::install` |
| `<config dir>/plugins/secrets/<id>.json` | Secret settings (0600) | `plugins.rs` |
| `<config dir>/plugins/grants/<id>.json` | Folders the user picked | `grants.rs` |
| `<config dir>/plugins/hints/<id>.json` | `<echo_view>` hints | `grants.rs` |
| `<config dir>/plugins/session/<id>/` | The Claude Code plugin Echo writes for its sessions | `grants.rs` |
| `<home>/.echo/plugins/<id>/` | The plugin's store, `settings.json`, `inbox.json` (per data folder) | the plugin via `ctx.store`; Echo for settings / inbox |

## Known gaps
- `brain_source` and `export` capabilities are known but not implemented: plugins that ask for them can't install.
- Telegram: messages sent while the session was closed aren't read. A Claude Code started inside another
  (`CLAUDE_CODE_CHILD_SESSION` inherited) keeps no transcript, so it can't be resumed.
- The `js` feature (QuickJS, C) is off in the Windows cross-check, so host-half code isn't compiled there.
