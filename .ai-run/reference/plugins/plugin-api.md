# Echo Plugin API

**Reference**, not a guide: the facts of Echo's plugin API — the manifest, `uses`, the host half's and the client half's
`ctx`, events, widgets, scripts, Claude's part, publishing — as the code has them. It changes in the same PR as the code
it describes. The rules for using it are in `.ai-run/guides/plugins/plugin-authoring.md`; `echoctl plugin guide` prints
both, this after the rules. Where this and Echo's code differ, the code wins: fix this in that code's PR.

## Package and manifest

- A plugin is one folder with `echo-plugin.json` at its root: required `id` (1–40 chars, a–z 0–9 `-`, no leading `-`),
  `name` (≤ 60) and `version` (`X.Y.Z[-pre]`); optional `echo` (`>=X.Y.Z`, the Echo it needs), `summary` (≤ 240),
  `uses`, `settings`, `main`, `client`, `widgets`, `skills`, `agent` and `icon` (an `.svg` in the plugin folder, named
  with a–z 0–9 `.` `_` `-` `/` only, ≤ 16 KB, shown only as an image beside its work rows and ⌘K entries).
  Evidence: `plugins::read_manifest`, `plugins::valid_id`
- `main` and `client` point at a `.js`/`.mjs` inside the plugin folder; any other file it ships is read with
  `ctx.package.read(path)` / `ctx.package.list(dir)`. Evidence: `jshost::package_file`
- A setting is `{key, label, type, default?, help?}`: `key` unique (a–z 0–9 `-` `_`), `type` `string` (default), `bool`,
  `folder` or `secret`; read with `ctx.settings.get(key)`. Evidence: `plugins::set_setting`
- A plugin and a marketplace tree hold plain files only (no links or special files): 400 files / 20 MB a plugin,
  5000 / 64 MB a marketplace. Evidence: `plugins::check_tree`
- `echoctl plugin check <folder>` runs an install's checks of the package (manifest, `uses`, widgets, skills, scripts,
  `agent/`): exit 0, or each problem named by file; `--json`. Needs no data folder, nothing installed, no network.
  Evidence: `plugins::agent::check_folder`

## `uses` and the permission sheet

Each `uses` key is a line on the permission sheet, in Echo's words; the user agrees to the sheet on install, and an
update adding a line asks again (`echoctl plugin update --yes`). Only these keys are known; `brain_source` and `export`
are not granted. Evidence: `plugins::CAPABILITIES`, `plugins::describe_folders`, `plugins::sheet`

| Key | Shape and use |
|---|---|
| `claude` | runs Claude (§ Claude runs); the sheet: "Runs Claude Code on your AI profiles, as Claude in Echo works for you — acting on its own or preparing and asking, as you set it in Settings"; `{"skip": true}` is accepted and changes nothing |
| `schedule` | works on its own on a schedule while Echo runs |
| `events.on` | the events it hears (§ Events) |
| `deliver.to` | where it sends to the user |
| `deliver_provider.kinds` | the delivery kinds it provides (§ Delivery) |
| `network.hosts`, `network.hosts_from` | the `https://` hosts it fetches (§ Network); string settings without a default whose addresses the user lists |
| `channel.plugin` | the Claude Code plugin it installs (§ Claude runs) |
| `sessions`, `sessions.commands` | its runtime skills for Echo's sessions; its commands Claude in Echo may run (§ Claude in Echo) |
| `folders.what`, `folders.write` | folders the user picks, read-only, or changed with `folders.write` (§ Folders and files) |
| `files.read` | other apps' files, each named in full, read with `ctx.system.readFile(entry)` |
| `scripts` | the scripts of its package skills Echo may run (§ Scripts) |
| `echo.read`, `echo.write` | Echo's data it reads (§ Echo's data); `["dictionary"]` to add to the dictionary |
| `signin.hosts` | 1–4 plain host names, each once, no IP addresses: the hosts whose sign-in saved on this computer it gets (§ Sign-in) |
| `signin.keep` | `true`: Echo keeps a sign-in it makes itself for those hosts, its own (§ Sign-in) |
| `work` | `{}`: its items on My work and in threads' Work (§ Work rows) |

The sheet also names each script, says it runs on this computer with the user's rights and can reach the network, its
time when over 300 s, and that Echo can't check a `readonly` one only reads; an update that adds a script or changes any
file of a skill folder that has one asks again. Evidence: `plugins::scripts::describe_scripts`, `plugins::scripts::check`

## Host half

- The host half exports `apply(ctx)` and registers its commands, timers, event handlers and delivery ways inside it.
  Evidence: `jshost::load`
- It is one file with no `import` of other files (the client half may import its own modules).
  Evidence: `window::plugins::plugin_file`
- `ctx.command(name, fn)`: `fn` takes JSON arguments, may be `async`, returns JSON and throws an `Error` to fail. A
  window-called command has 30 s. More than 5 s of JavaScript between awaits is interrupted; bulk file reads go through
  `ctx.fs.readMany` / `ctx.fs.walk`, which run off that slice. Evidence: `jshost::CALL_TIMEOUT`, `jshost::SLICE`
- `ctx.every(seconds, fn)`: periodic work, 10 s minimum; there are no timers in `echoctl plugin call` or
  `echoctl deliver`. Evidence: `jshost::no_timers`
- Memory: 64 MB shared by all host halves. Evidence: `jshost::set_memory_limit`
- `ctx.sessions.skills`, `ctx.sessions.read` and `ctx.view.hints` are each set as the whole set (`[]` removes it) and
  are not kept across loads: set them from `apply` on every load. Evidence: `grants::drop_plugin`
- `ctx.inbox.set(key, {title, why, urgent, go, actions: [{label, command, args, primary}]})` (key ≤ 80 chars) asks the
  user for a decision; `ctx.inbox.clear(key)` takes it away. Evidence: `attention::inbox_items`
- `ctx.markdown(md)` renders Markdown to safe HTML in the host half. Evidence: `jshost::PRELUDE`
- In a terminal (`ctx.terminal`) only this plugin is loaded. Evidence: `plugins::session_commands`

## Events

`ctx.on(event, fn)` for an event named in `uses.events.on`. The events (`plugins::EVENTS`): `meeting_soon`,
`call_started`, `call_ended`, `recording_started`, `recording_stopped`, `transcribed`, `minutes_written`,
`thread_synced`, `sources_changed`, `threads_changed`, `shared_changed`.

| Event | Payload |
|---|---|
| `meeting_soon` | `{id, subject, start, threads}` (`threads`: the ids of the threads Echo puts the meeting in, `[]` when none) |
| `threads_changed` | `{thread, born, change: 'renamed', name, from}`, `{thread, born, change: 'merged', into, dropped}` (each key `into` lacks is moved to it; `dropped`: yours that didn't fit in its 64) or `{thread, born, change: 'deleted', values}` (your own values, removed); `born` tells this thread from a later one with its id |
| `shared_changed` | `{namespace, key, thread?, by}`: a shared value changed (not the same value again), told to the namespace's other plugins, never to the one that set it; it carries no value |

A call's words are not in a payload: read them with `ctx.echo.read('source', id)`. Events are not replayed — except
`threads_changed` and `shared_changed` from another process (`echoctl project delete`, a terminal's `plugin call`),
which wait for the running app (heard within about a second, or at its next start), at least once; a queued `renamed`
or `deleted` of an id that by then belongs to a new thread is dropped.
Evidence: `plugins::EVENTS`, `plugins::thread_values::take_queued`, `plugins::shared::tell_changed`

## Store

- `ctx.store.read/write/list/remove/path`, by relative path only; a name at the store's top level is plain ASCII
  (letters, digits, `-` `_` `.`). Evidence: `plugins::data_dir`, `jshost::not_echos`
- Echo keeps its own files in the store and refuses a plugin's `ctx.store.write` of `settings.json`, `inbox.json`,
  `widgets.json`, `threads.json` or `work.json`, and its `ctx.store.remove` of `settings.json`, `threads.json`,
  `work.json` or the store folder itself: those change through `ctx.settings.set`, `ctx.inbox.set` / `clear`,
  `ctx.widgets.set` / `clear` and `ctx.threads.set`; `work.json` is what `ctx.work.provide` answered.
  Evidence: `jshost::not_echos`, `jshost::inbox_file`, `jshost::widgets_file`
- `ctx.http.fetch(url, {save: "<store path>"})` saves audio or a picture, shown from `/plugin/<id>/store/<path>` (media
  kinds only). Echo removes what lies in `cache/` over 7 days; the rest is the plugin's to remove with `ctx.store.remove`.
  Evidence: `jshost::sweep_store`, `window::plugins::store_file`

## Network

`ctx.http.fetch(url, {method, headers, body, save?})` resolves `{status, body}` for any HTTP answer, a redirect too
(never followed). It waits 60 s at most, downloads 10 MB at most, and rejects as below. Plain http is allowed only to
an address the user listed, inside their own network; only the user enters those addresses (window or CLI), never the
plugin (`ctx.settings.set`, its `settings.json`, a manifest `default`). Evidence: `jshost::fetch`, `jshost::curl_head`,
`plugins::addresses`, `plugins::listed_addresses`

| Rejected | When |
|---|---|
| the host | not granted: neither `https://` to a host in `network.hosts` (named with no port and no user) nor an address the user listed (`network.hosts_from`) |
| the method | other than `GET`, `POST`, `PUT`, `PATCH` or `DELETE` |
| a header | a name that isn't a plain token, a value with a NUL or a line break (a NUL in the URL or body too) |
| a write | a method other than `GET`, or `save`, in a routine's or Yolo's run preparing and asking |
| the request | the host can't be reached, the 60 s or the 10 MB passed |

## Claude runs

- `await ctx.claude.run({prompt, title, profile, model, tools, dir, timeout})`, only with `claude` in `uses`; refused
  in a routine's or Yolo's run preparing and asking. It runs as the folder session, by the user's one choice (acting on
  its own or preparing and asking, `untrusted-text.md` R20–R22); only `tools: "none"` changes that (no tools: what it
  needs goes in the prompt); any other value is accepted, kept in the run's record and changes nothing, as
  `"claude": {"skip": true}` does. Its result lists the changes and sends it made (`actions`).
  Evidence: `claudejob::Tools::parse`, `plugins::may_run_claude`
- `ctx.claude.installPlugin(spec)` installs the channel plugin the manifest names (`uses.channel.plugin`) into the
  default AI profile's Claude Code; `ctx.claude.hasPlugin(spec)` asks whether it is there; both take only that spec; an
  install in a routine's or Yolo's run preparing and asking is refused. Evidence: `capabilities::describe_channel`

## Delivery

`ctx.deliver.provide(kind, {label, send(msg)})` for a kind in `uses.deliver_provider.kinds` (never `teams` or `mail`);
`ctx.deliver.ready(kind, true)` once it can send. Evidence: `jshost::provider_ready`, `plugins::may_send`

## Echo's data

`ctx.echo.read(kind, arg)` for kinds in `uses.echo.read`: `clients`, `threads`, `people`, `dictionary`, `sources`,
`source` (`calls` is named on the sheet but not served yet). `ctx.echo.dictionary.add(...)` under
`uses.echo.write: ["dictionary"]`. Evidence: `plugins::ECHO_READS`, `share::dictionary_add`

## Folders and files

`await ctx.system.pickFolder(prompt)` (needs `uses.folders`) gives a folder, read as `(root, relative path)`, in bulk
through `ctx.fs.readMany/walk`; with `folders.write`, `ctx.fs.write/move/remove` change it (never through a link,
never over another file). In a debug build `ECHOCTL_PICK_FOLDER=<dir>` answers the picker. Other apps' files: as
named in `uses.files.read`, with `ctx.system.readFile(entry)`. Evidence: `app::choose_folder`, `plugins::app_file`

## Thread values and shared values

- `ctx.threads.set(thread, key, value)` — `key` 1–80 chars, `value` JSON of 16 KB at most, 64 keys a thread, 4 MB a
  plugin in all; `null` removes it; refused in a routine's or Yolo's run preparing and asking and for a thread that
  doesn't exist. `ctx.threads.get(thread, key)` (null when none), `ctx.threads.list(key)` (`[{thread, value}]`,
  threads gone left out). No grant needed. A `threads.json` that isn't JSON makes `get` and `list` fail and is moved
  aside as `threads.json.bad-<time>` at the next change; a thread id used again inherits nothing. A thread's changes
  arrive as `threads_changed` (§ Events). Evidence: `plugins::thread_values::changed`
- A key with a dot, `<ns>.<name>`, is shared with the other plugins of the marketplace: `ns` is the marketplace's
  declared `name` (`echo-marketplace.json`) in lower case, letters, digits and `-` (`Acme` → `acme.project_codes`) —
  per thread with `ctx.threads.get/set/list` (one value for every plugin; it follows the thread the same way; a
  `deleted` `threads_changed` gives each plugin of the namespace's source its removed shared values too), or one for
  the computer with `ctx.shared.get(key)` (null when none) and `ctx.shared.set(key, value)` (`null` removes it). A set
  creates it when it isn't there. Every plugin reads any namespace; only plugins from the marketplace source that owns
  `ns` set it — the source of the earliest installed plugin whose marketplace declares that name (another source with
  the same name only reads; when the owner's plugins are all removed, the next takes it); a namespace no marketplace
  owns reads null and refuses a set. Removing a plugin leaves the values. Limits as for thread values, counted per
  namespace, with 64 keys for the computer too. A change is told as `shared_changed` (§ Events).
  Evidence: `plugins::shared`, `plugins::shared::tell_changed`

## Sign-in

- `await ctx.signin.get(host)` for a host in `uses.signin.hosts` → `{token, from: "gh" | "git", user?}` (the GitHub
  CLI's token, else what git's credential helper keeps — for a self-hosted git maybe the user's own password) or
  `null`; refused in a routine's or Yolo's run preparing and asking. The sign-in grants no network: the API host goes
  in `network.hosts`. `ctx.signin.get(host, {fresh: true})` asks again (after a 401). `ctx.signin.sources(host)` →
  `{gh, git}` says which programs are here, no sign-in read. Evidence: `plugins::signin::get`, `plugins::may_use_signin`
- With `uses.signin.keep: true`, `ctx.signin.keep(host, token)` (one line, 8 KB at most) has Echo keep a sign-in the
  plugin made itself (its own client id and scopes, a device code or browser sign-in); `ctx.signin.get(host)` hands it
  back first as `{token, from: "echo"}`, to that plugin only; `ctx.signin.forget(host)` signs out. It goes when the
  plugin is removed; the same id from another marketplace, or a version that no longer says `keep`, starts without it;
  keep and forget are refused in a routine's or Yolo's run preparing and asking. After a 401 on an `"echo"` sign-in,
  `{fresh: true}` gives the same one. Evidence: `plugins::signin::keep`

## Claude in Echo: skills, commands and `agent/`

- **Package skills**: a folder in the plugin (`skills/<name>/SKILL.md`, with `scripts/` and `references/` beside it as
  needed) named in `skills` (at most 10, with the runtime skills); the folder's name a–z 0–9 `-`, ≤ 40 chars, once;
  `SKILL.md`'s frontmatter only `name` (the folder's name) and `description` (one line, ≤ 1024 chars), no `!` commands
  anywhere. They need no `uses.sessions` (the sheet names them from `skills`). The folder session, routines and Yolo
  name each `<prefix>-<name>` through `.plugins`; a plugin not laid out keeps `echo-<id>:<name>` from its
  `--plugin-dir` copy. Evidence: `plugins::package_skills`, `grants::copy_package_skills`
- **Runtime skills**: text built at run time, `ctx.sessions.skills = [{name, description, body}]`; Echo writes the
  frontmatter and the plugin around them. One name is never both a package and a runtime skill.
  Evidence: `grants::set_session_skills`
- **Commands**: a skill's Echo CLI is `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`; Echo's sessions run only the plugin's
  commands listed in `uses.sessions.commands` (`plugin call <id> <command>`). Evidence: `plugins::session_commands`
- **`agent/`**: `"agent": {"summary": "…", "prefix": "…"}` in the manifest — `summary` one line, ≤ 200 chars, the row
  Echo's core gives the plugin (needs the `agent/` folder); `prefix` 2–4 chars `[a-z0-9]`, no `-`, required when it
  has skills (a plugin with skills and no guide declares only `"agent": {"prefix": …}`, and Echo writes its
  CLAUDE.md): skills are `<prefix>-<name>` (not doubled when the name starts with it); a prefix belongs to one
  marketplace source (its plugins may share it), `echo` is reserved, and a prefix another source holds or a laid-out
  name another enabled plugin uses is refused. Evidence: `plugins::agent::problems`

  | Path | Holds | Limits |
  |---|---|---|
  | `agent/CLAUDE.md` | required: what the plugin gives Claude, when to turn to it, its commands (`${CLAUDE_PLUGIN_ROOT}/scripts/echoctl plugin call <id> <cmd>`), skills and guides — a guide named as "Read guides/x.md", never `@`-imported | ≤ 8 KB |
  | `agent/skills/<name>/SKILL.md` | as package skills; `<prefix>-<name>` in the folder session, routines and Yolo | ≤ 20; a laid-out name ≤ 64 chars |
  | `agent/rules/*.md` | rules | ≤ 10, ≤ 4 KB each |
  | `agent/guides/**/*.md` | reference | ≤ 50, ≤ 64 KB each |

  ≤ 1 MB in all; no folder named `.claude` anywhere in it or in a skill folder (other dot files are left out);
  `${CLAUDE_PLUGIN_ROOT}` in them is the folder Echo lays them out in (`echo-host.md` § Layout). Claude reads them only
  when it works with the plugin. A plugin whose agent skill declares a script sets `"echo"` to the version that ships
  `agent`.
- `ctx.agent.set({summary?, entries: [{name, summary, guide}]})` gives what changes at run time (one entry per vault,
  per watched project) — needs `agent` in the manifest; `name` as a skill name, ≤ 20 entries, `summary` ≤ 200 chars (it
  replaces the manifest's when given), `guide` Markdown ≤ 16 KB; each entry is a row in Echo's core and a `CLAUDE.md`
  Claude reads before working with it; entries not given again are removed; refused in a routine's or Yolo's run
  preparing and asking. Evidence: `plugins::agent::set_runtime`

## Scripts

- Declared in `uses.scripts: [{skill, script, readonly?, timeout?}]`, 1–20 a plugin, each once: `skill` one of its
  package skills; `script` a file in that skill's folder, as a relative path of plain names (letters, digits, `.` `_`
  `-`, none starting with a dot), ≤ 200 chars; `readonly: true` marks one that changes nothing (only such a script runs
  in a routine's or Yolo's run preparing and asking); `timeout` the seconds it may take, 1–3600, 300 by default (the
  sheet names one over 300). Evidence: `plugins::scripts::declared`
- Interpreter: the shebang (`#!/usr/bin/env node`); on Windows a `.ps1` runs with PowerShell, another script with the
  interpreter its shebang names. Echo ships no interpreter; a missing one fails the run with its message.
- Environment: the user's environment minus Echo's secrets (`ECHOCTL*`, but for `ECHOCTL_HOME`, `ECHOCTL_CONFIG_DIR`
  and `ECHOCTL_BIN` set to the running Echo's, so an echoctl it starts acts on the same data, and inside a routine's or
  Yolo's run its markers, so that echoctl is limited and recorded as the run) and every `CLAUDE*` / `ANTHROPIC*` key;
  a PATH of the app's plus the usual program folders (Homebrew, `~/.local/bin`, nvm's newest version);
  `ECHO_PLUGIN_SCRIPT=1` (nobody is there to answer a prompt) and `ECHO_PLUGIN_CACHE` (the store's `cache/`, never the
  store itself). No stdin; it runs in its copied skill folder. Evidence: `plugins::scripts::run`,
  `plugins::scripts::run_markers`
- Arguments: at most 100, each ≤ 32 KB with no NUL, passed as plain words, never through a shell.
  Evidence: `plugins::scripts::run`
- In a session: `SKILL.md` names a script by its path relative to the skill (`scripts/read.mjs <args>`); in Echo's
  sessions that path is Echo's launcher of it, allowed only on its own by its full path (in a routine's or Yolo's run
  preparing and asking only a `readonly` one). A session's run is bounded by Claude Code's Bash time (10 min at most).
  Evidence: `plugins::scripts::wrapper`
- From the host half: `await ctx.scripts.run(skill, script, args, {timeout})` → `{status, stdout, stderr, truncated?,
  timed_out?}`: `args` a list of strings, `timeout` in seconds up to the declared one (its default), never more; stdout
  and stderr 1 MB each (what is past it cut, with `truncated: true`); on a timeout or a kill `status` is `null`, with
  `timed_out: true` on a timeout; a second run of the same script while one runs is refused ("already running"). The
  run is async: the host half keeps answering its commands, widgets and work while it runs, and it doesn't count
  against their limits. A skill's script in a session hands its result to the host half with
  `plugin call <id> <command> '<json>'`. Evidence: `plugins::scripts::run`

## Widgets

### Declaring

In `widgets`, each `{id, label, about?, kind, sizes?, heights?, height?, refresh?, params?}`: `id` unique (a–z 0–9 `-`),
`label` 1–40, `about` ≤ 120, `kind` `stat`, `stats`, `chart`, `list`, `status`, `card`, `table` or `page` (a page needs
a `client`), `sizes` of `S` `M` `L` each once (default `["M"]`; a `table` only `M` and `L`), `heights` of `1` `2` each
once, with `1` (default `[1]`: the rows of Today's grid it may take), `height` `hug` (default) or `fill`, `refresh`
≥ 60 s (default 60); at most 24, and only with a `main` to give their data. Settings in `params`, 6 at most, as
`{key, label, type, options?, default?}`: `key` unique (a–z 0–9 `-` `_`), `label` 1–40, `type` `choice` (1–12
`options`, each 1–40 chars), `bool`, `thread` or `client` (each of these two once at most), a `default` only a value it
takes. Evidence: `plugins::check_params`

### Giving data

- `ctx.widgets.provide(id, ({size, params}) => data)` — Echo asks it in the background, answered within 5 s, again
  after `refresh`; or `ctx.widgets.set(id, data)` pushes it, and `ctx.widgets.clear(id)` takes the widget off Today and
  every dashboard; only ids the manifest declares, 64 KB of data at most. Evidence: `jshost::widget_ask`
- A widget with `params` gets its data only by `provide`: each copy — on Today or a dashboard — is asked with its own
  settings; a `set` replaces them all with one (`set` and `clear` reach every copy).
- `ctx.widgets.changed(id?)`: every copy of that widget (or of each one declared), Today's and each dashboard's, is
  asked again with its own settings, at most once in 10 s a widget (the last change within that time asked at its
  end); from a terminal's `plugin call` the running app asks it. A call made from inside a provider is ignored.
  Evidence: `jshost::widgets::widgets_changed`
- `params`: its defaults, then what the user set in Today's edit mode, with `echoctl today set`, or for every widget
  of the page at once with Echo's Filters widget or `echoctl today filter`; on a dashboard Echo sets its `thread` /
  `client` setting (`echo-host.md` § Dashboards). Echo asks again when one changes — until then an answer given for
  other settings isn't shown — and never gives a value the setting doesn't take. One filter reaches every widget with
  a setting of the same key (and options). Evidence: `today_layout::params_on`
- `asOf` (an ISO date and time) in the data: Today shows that time, not when the provider answered (one ahead of now,
  or not a time, is not believed). Evidence: `pluginWidgetView` (window script)
- `echoctl widget show plugin:<id>:<widget>` prints the data Echo gets.

### Data by kind

Text is plain (Echo escapes it); a text field given as anything but a string or a number is refused, text past its
limit cut. `tone` is `good`, `warn` or `bad` (a table's cell and a chart's series also `muted`). A list's or card's
`link` goes only to `echo://`; a table row's and a `stats` item's `link` also to `https://`.
Evidence: `pluginWidgetHtml`, `pluginTableHtml` (window script)

| Kind | Fields | At S / M / L | Characters kept | Refused |
|---|---|---|---|---|
| `stat` | `{value, unit?, label?, tone?, note?, progress?: {used, of}}` | `note` hidden at S | value 24, unit 12, label 80, note 200 | no `value`; `used` or `of` not a number |
| `stats` | `{items: [{label, value, tone?, trend?, link?}]}`; `trend`: numbers, the last 30 drawn as a sparkline under the value (none with fewer than 2) | first 2 / 4 / 6 items | value 24, label 60 | `items` not a list; a `trend` not a list of numbers |
| `chart` | § Charts | § Charts | § Charts | § Charts |
| `list` | `{items: [{title, sub?, link?}], empty?}` | first 3 / 5 / 10 items | title 200, sub 200 | `items` not a list |
| `status` | `{state, title, text?}`, `state` `ok`, `warn` or `bad` | the same | title 80, text 300 | another `state` |
| `card` | `{title?, text?, link?}` | the same | title 120, text 2000 | neither `title` nor `text` |
| `table` | `{columns: [{key, label, kind?, hide?}], rows: [{id, cells: {<key>: text, number or {text, tone?}}, link?, group?, detail?: [{text, sub?}]}], groups?: [{id, label}], note?, empty?}`; column `kind` `text` (default), `number`, `pill` or `date`; rows under their group's header in `groups`' order, the rest last; a row's `link` lands on its first text column | M and L only; a column with `hide: "M"` left out at M | cell 200, column label 40, group label 120, detail line 200, `note` 120 (beside the label) | 0 or over 10 columns; a column key not `[A-Za-z0-9_-]{1,40}`, or twice; over 200 rows; a row `id` missing, over 80 chars or twice; a `group` not in `groups`; over 50 groups; over 6 detail lines; a `number` cell not a number; a `date` cell not `YYYY-MM-DD` |
| `page` | the plugin's, drawn by its client half (§ Client half) | the draw gets `size` | the plugin's | — |

### Charts

`{type, points, series?, unit?, note?, percent?, center?, headline?}`; values are numbers up to 10^15 (larger is
refused). All series share one y-axis and the `unit`. Evidence: `pluginChartHtml` (window script)

| `type` | Draws | Points drawn | Series | Negative values |
|---|---|---|---|---|
| `line` | a line | the last 60 | 1–6 | below zero |
| `area` | a filled line | the last 60 | 1–6 | refused |
| `bar` | upright bars | the last 24 | 1–6 | below zero |
| `stacked` | upright stacked bars | the last 24 | 1–6 | refused |
| `bars` | horizontal bars | the last 4 / 6 / 12 at S / M / L | 1–6 | refused |
| `stacked-bars` | horizontal stacked bars | the last 4 / 6 / 12 at S / M / L | 1–6 | refused |
| `pie`, `donut` | slices | all read; up to 6 drawn in the given order, those under 3% and past the 6th folded into one Other after them (all zero: "Nothing to show yet") | one | refused |
| `funnel` | stages | the first 8 in the given order, each with its share of the one before | one | refused |

| Field | Shape |
|---|---|
| `points` | one series: `[{label, value}]`; several: `[{label, values: {<key>: number}}]` — a point without `values` refused, a missing key drawn as 0; label 40 chars |
| `series` | `[{key, label, tone?}]`, keys unique, 6 at most (more refused; more than one on a `pie`, `donut` or `funnel` refused); `tone` (`good`, `warn`, `bad`, `muted`) colours it by that meaning |
| `unit` | ≤ 12 chars |
| `note` | hidden at S |
| `percent` | `true`: each point as shares of 100% on `stacked` and `stacked-bars` (ignored elsewhere) |
| `center` | `{value, label?}` in a `donut`'s hole (ignored elsewhere); value 12, label 24 chars |
| `headline` | `{value, label?, change?: {by, than?, good?}}` above the chart: `value` text or a number (24 chars), `label` 60 chars; `by` a number up to 10^15 (larger refused), shown without its sign after an arrow (↑ above zero, ↓ below, → at zero) with the `unit`; `than` (40 chars) shown after "vs"; `good: true` shows the change in tone `good`, `false` in `bad`, absent neutral |

### Open, actions and links

Any widget but a `page` (which draws its own) may have `open` (an `echo://` place, or an `https://` page) and up to 3
`actions: [{label, command, args}]` (label ≤ 40 chars) — each runs the host half's `ctx.command(command)` with `args`
on the user's click, then Today reads the widgets again. A plugin's `https://` link — a widget's `open`, a table row's
or a `stats` item's `link`, a work row's `url`, its ⌘K entry — opens in the browser on the user's first click, nothing
asked; its host shows in the link's tooltip. Evidence: `pluginWidgetHtml`, `workRowOpen` (window script)

## Work rows

`ctx.work.provide(({scope, thread}) => rows)` under `uses.work`: `scope` `"me"` (Today → My work) or `"thread"` (that
thread's Work tab, `thread` its id); 200 rows at most, one wrong row refusing the answer; text plain (Echo escapes it).
Rows are acted on with `task.actions` (§ Client half); Echo's Close and Claude stay off them. Echo asks in the
background (5 s) and draws the last answer; it asks again when the answer is a minute old or after
`ctx.work.changed()`, a scope at most every 10 s (a `changed()` said sooner is asked once its time is up; one from
inside `provide` is ignored). A provider that fails or is slow keeps its last good rows, marked "not updated", for
24 h after that answer, with a line in Settings → Plugins naming where. A thread's rows go when it is deleted or
merged; a thread's scope not asked for in 30 days goes. `echoctl work show [--thread <id>]` prints them.
Evidence: `jshost::work::check_rows`

| Field | Shape |
|---|---|
| `id` | text, 1–200 chars, unique |
| `title` | text, 1–300 chars |
| `kind` | text, 1–30 chars |
| `where` | optional text, ≤ 120 chars |
| `url` | optional `https://` address, ≤ 2000 chars, no spaces, quotes, `<`, `>` or `\` (opened as § Open, actions and links) |
| `due` | optional `YYYY-MM-DD` |
| `late` | optional `true` or `false` |
| `thread` | optional thread id, ≤ 60 chars; on a "me" row it also puts the row in that thread's Work |

## Client half

- The client half exports `apply(ctx)`; its `ctx`: `call(command, args)` (the host half's commands), `listen`,
  `slots.list` / `slots.add` on a known slot, `panels.add`, `showPanel(view)` (one of its panels),
  `openEchoPanel(view)` (one of Echo's), `activities.add`, `settings.add` / `section`, `palette.add`, `widgets.draw`,
  `where()` (the activity and Settings section shown), `go(hash)`, `render()` and `ui` (`esc`, `setRow`, `setCard`,
  `sw`, `ICON`, `relDay`, `ago`, `notice`, `copy`, `style(css)`, `graph`, `newGraph`). Echo takes out what a plugin
  added when it unloads. Evidence: `pluginCtx`, `loadPluginClients` (window script)
- Slots: `call.menu`, `calendar.event`, `task.actions`, `thread.profile`, `knowledge.tab` and `knowledge.settings` (the
  last two change with Knowledge's rework). Evidence: `SLOTS` (window script)
- `ctx.ui.copy(text)` puts text on the clipboard on the user's click or key (without the page's user activation, or
  where the engine can't tell, it throws), 10,000 characters at most (the rest cut).
- `ctx.widgets.draw(widget, ({data, size, theme}) => HTML)` draws a `page` widget from `data` (given by `provide` /
  `set`). Evidence: `pluginWidgetView` (window script)
- `ctx.slots.add('thread.profile', {id, label, render(thread) → HTML, after?(box, thread)})`: `thread` is
  `{id, name, client}`, drawn under the thread's Sources; `render` is synchronous (a `ctx.render()` inside it is
  ignored), `after` runs once per box drawn afresh (fetch there, then `ctx.render()`), one row per id, and one that
  throws is left out, the Profile drawn without it. Evidence: `threadSlotRows`, `threadSlotAfter` (window script)
- `ctx.slots.add('task.actions', {…, when: (t) => t.kind === 'plugin' && t.source === ctx.id, run(t)})`: `t.row` is
  the work row, each row its own target; an item without `when` stays on Echo's tasks; after `run`, Echo reads the rows
  again. Evidence: `slotFor` (window script)

## Testing against made-up services

- In a development build `ECHOCTL_TEST_HOSTS="<host>=http://127.0.0.1:<port>,…"` (or `http://localhost:<port>`, an
  optional path after it) sends `ctx.http.fetch("https://<host>/<path>")` for a host the manifest names in
  `network.hosts` to `<base>/<path>` (`save:` downloads too), never through a proxy; the grant is checked on the real
  host first, and an address the user listed (`network.hosts_from`) is reached as it is. It fails closed: while it is
  set, one wrong entry (malformed, or not a local address) fails every request, and a named host it doesn't map is
  refused ("isn't mapped in ECHOCTL_TEST_HOSTS"). Curl sends the fake `Host: 127.0.0.1:<port>`: hosts it must tell
  apart are served on different ports. A release build never reads it. Evidence: `jshost::test_hosts::test_host`
- Made-up `gh` / `git` first on `PATH` (`gh.cmd` / `git.cmd` on Windows) answer `ctx.signin.get` in any build
  (`tests/app_profile.md` → Plugins against fake services).
- `ECHOCTL_PICK_FOLDER=<dir>` answers the folder picker in a debug build.

## Publishing

- A marketplace is a folder or git repository with `echo-marketplace.json` at its root
  (`{name, featured, plugins: [{id, source, categories}]}`), each entry's manifest `id` equal to its `id`.
  Evidence: `plugins::CatalogEntry`
- A marketplace source is `owner/repo[@ref]`, a git address (`https://`, `ssh://`, `git@`, `#ref`) or a local folder;
  a ref is a plain branch, tag or commit. Evidence: `plugins::parse_source`
- An update is offered when `version` changes. Evidence: `plugins::update_for`
- `echoctl plugin install | update | enable | disable | remove` from a terminal needs no restart: the running app's
  host halves and its window pick up the change. Evidence: `plugins::stamp`

## Examples to read

| When you are | Exemplar | Tests |
|---|---|---|
| writing a first host half | `jshost::a_host_half_gets_echos_services_and_answers_commands` | exemplar is a test |
| using folders, sessions or `echo.read` | `jshost::a_plugin_gives_echos_sessions_its_skill_and_the_folders_it_was_given` | exemplar is a test |
| adding Today widgets | `jshost::a_plugins_widgets_are_in_the_catalog_and_give_their_data_by_provide_and_set` | exemplar is a test; a plugin with one widget of each kind: `tests/fixtures/plugins` |
| giving work rows | `jshost::a_plugins_work_rows_come_by_scope_from_its_last_answer_and_go_with_it` | exemplar is a test |
| keeping values per thread | `jshost::a_plugin_keeps_values_per_thread_and_hears_when_a_thread_goes` | exemplar is a test |
| using the user's sign-in, or testing against fake services | `jshost::a_plugin_is_handed_a_sign_in_only_for_the_hosts_its_sheet_named_and_never_in_a_read_only_routine` | `jshost::a_plugins_named_host_reaches_the_fake_service_set_for_tests` |
| publishing an update | `plugins::install_update_and_remove_from_a_local_marketplace` | exemplar is a test |
