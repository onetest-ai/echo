# Echo Plugin API

**Reference**, not a guide: the facts of Echo's plugin API — the manifest, `uses`, the host half's and the client half's
`ctx`, events, widgets, scripts, Claude's part, publishing — as the code has them. It changes in the same PR as the code
it describes. The rules for using it are in `.ai-run/guides/plugins/plugin-authoring.md`; `echoctl plugin guide` prints
both, this after the rules. Where this and Echo's code differ, the code wins: fix this in that code's PR.

## Package and manifest

- A plugin is one folder with `echo-plugin.json` at its root: required `id` (1–40 chars, a–z 0–9 `-`, no leading `-`),
  `name` (≤ 60) and `version` (`X.Y.Z[-pre]`); optional `echo` (`>=X.Y.Z`, the Echo it needs), `summary` (≤ 240),
  `uses`, `settings`, `main`, `client`, `widgets`, `dashboards` (§ Dashboards it offers), `routines` (§ Routines), `skills`, `agent` and `icon` (an `.svg` in the plugin folder, named
  with a–z 0–9 `.` `_` `-` `/` only, ≤ 16 KB, shown only as an image beside its work rows and ⌘K entries).
  Evidence: `plugins::read_manifest`, `plugins::valid_id`
- `main` and `client` point at a `.js`/`.mjs` inside the plugin folder; any other file it ships is read with
  `ctx.package.read(path)` / `ctx.package.list(dir)`. Evidence: `jshost::package_file`
- A setting is `{key, label, type, default?, help?}`: `key` unique (a–z 0–9 `-` `_`), `type` `string` (default), `bool`,
  `folder` or `secret`; read with `ctx.settings.get(key)`, which gives what the user set, else the setting's `default`,
  else `null` — never a default for a `secret` or an address list the plugin may reach (only the user enters those).
  Before this, an unset setting gave `null` even when the manifest had a `default`: a plugin that wrote `get(key) ?? fallback`
  now gets the manifest's default where it got its fallback, so the two may differ.
  Evidence: `plugins::set_setting`, `plugins::setting`
- A settings page's "Test connection": the plugin's host half registers `ctx.command('__testSettings', async ({section}) => …)`
  and reads what it needs itself (`ctx.settings.get('token')`, secrets included); its answer is `true`, a message
  (ok), or `{ok, message}`. Its window half runs it with `ctx.call('__testSettings', {section})` and is given only
  `{ok, message}` — whatever else the host half answered is dropped, `message` is cut at 300 characters, and in it each
  value the user keeps as a secret (3 characters or more) is replaced by `••••` — as it is, URL-encoded, JSON-escaped,
  and in base64 (of the secret, of `:secret` and of `*:secret`), longest first — and every URL loses its query string and
  fragment. A host half that throws gives `{ok: false, message}` (its own words, masked); a plugin that has no such
  command gives "this plugin has no connection test". The same masking applies when the command runs from an Inbox
  item's button (`plugin_call`: a failed test's notice). The `message` is free text for the user: it must not echo
  request details (headers, bodies, URLs with keys). The masking guards against accidents only: the host half is the
  plugin's own code, trusted with the secrets, and can leak one through any channel it has. Evidence: `plugins::test_answer`,
  `plugins::call_notice`
- A plugin and a marketplace tree hold plain files only (no links or special files): 400 files / 20 MB a plugin,
  5000 / 64 MB a marketplace. Evidence: `plugins::check_tree`
- `echoctl plugin check <folder>` runs an install's checks of the package (manifest, `uses`, widgets, dashboards, skills,
  scripts, `agent/`), and its dashboards' widgets of the plugins beside it in the folder's parent (a marketplace's
  `plugins/`) when they are there: exit 0, or each problem named by file; `--json`. Needs no data folder, nothing installed, no network.
  Evidence: `plugins::agent::check_folder`

## `uses` and the permission sheet

Each `uses` key is a line on the permission sheet, in Echo's words; the user agrees to the sheet on install, and an
update adding a line asks again (`echoctl plugin update --yes`). Only these keys are known; `brain_source` and `export`
are not granted. Evidence: `plugins::CAPABILITIES`, `plugins::describe_folders`, `plugins::sheet`

| Key | Shape and use |
|---|---|
| `claude` | runs Claude (§ Claude runs); the sheet: "Runs Claude Code on your AI profiles, acting on its own as you set routines in Settings (Auto, or skipping every permission check)", then with `{"skip": true}` "; a run you mark so may skip every permission check" (only then is a run's own `tools: "skip"` honoured), then ". In a folder you gave it, Claude works with that folder's own Claude Code setup: it reads the folder and, if the plugin may change it, edits files there or in a copy for you to review — as you set routines; on your click in its screens it opens Claude Code there in Echo's Claude panel for you to ask, as you set Claude in the window" (§ Claude runs: `ctx.claude.session`; no capability of its own: `claude` and `folders` cover it) |
| `schedule` | works on its own on a schedule while Echo runs |
| `events.on` | the events it hears (§ Events) |
| `deliver.to` | where it sends to the user |
| `deliver_provider.kinds` | the delivery kinds it provides (§ Delivery) |
| `network.hosts`, `network.hosts_from`, `network.ask` | the `https://` hosts it fetches (§ Network); string settings without a default whose addresses the user lists; `ask: true`: it may ask the user for one address to fetch once (§ Network, A typed address) |
| `channel.plugin` | the Claude Code plugin it installs (§ Claude runs) |
| `sessions`, `sessions.commands` | its runtime skills for Echo's sessions; its commands Claude in Echo may run (§ Claude in Echo) |
| `folders.what`, `folders.write` | folders the user picks, read-only, or changed with `folders.write` (§ Folders and files) |
| `files.read` | other apps' files, each named in full, read with `ctx.system.readFile(entry)` |
| `scripts` | the scripts of its package skills Echo may run (§ Scripts) |
| `echo.read`, `echo.write` | Echo's data it reads (§ Echo's data); what it writes: `dictionary` (adds to the dictionary), `thread_stage` (a thread's stage), `tasks` (its own tasks in a thread) — each its own line on the sheet |
| `echo.jobs` | `true`: it shows Echo's background jobs and changes them (echo-plugins R25). The sheet: "Shows Echo's background jobs and changes how often each runs, when it pauses at night and which model it uses — only when you change them in its screens". Its client half's `ctx.echo.jobs.list()` / `set(job, ["key=value", …])` (§ Client half); nothing in its host half. Evidence: `capabilities::describe_echo`, `jshost::may_tune_jobs` |
| `m365.read` | `mail`, `chats`: the user's Microsoft 365 it reads through Echo's sign-in, never sending (§ Microsoft 365) |
| `me` | `{}` (nothing else): its host half reads About me through `ctx.me` (§ Echo's data, About me). The sheet: "Reads your About me in Echo (Settings → About me): your name and role, the people and terms of your calls, how you like things written and how you work". Evidence: `capabilities::describe_me` |
| `skills` | `true` (nothing else): its window half adds skills to the user's own Claude Code, each on a click (§ Client half: `ctx.claude.installSkill`); the sheet: "Adds instructions for Claude Code (skills) to your own Claude Code (~/.claude/skills), each only when you click to add it: skills that run commands are refused, never over a skill that is there already, changing or removing only the skills it added — they go when you remove the plugin" — on its own: it doesn't let the plugin run Claude Code (that is `claude`) |
| `signin.hosts` | 1–4 plain host names, each once, no IP addresses: the hosts whose sign-in saved on this computer it gets (§ Sign-in) |
| `signin.keep` | `true`: Echo keeps a sign-in it makes itself for those hosts, its own (§ Sign-in) |
| `work` | `{}`: its items on My work and in threads' Work (§ Work rows) |
| `microphone` | `{}` (nothing else): its window half hears the microphone through `ctx.microphone` (§ Client half). The sheet: "Hears your microphone in its own view: only after you click there, with Echo's recording sign on while it listens, and never while Echo records a call or a memo; it stops when you leave the view. What it hears can go to the addresses it may reach". Nothing in its host half. Evidence: `capabilities::describe_microphone`, `window::plugins::mic_open` |
| `ai.wrapper` | `{name, command}`: a wrapper profile it offers — `command` one program name (letters, digits, `.`, `_`, `-`), no arguments or path, found on the user's PATH when they add it, never a file of the plugin; `name` up to 40 characters. The sheet: "Offers an AI profile that starts <command> — a program you install yourself — in place of Claude Code; it sees and can change everything Claude in Echo is sent, call transcripts included. You add it in Settings → AI & minutes, if you want it". Only offered: the user adds it (Settings → AI & minutes, `echoctl ai add-wrapper --plugin <id>`); the plugin never makes it the default or picks it for anything. An update naming another program asks again, and the profile keeps its program until the user takes the new one on it (Use it, `echoctl ai update-wrapper <id>`); the plugin off or removed leaves the profile. Its offer is bound to its marketplace: a plugin of the same id from another one offers nothing to that profile. Its jobs (`ctx.claude.run({profile})`) run on a wrapper profile only when it is the user's default. Evidence: `plugins::describe_ai`, `ai::offered`, `ai::plugin_may_name` |

The sheet also names each routine (§ Routines): "Has a routine “<name>”, off until you switch it on in
Settings → Plugins: then <every, in words: every hour, every day, every 2 h, every 45 min> Echo runs Claude Code on your default AI profile with the prompt it ships, acting on its
own as you set routines — it may change your Echo data and send as you, as your Claude · Echo folder session may (its
prompt: <hash>)" (its own `tools`: "in auto mode", "skipping every permission check" when the sheet has `claude.skip`,
or "with no tools: it can only answer"); the hash is of its definition and its prompt, so an update that changes either
asks again, naming the changed routine; the user's yes to the update keeps the routine as they had it (on stays on).
A routine the update adds starts off (an install over the installed version too), whatever an earlier version of that
id left. Under the lines the window's sheet has "Routines — off unless you switch them on": a switch per routine it
offers — on an install every routine, on an update (or an install over the installed version) only those it adds or
whose sheet line changed (its prompt, its interval, or what its run may do, as an update adding `claude.skip` does; an
installed manifest that can't be read: all, and none is switched off) — each with its name, when it runs in plain words ("Every hour", "Every day", "Every 2 h", "Every 45 min", then
" · paused at night"), its `what` and, in small muted text under it, its facts (acts on its own as you set routines; may
change your Echo data and send as you; when its `tools` name one, "auto mode" or "skipping every permission check"; for "none" only "no tools: it can only answer"; its prompt <hash>); one it adds starts off, one it changes as Echo has it now. A routine shows once
there: the window leaves its long line out of "It will be able to" (the review's `routines[].line` names it, `facts`
the small text), while the CLI's review keeps the line. The routines the user switched there are set so once it is installed (only the switches
moved are sent: `plugin_op` `install` / `update` with `routines: {<routine>: on}`); a routine kept unchanged has no
switch and stays as the user had it. The CLI's `echoctl plugin install <id> --routines <a,b>|all` and `echoctl plugin
update <id> --routines <a,b>|all` switch on the routines named (`all`: every one the sheet offers); the review lists
them ("Its routines stay off unless you switch them on: --routines a,b (or all)"); a routine the sheet doesn't offer is
refused before anything is installed ("<name> <version> has no routine “x” to switch on here — its routines on this
sheet: …"); `update --routines` needs the plugin's id. Evidence: `plugins::routines::offered`,
`plugins::routines::install_with`, `plugins::routines::update_with`

The sheet also names each script, says it runs on this computer with the user's rights and can reach the network, its
time when over 300 s; an update that adds a script or changes any
file of a skill folder that has one asks again. Evidence: `plugins::scripts::describe_scripts`, `plugins::scripts::check`

## Host half

- The host half exports `apply(ctx)` and registers its commands, timers, event handlers and delivery ways inside it.
  Evidence: `jshost::load`
- It is one file with no `import` of other files (the client half may import its own modules).
  Evidence: `window::plugins::plugin_file`
- `ctx.command(name, fn)`: `fn` takes JSON arguments and, second, `{files}` (the handles its caller passed: its window
  half's `ctx.call(…, {files})`, `echoctl plugin call … --file`; `[]` otherwise, § Folders and files), may be `async`,
  returns JSON and throws an `Error` to fail. A
  window-called command has 30 s. More than 5 s of JavaScript between awaits is interrupted, and every call still
  waiting on the plugin then fails with “<id>'s job took too long”. The plugin's JavaScript may still finish its work
  after the caller was told the call failed (a call waiting on the network then, or work queued before the cut), so a
  retry must be idempotent; bulk file reads go through
  `ctx.fs.readMany` / `ctx.fs.walk`, which run off that slice. Evidence: `jshost::CALL_TIMEOUT`, `jshost::SLICE`
- `ctx.every(seconds, fn)`: periodic work, 10 s minimum; there are no timers in `echoctl plugin call`,
  `echoctl deliver`, `echoctl widget show` or `echoctl work show`. Evidence: `jshost::no_timers`
- Memory: 64 MB shared by all host halves. Evidence: `jshost::set_memory_limit`
- `ctx.sessions.skills`, `ctx.sessions.read` and `ctx.view.hints` are each set as the whole set (`[]` removes it) and
  are not kept across loads: set them from `apply` on every load. Evidence: `grants::drop_plugin`
- `ctx.inbox.set(key, {title, why, urgent, go, actions: [{label, command, args, primary}]})` (key ≤ 80 chars) asks the
  user for a decision; `ctx.inbox.clear(key)` takes it away. Evidence: `attention::inbox_items`
- `ctx.markdown(md)` renders Markdown to safe HTML in the host half (raw HTML as text, each line kept, https and
  `echo://` links only, no images). A page of a vault: `ctx.markdown(md, {breaks, links, images})` (both halves; the
  window half's is `async`). The minutes' writer, raw HTML shown as text, no `[mm:ss]` marks, with `[[Page]]`,
  `[[Page|label]]`, `[[Page#heading]]` and `![[image.png]]`; `breaks: true` makes a single newline a line break; a
  ```` ```mermaid ```` fence is shown as its code. 2 MB of text at most (more throws "ctx.markdown renders 2 MB of text
  at most"); the window half sends the page in a POST's body, as `ctx.call` does (§ Client half). Echo
  can't call the plugin while it writes, so the plugin resolves first: `ctx.markdown.links(md)` → `{links, images}`, each
  destination once as written (a wikilink's target: `Page#heading`; nothing in code); `ctx.markdown.links(md, {detail:
  true})` adds `entries: [{dest, kind: 'wiki' | 'md', embed}]` — each way a destination was written, once and in order
  (`[[Plan]]` and `[Plan](Plan)` on one page: two entries; `embed` for `![[…]]` and `![…](…)`) —, then passes `links` / `images`
  as `{"<dest>": "<href>" | null}`. Echo keeps a link's href only when it is an `https://` URL, an `echo://` link Echo
  knows (`data-echo-link`, the window opens it) or the plugin's own route — `#<id>:…`, or its Knowledge tab's `#brain/<id>:…`; an image's src only from its folder
  route `plugin/<id>/folder/…` (§ Folders and files; no `.` or `..` segment, `%2e` and `%2F` decoded); nothing with
  whitespace, control characters or a backslash, never `data:`, `http:`, `//host` or another plugin's route. Encode a
  name with spaces (`encodeURIComponent`). A destination not given or not allowed is text; `null` is a page that isn't
  there: `<a class="md-missing" data-md-missing="<dest>" role="link" tabindex="0">` with no href (an image:
  `<span class="figref md-missing" …>`), for the plugin to style and catch: a click (`ctx.listen('click', …)`) and, as
  it has no href, Enter on it (`ctx.listen('keydown', …)`). Headings get no `id`. Evidence:
  `plugins::markdown::render`, `minutes::page_html`
- In a terminal (`ctx.terminal` true: `echoctl plugin call`, `deliver`, `widget show`, `work show`) only the plugin asked is loaded
  (`work show`: the plugins that give work rows; `deliver`: all of them); skip start-up work there. Evidence: `plugins::session_commands`, `jshost::no_timers`

## Events

`ctx.on(event, fn)` for an event named in `uses.events.on`. The events (`plugins::EVENTS`): `meeting_soon`,
`call_started`, `call_ended`, `recording_started`, `recording_stopped`, `transcribed`, `minutes_written`,
`thread_synced`, `sources_changed`, `threads_changed`, `shared_changed`.

A call is named by its folder (`call`, as `call:<folder>` is its source id), never its path, and also by its `stamp`: the `YYYY-MM-DD_HHMMSS` its folder name starts with, which a retitle keeps (the folder's name changes, the stamp doesn't); a folder without one is its own stamp (its whole name). A call recorded or imported since Echo 0.10.0 has a stamp of its own that never changes, so key a record by it: a call that starts in a second another call's folder already has takes the next free second (its folder's time moves a second or two on; `started_at` keeps the real start). Two older calls started in the same second can share a stamp, and `call:<stamp>` reads neither of them (their folder names do). A plugin that reads
sources (`uses.echo.read` names `source` or `sources`) hears `minutes_written` and `sources_changed` whole; any other
plugin that hears them gets them without the keys marked *readers* below (`plugins::as_heard`).

| Event | Payload |
|---|---|
| `meeting_soon` | `{id, subject, start, threads}` (`threads`: the ids of the threads Echo puts the meeting in, `[]` when none) |
| `call_started` | `{app}`: `teams` or `meet` |
| `call_ended` | `{app, recording, next_call}`: `recording` whether Echo was recording it, `next_call` true when Teams went straight on to another call (its `call_started` follows) |
| `recording_started` | `{call, stamp, title, memo}`: `title` the arm's, else the meeting's subject (a memo's `Memo`) |
| `recording_stopped` | `{call, stamp, duration_s, reason}`: `reason` `call_ended`, `user`, `next_call`, `computer_slept` or `watch_stopped` |
| `transcribed` | `{call, stamp, ok}`: `ok` false when the transcription failed |
| `minutes_written` | `{id, link, call, stamp, title, started_at, ended_at, duration_min, app, memo, revision, rewritten}`; *readers* also `threads`, `client`, `profile`, `participants`, `minutes` — as `echo.read('source')` gives them for the call (§ Echo's data), `minutes` without `body`. `revision`: 16 hex characters of the minutes, another one when they change; `rewritten`: Echo told this call's minutes before (they were written again). No transcript or chat |
| `thread_synced` | `{thread, processed, left}`: a thread's work read again from its calls and mail — how many calls were read, how many wait for the next sync |
| `sources_changed` | `{changed, removed}`: the ids (`mail:<source>`) of Recall's mail conversations and channel posts new or changed, and gone, after a sync; *readers* also `items` — for each changed one (100 at most) what Recall decided when it indexed it: `{id, kind, title, subject, at, threads, client, profile, from, people, trust, link, url, m365_id, conversation_id}` (§ Echo's data, `source` of a mail or post), never its text. Nothing for a removed one but its id |
| `threads_changed` | `{thread, born, change: 'renamed', name, from}`, `{thread, born, change: 'merged', into, dropped}` (each key `into` lacks is moved to it; `dropped`: yours that didn't fit in its 64) or `{thread, born, change: 'deleted', values}` (your own values, removed); `born` tells this thread from a later one with its id |
| `shared_changed` | `{namespace, key, thread?, by}`: a shared value changed (not the same value again), told to the namespace's other plugins, never to the one that set it; it carries no value |

A call's words are not in a payload: read them with `ctx.echo.read('source', id)`. Events are not replayed — except
`threads_changed` and `shared_changed` from another process (`echoctl project delete`, a terminal's `plugin call`),
which wait for the running app (heard within about a second, or at its next start), at least once; a queued `renamed`
or `deleted` of an id that by then belongs to a new thread is dropped.
Evidence: `plugins::EVENTS`, `plugins::as_heard`, `share::minutes_payload`, `share::sources_payload`,
`plugins::thread_values::take_queued`, `plugins::shared::tell_changed`

## Store

- `ctx.store.read/write/list/remove/path`, by relative path only; a name at the store's top level is plain ASCII
  (letters, digits, `-` `_` `.`). `ctx.store.read(path)` → its text or `null` (none there), and throws on a file that
  isn't UTF-8; `ctx.store.read(path, {lossy: true})` gives it with those bytes as U+FFFD (no flag: the replacement
  character shows where). Evidence: `plugins::data_dir`, `jshost::not_echos`
- Every `ctx.store.write` takes the plugin's lock across processes (the app and `echoctl plugin call`).
  `ctx.store.write(path, text, {ifSha})` writes only while the file's sha256 is still `ifSha` (else "changed since you
  read it"), `{createNew: true}` only where no file is ("is there already"). `ctx.store.update(path, fn)` reads,
  calls `fn(text | null)` and writes its answer only over what it read; when another process wrote meanwhile it reads
  again and calls `fn` again (so `fn` may run more than once and only computes), up to 50 times. `undefined`, `null` or
  the same text writes nothing; it returns the text kept. `ctx.crypto.sha256(text)` → the hex sha256 of the text's
  UTF-8 bytes. Evidence: `jshost::folders::store_write`, `jshost::folders::put`
- Echo keeps its own files in the store and refuses a plugin's `ctx.store.write` of `settings.json`, `inbox.json`,
  `widgets.json`, `threads.json` or `work.json`, and its `ctx.store.remove` of `settings.json`, `threads.json`,
  `work.json` or the store folder itself: those change through `ctx.settings.set`, `ctx.inbox.set` / `clear`,
  `ctx.widgets.set` / `clear` and `ctx.threads.set`; `work.json` is what `ctx.work.provide` answered. The same for
  `client-values.json` (write and remove), changed through `ctx.clients.set`.
  Evidence: `jshost::not_echos`, `jshost::inbox_file`, `jshost::widgets_file`
- `ctx.store.usage(dir?)` (the same as `ctx.store.size`) → `{bytes, files}`: what the plugin keeps in its store, or in
  its folder `dir`; its own files only (not `settings.json`). `ctx.cache.size()` is that of `cache/`. Evidence: `jshost::service`
- `await ctx.store.copyIn(file, path, {createNew})` → `{path, size}`: a file handed to it (a handle: `ctx.system.pickFiles`,
  `ctx.web.fetch`, a saved picture, `--file`) kept as it is in its store, 50 MB at most, as `ctx.store.write` writes
  (store paths only, never Echo's own files there, never over a file unless `createNew: false`, the default is `true`),
  off the time slice. Evidence: `jshost::files::copy_in_store`
- `await ctx.store.walk(path, {ext, hidden, max, sha})` → `[{path, size, mtime, sha?}]`: as `ctx.fs.walk`, under the
  store's folder `path` (`""` the store) — its paths relative to the store (what `ctx.store.read` takes), off the
  time slice, 20,000 rows by default and 100,000 at most, dot names only with `hidden`, no link followed and nothing
  outside the store, `sha` for a file of 64 MB at most; a folder that isn't there gives `[]`. Evidence:
  `jshost::folders::store_walk`
- `ctx.http.fetch(url, {save: "<store path>"})` saves audio or a picture, shown from `/plugin/<id>/store/<path>` (media
  kinds only). Echo removes what lies in `cache/` over 7 days; the rest is the plugin's to remove with `ctx.store.remove`.
  Evidence: `jshost::sweep_store`, `window::plugins::store_file`

## Network

`ctx.http.fetch(url, {method, headers, body, save?, file?})` resolves `{status, body}` for any HTTP answer, a redirect
too (never followed). It waits 60 s at most, downloads 10 MB at most, and rejects as below. With `save` (a path in its
store) or `file` (a handle the user handed it, `ctx.files`, sent as the body as it is with `POST`, `PUT` or `PATCH` —
never with `body` too) the limits are 512 MB each way and 15 minutes. Evidence: `jshost::http::fetch` Plain http is allowed only to
an address the user listed, any host; one outside the user's own network (private IPv4 ranges, `localhost`, `*.local`)
gets a note on the setting's row and from `echoctl plugin set` ("plain http: anyone on the way can read it"). Only the
user enters those addresses (window or CLI), never the plugin (`ctx.settings.set`, its `settings.json`, a manifest
`default`). The sheet's line: "Talks to the addresses you list in its setting “<key>” (wildcards allowed, plain http
too; its window part also over WebSocket)". Evidence: `jshost::http::fetch`, `jshost::http::curl_head`, `plugins::addresses`,
`plugins::listed_addresses`, `plugins::addresses::notes`

The list is `scheme://host[:port]` patterns (http or https; no port: the scheme's own; `:*` any), comma- or
space-separated; `*` for a number of an IPv4 address (not the first), a leading name label (`*.lab.local`: one or more
labels; `*.local` alone is allowed, any other one-label name isn't) or the port. A name's last label is never a number
— all digits or `0x` and hex digits, which curl and the window read as an IPv4 address (`0x7f.0.0.1`, `1.2.3`,
`2130706433` are refused: an IPv4 address is four plain numbers); `https://3dprinter`, `http://3dprinter.local` are
names.

**The window half** reaches the same addresses with the page's own Web APIs — `WebSocket`, `fetch` (streams too),
`EventSource`, `<audio>` / `<video>` — and no other outside address: the window's content security policy lists, in
`connect-src` and `media-src`, every address the user listed for a plugin that is on, each for its scheme and the
matching WebSocket one with its port (`http://*.local:*` → `http://*.local:*` and `ws://*.local:*`;
`https://voicecho.local` → `https://voicecho.local:443` and `wss://voicecho.local:443`). An IPv4 pattern with a `*` number
can't be said in a policy and isn't added: the setting's row says its window part can't reach it (list each address).
The policy is fixed for the page: when the list changes (an address added or removed, such a plugin switched on, off or
removed) the page loads again by itself on the same route (the plugin picked in Settings → Plugins stays picked), but
not while a focused text box, text area or editable region holds text the user typed. The policy is the page's, not
one plugin's: while a plugin with listed addresses is on, the window part of every enabled plugin can reach them. Each
plugin's sheet names only its own setting; the setting's row (and `echoctl plugin set`) says "Echo's window is one
page: while this plugin is on, the window part of any plugin you have on can reach these addresses too" once the list
holds an address the window may reach. Evidence: `plugins::SHARED_WINDOW`
- The platform's own network rules apply on top of the policy:
  - macOS: App Transport Security lets the window (WKWebView) reach plain `http://` / `ws://` only to an IP address,
    a `.local` name or a single-label name — not to a public name (`http://tts.acme.example` is blocked there even
    when listed); use https for those. The host half's `ctx.http.fetch` runs through curl and isn't affected.
  - macOS 15 and later may ask the user once for Local Network access the first time the window reaches the local
    network; refused, those connections fail (System Settings → Privacy & Security → Local Network turns it back on).
  - Windows: WebView2 may apply Chromium's local-network-access rules (a public page reaching a private address);
    Echo's page is `http://echo.localhost`, a local origin, but a future WebView2 may still ask or block.
- `fetch` and `EventSource` still go through CORS: the server must allow Echo's origin (`echo://localhost` on macOS,
  `http://echo.localhost` on Windows; `Access-Control-Allow-Origin: *` covers both). WebSocket has no CORS: the
  server sees the page's `Origin` and decides.
- `wss://` / `https://` with a self-signed certificate isn't accepted by the webview: the keys the user trusts are the
  host half's alone (below). Reach such a server from the host half, use plain `ws://` / `http://` inside the user's own
  network, or a certificate the system trusts.
- The host half has no microphone.

Evidence: `window::window_csp`, `plugins::window_sources`, `plugins::addresses::Address::csp_sources`, `netReload`
(window script)

**The host half's connections** (it runs without the window, so it has none of the page's Web APIs):

- `ctx.http.stream(url, {method, headers, body, file}, onChunk)` → `{status, headers, done, cancel()}` at once, the same
  checks as `fetch`: `onChunk(text, s)` for each piece as it comes (SSE, chunked; UTF-8 never cut inside a character),
  `status` and `headers` (lower-case names) set once the answer's head arrived, `done` resolving `{status, bytes,
  cancelled}` at its end (rejecting with why it failed), `cancel()` ending it. No time limit (an event stream stays
  open), 512 MB at most. Evidence: `jshost::http::stream`
- `await ctx.net.websocket(url, {headers, protocols})` → `{protocol, send, close, onMessage, onClose, onError}` once
  open (rejects with why not): `ws://` and `wss://` only to an address the user listed — its `http://` / `https://`
  address and port (never a `network.hosts` host). `send(text)`, `send(base64, {binary: true})`, `send(handle)` (a file
  the user handed it, as one binary message); `onMessage(data, {binary})` (binary as base64), `onClose({code,
  reason})`, `onError({message})`; `close(code, reason)` (1000 or 3000–4999). Events wait for their handler, set after
  the `await` — up to 1,000 of them and 8 MB of their data, all the plugin's WebSockets together; past that the
  connection is closed and `onError` says why. Headers can't be the WebSocket's own (`Host`, `Upgrade`, `Connection`, `Sec-WebSocket-*`); `protocols`
  are plain words. Evidence: `jshost::net::websocket`
- Each runs on a thread of its own; its events reach the plugin's JavaScript like Echo's other events, one slice each.
  A plugin keeps 8 connections (WebSockets and streams) open at once. A WebSocket message is 16 MB at most out and 4 MB
  in (binary in arrives as base64, in the 64 MB every plugin's host half shares): a bigger one in closes the connection
  (code 1009) with an error saying so. While the plugin is 256 events behind on a connection, or its connections' events
  waiting for it pass 16 MB together, Echo reads no more from them. Switching the plugin off, updating or removing it closes every one (no event follows). Evidence:
  `jshost::net::MOST`, `jshost::net::stop_plugin`

**Certificates the system doesn't trust** (a server of the user's own, self-signed or from an internal CA): https and
wss to an address the user listed fail the first time with the key of the certificate met there — "the certificate at
`<host:port>` isn't one this computer trusts (its key: `sha256//…`): if it is your own server's, trust it in Settings →
Plugins → `<name>`, or with `echoctl plugin trust <id> <host:port>`" — and Echo keeps that key as met. The user trusts
it in the plugin's detail under Certificates ("Trust": a dialog of the system's asks again) or with `echoctl plugin
trust <id> <host:port> [--key sha256//…]`; `echoctl plugin certs <id>` lists them, `echoctl plugin untrust <id>
<host:port>` (or "Forget") drops one. Without `--key` `trust` takes the key `certs` shows as met, and is refused (give
`--key`) once Echo met more than one key there since the user last trusted or forgot one. From then on that address is reached with that key alone — not the system's
trust, its name or dates — and another key is refused ("… changed since you trusted it (its key is now …; you trusted
…): refused until you …") until the user trusts it. A key is the SHA-256 of the certificate's public key, as curl's
`--pinnedpubkey` writes it (`openssl x509 -in cert.pem -pubkey -noout | openssl pkey -pubin -outform der | openssl dgst
-sha256 -binary | base64`): a certificate renewed with the same key goes through. Echo keeps them outside the plugin's
store (`<config dir>/plugins/pins/<id>.json`), only for https addresses the user listed; no plugin sets one, and none
is changed from inside a run Echo started. A plugin removed with its data loses them. Evidence: `plugins::pins`,
`jshost::tls::connect`

| Rejected | When |
|---|---|
| the host | not granted: neither `https://` to a host in `network.hosts` (named with no port and no user) nor an address the user listed (`network.hosts_from`); a WebSocket: not an address the user listed |
| the certificate | https / wss to a listed address the system doesn't trust and no key trusted, or not the key the user trusted |
| too many | a 9th connection open at once, a WebSocket message over 16 MB out or 4 MB in, a file sent over 512 MB |
| the method | other than `GET`, `POST`, `PUT`, `PATCH` or `DELETE` |
| a header | a name that isn't a plain token, a value with a NUL or a line break (a NUL in the URL or body too); `Content-Length` or `Transfer-Encoding` with `file` (Echo sets them) |
| the request | the host can't be reached, the 60 s or the 10 MB passed (15 minutes, 512 MB with `save` or `file`) |

**A typed address** (`uses.network.ask: true`): `await ctx.http.askAddress()` opens Echo's own box — the system's text
dialog, not the plugin's page — saying, in Echo's words, "<plugin name> asks for the address of a web page to fetch
once. Type or paste it:"; it resolves the address typed (trimmed) or `null` (cancelled). Echo takes only `http://` or
`https://` with a host, no user part (`@`), no spaces, quotes, `<`, `>`, `\`, backticks or control characters, 2,000
characters at most (anything else rejects: "… isn't a web address Echo fetches"). That plugin may then
`ctx.http.fetch` exactly that address once, within 10 minutes (a newer one replaces it), as a plain `GET`: no
`headers`, no `body`, and like every fetch no redirect followed (a page that moved answers its 3xx). Another address,
another plugin, a second fetch, another method, headers or a body is refused as any host not granted. Refused inside a run Echo started. The sheet: "Fetches a web page from an address you
type in Echo's own box when it asks — that page only, once each time" (after the `Talks to …` line when it names hosts
too). A plugin never sets the address: not by its settings, its store or its window half. Evidence: `jshost::typed`

### Reading a web page (`ctx.web`)

No capability of its own and no line on the sheet: reading a page is a GET under the network grants above, made
through the same check and the same curl as `ctx.http.fetch` (`http::admit`, `http::get_capped`) — a host in
`network.hosts` over https, an address the user listed (its pinned key too), or the page the user typed in Echo's box
(`askAddress`, once). Like every fetch it follows no redirect, and in a run Echo started it is allowed or refused as
`ctx.http.fetch` is. Each request is a plain GET: no cookies, no credentials or sign-ins, no headers of the plugin's,
`User-Agent: Mozilla/5.0 (compatible; Echo)`, `Accept-Encoding: identity`. Evidence: `jshost::web`

- `await ctx.web.read(url, {max, timeout, images})` → `{url, title, markdown, links: [{text, href}], images: [{src, alt,
  caption, width, height}], lang, byline, published, description, source, needs_js, truncated, bytes}`. `url` is the
  address asked for (no redirect followed: a page that moved rejects with its 3xx). HTML only (`text/html`,
  `application/xhtml+xml`; anything else rejects "this is <type>, not a web page"), never compressed (a
  `Content-Encoding` other than identity rejects before its body is read), 5 MB (`max` less) read as it comes and
  stopped past it, 30 s (`timeout` less); the charset the server or the page names (else UTF-8; bytes that aren't in it
  become �). Off the plugin's 5 s slice, as every async service.
  - `markdown`: the main part (the largest `<article>`, else `<main>`, else `<body>`) without scripts, styles,
    navigation, headers, footers, asides, forms, frames, objects, SVG or templates; headings, paragraphs, lists, code,
    tables, links and pictures kept. The title (its first `<h1>`, else JSON-LD's headline, else `og:title`, else
    `<title>`) isn't repeated in it. A page that builds itself with JavaScript is never run: what it embeds — JSON-LD
    (an article's `articleBody`, `headline`, `author`, `datePublished`, `image`), a framework's JSON state
    (`__NEXT_DATA__`, `__NUXT_DATA__`, `application/json` scripts: its strings that read as prose, HTML in them read as
    HTML), `<noscript>` — is read as data, and its text is taken when clearly fuller than the HTML's (`source`: `html`,
    `json-ld`, `state`, `noscript`, or `meta` for an Open Graph description alone). `needs_js`: under 200 characters of
    text on a page with scripts.
  - Links: absolute `http://` / `https://` only (relative ones against `url`); `javascript:`, `data:`, `file:`,
    `mailto:`, the page's own anchors, an address with a user part (`https://bank.example@evil.example`) and one over
    2 KB once resolved are their words. `links` lists the main part's, each once (2,000 at most), with its text (500
    characters at most); a link inside a link isn't listed again (a browser never nests them).
  - Pictures: in place as `![alt](src)`, a figure's caption under it as text; `images` lists each once (500 at most,
    its `alt` and `caption` 500 characters at most; a caption leaves out figures inside it),
    from `<img>` (`srcset`'s largest, a lazy `data-src`), `<picture>`'s largest source, then `og:image` and JSON-LD's;
    `data:` and a picture whose page-given width or height is under 32 left out.
  - The page's words are never markup: outside code, every ASCII mark with a meaning to Markdown or GFM wherever it
    stands is backslash-escaped (`` \ ` * _ ~ [ ] ( ) < > | # ! { } ``), an `&` only where it starts an entity (`&#…`,
    `&name;`), and at a line's start (after its indent) a `-`, `+` or `=` (`\-`) and digits with a `.` (`1\.`); an inline code span is fenced one backtick longer than the longest run in it and never touches another
    backtick (a space between), and in a table's cell code holding a `|` is escaped text; a `<pre>` is fenced by
    backticks one longer than its longest run (never tildes); a link's or picture's address has `|` and `` ` ``
    percent-encoded. Control characters and invisible direction and joining marks (U+200B–U+200F, U+202A–U+202E,
    U+2060–U+2064, U+2066–U+2069, U+FEFF) are dropped from the text, title, links and pictures. Its work is bounded:
    256 levels of nesting (deeper elements keep their text), 250,000 elements and text pieces (`truncated`: the rest
    left out), 2 MB of each embedded JSON. Its output is bounded too, checked as it is written: the Markdown is at most
    ten times the HTML and 20 MB, one budget for the main text and one for each alternative (all of a page's
    noscripts together, all of its framework state together). A cut falls only between blocks or a paragraph's
    lines: a fenced block, a heading, a picture is left out whole; a table's rows and a paragraph's lines are kept
    while they fit. A table keeps 64 columns; list items have a marker and indent by its width for 8 levels of items
    (lists without items don't count; a deeper item's text sits at the 8th level's indent, with no marker); a numbered
    list starts at 1,000,000 at most (`truncated` when something was left out). Evidence: `web::read::read`
  - `images: 'save'` also downloads its pictures: each entry gets `saved` — a file handle `{file, name, size, type}`
    (§ Folders and files: `ctx.files.read`, `ctx.fs.copyIn`) — or `error`, why not. 50 pictures, 10 MB each, 50 MB and
    2 minutes for a page; PNG, JPEG, GIF, WebP, AVIF and SVG only (by the answer's type; Echo never shows an SVG: the
    folder picture route serves raster only). Saved in the store's `cache/web/` (swept with `cache/`) as `NN.<ext>`;
    the Markdown keeps each picture's `src`, and the plugin maps `src` → `saved.name` when it writes them elsewhere.
    A picture is fetched as anything the plugin may reach — or, **the one widening**, as a browser shows a page: a page
    on the public internet may have Echo fetch its pictures from their own hosts though the plugin wasn't granted them,
    only these pictures, GET only, https (http only for an http page), port 80 or 443 (or the page's own port on the
    page's own host), a host name or an IPv4 address (an IPv6 address written as one refused), and only from a public
    address: Echo looks the host up (10 s at most, within the 2 minutes), refuses it when any of its addresses is this
    computer or a private, link-local, shared, unique-local, site-local or reserved range, or an IPv6 form that carries
    or tunnels to an IPv4 address (IPv4-compatible and -translated, 6to4, Teredo, local-use NAT64; v4-mapped and NAT64
    only to a public IPv4), an address written as a number other than plain IPv4 or IPv6 refused too, and connects to
    the address it checked, never through a proxy. A page on the user's own network or this computer gets only pictures the plugin
    may reach. Evidence: `jshost::web::picture_reach`
- `await ctx.web.fetch(url, {max})` → a file handle `{file, name, size, type}` for one file the plugin may reach (the
  same rules; any type), 10 MB (`max` up to 50 MB, what `ctx.fs.copyIn` copies), 2 minutes; `name` the address's last
  segment in letters, digits, `.`, `_`, `-`.
- `await ctx.web.markdown(html, {base})` → the same as `read` for HTML the plugin has (a mail's body, a page it fetched
  itself), without `url` and `bytes`; `base` an http(s) address of 2 KB at most for its relative links (none: they are
  text). 5 MB.
  Nothing is fetched. Also for a page's DOM rendered elsewhere.

What comes back is the page's: untrusted data. A plugin shows it only through `ctx.markdown` (raw HTML as text, its
links and pictures only as that allows: § Host half) — never as HTML of its own, never in a prompt as instructions.

| Rejected | When |
|---|---|
| the host | as `ctx.http.fetch` (a page's picture: also on this computer or the user's network, unless the plugin may reach it) |
| the answer | not 2xx (a 3xx: "it moved", never followed), compressed, not HTML (`read`) or not a picture (`images`), over its size or time |

## Claude runs

- `await ctx.claude.run({prompt, title, profile, model, tools, dir, timeout})`, only with `claude` in `uses`. It runs
  as the folder session and acts on its own (`untrusted-text.md` R20, R21), its permissions by `tools`, a routine's own
  choice:
  - `"auto"`: Claude Code's auto mode where the profile's model has it, else edits accepted;
  - `"skip"`: every permission check skipped — only when the sheet has `"claude": {"skip": true}`, else as not given;
  - not given (`""`): the user's setting for routines (Settings → AI & minutes → Claude's permissions: Auto by default,
    or Skip all permission checks);
  - `"none"`: no tools at all (what it needs goes in the prompt);
  - from before, `"read"` and `"folder"` are taken as `"auto"`; any other value is refused.

  What it asked is kept in the run's record. Its result lists the changes and sends it made (`actions`).
  Evidence: `claudejob::Tools::parse`, `plugins::may_run_claude`
- A run under a key of its own: `ctx.claude.run({..., key})`, `key` 1 to 64 letters, digits, `.`, `_` or `-` (any
  other refused; upper and lower case the same key). One run at a time per plugin and key, across Echo's processes (the app and a terminal's
  `echoctl plugin call`): a second while one runs is refused ("a run with key “…” is running already"). `await
  ctx.claude.stop(key)` (needs `uses.claude`) stops the plugin's own run under that key — Claude Code and every process
  it started (its process group; Windows: `taskkill /T /F`), wherever it runs — and gives `true`; `false` when none
  runs. The stopped `run` fails with the message `stopped`; its log ends "stopped by the plugin (ctx.claude.stop)" and
  its record (`permissions.json`) has `stopped: true`; a `copy` run's snapshot is removed. A key names only the
  calling plugin's runs: another plugin's run under the same key goes on. A plugin removed has its runs under a key
  stopped first. Evidence: `claudejob::Keyed`, `claudejob::stop_keyed`, `claudejob::stop_keyed_runs`
- In a folder it was given (needs `uses.folders`): `await ctx.claude.run({folder, path, mode, prompt, title, profile,
  model, dir, timeout})` — `folder` a root as `ctx.fs.folders()` names it, `path` a subfolder of it (`""` the root).
  Claude Code starts there as that folder's own project: its `CLAUDE.md`, `.claude/` skills, settings and hooks load
  (`--setting-sources user,project` on a profile of Echo's own, as the run's last arguments; the user's own Claude
  Code setup and a wrapper load them anyway), none of Echo's session (no Echo CLI, Brain, plugins' skills or Echo's
  hooks), no MCP server (`--strict-mcp-config`), never Bash or the web. With `path`, Claude Code starts in that
  subfolder: the project is the subfolder's — its `.claude` (skills, settings, hooks), and the `CLAUDE.md` files of
  the subfolder and of the folders above it. `mode`:
  - `read` (the default): `--tools Read,Grep,Glob,Skill`;
  - `edit` (needs `folders.write`): `--tools Read,Grep,Glob,Skill,Edit,Write` — Claude Code keeps its edits inside the
    folder;
  - `copy`: the `edit` run on a snapshot of the folder in its store (`ctx.fs.snapshot` with dot names, so `.claude`
    comes along; never `.git` or `.obsidian/plugins`, its caps and its 3-snapshot limit), the folder itself untouched;
    the result's `snapshot` is the snapshot's answer (`{path, files, bytes, skipped, why}`), for the plugin to compare
    with the folder and apply what the user accepts (`ctx.fs.write`); nothing is applied by Echo. A `path` the
    snapshot left out is refused before the run, and a run that fails removes its snapshot. `exclude` (only with
    `copy`; in another mode or without a folder refused) is the snapshot's: the paths under the root it leaves out,
    100 at most.

  Its permissions are the user's setting for routines (Settings → AI & minutes → Claude's permissions), by
  `folder_args::permission_args`: Auto — `--permission-mode auto` where the profile's model has auto mode, else
  `acceptEdits` for `edit` and `copy` and `dontAsk` for `read`; Skip all permission checks —
  `--dangerously-skip-permissions` in every mode. A plugin can't choose a permission mode or tools of its own (`tools`
  is ignored there). Its record (`permissions.json`) and log name the mode it asked (`read`, `edit` or `copy`) and the
  permissions it runs with; a start in another permission mode than asked is said there too. On a profile of Echo's
  own, a folder whose `.claude/settings.json` or `settings.local.json` could route the run elsewhere (a routing variable in its `env`, a key or
  policy helper; or not JSON) is refused. A call's text reaches the run only as the plugin puts it in the prompt: Echo
  can't see it there, so a plugin that does passes the call's `profile` (`ctx.echo.read('source', …)`). Evidence:
  `claudejob::in_folder`, `jshost::folders::claude_folder`
- A session the user asks in, from the client half (needs `uses.claude` and `uses.folders`):
  `ctx.claude.session({folder, path, mode, profile})` opens Claude Code in Echo's terminal, in its own bottom-panel tab
  "Claude · <plugin name>", in `folder` (a root as `ctx.fs.folders()` names it) or its subfolder `path`, as that folder's
  own project — as a folder run: the folder's setup (`--setting-sources user,project` on a profile of Echo's own, last),
  `mode` `read` (the default, `--tools Read,Grep,Glob,Skill`) or `edit` (needs `folders.write`, also `Edit,Write`; no
  `copy`), no MCP server, never Bash or the web, refused on a folder whose `.claude/settings.json` or `settings.local.json` could route it — but
  interactive, its conversation kept (`--session-id`, then `--resume` for that plugin and folder on that profile), and
  its permissions the user's setting for Claude in the window (Settings → AI & minutes → Claude's permissions: Ask each
  time — Claude Code's own prompts, no flag; Auto — `--permission-mode auto`, or `acceptEdits` / `dontAsk` where the
  model has no auto mode; Skip all permission checks — `--dangerously-skip-permissions`), never the plugin's. `profile`:
  one of the user's AI profiles (a wrapper only when it is the default), else the default. It opens only on the user's
  click or key in the plugin's screens (`copyAllowed`, else it throws before anything is sent); asked again while it
  runs, it is shown (in another folder it throws: one session per plugin). Each plugin's session has its own tab, so
  sessions of several plugins run side by side; one found running when the window opens gets its tab back, and its
  Resume keeps the profile it ran on (Echo checks it again). ⌘K has "Restart" and "New conversation" for each, as for
  the Claude · Echo folder session. The panel names the plugin, the folder, the mode, the permissions and the profile.
  Its `ctx` has no way to type into or read the session, and Echo types nothing in — only the user's keys. Switched off
  or removed, its session runs on as the user's (its tab goes once it has ended) and doesn't start again. Refused
  before Claude Code starts: no `claude` or `folders` on
  its sheet, a folder not given, a path outside it or not a folder, `edit` without `folders.write`, another mode, a
  wrapper profile that isn't the default, the plugin off. Evidence: `terminal::start_plugin`,
  `jshost::folders::session_folder`, `claudejob::in_folder_session`, `pluginSessionAsk` (window script)
- `ctx.claude.installPlugin(spec)` installs the channel plugin the manifest names (`uses.channel.plugin`) into the
  default AI profile's Claude Code; `ctx.claude.hasPlugin(spec)` asks whether it is there; both take only that spec.
  Evidence: `capabilities::describe_channel`

## Routines

The one shape for every routine, whoever's it is: the user's (the Scheduler's own, mapped by it), Echo's jobs
(`source: "system"`) and a plugin's own (`"plugin:<id>"`, below). A plugin that shows routines draws them all from it.

- **Routine:** `{id, source, name, what, enabled, to: ["today" | "thread_work" | "thread_sources" | "minutes" | "prep" |
  "recall" | "live_notes" | "mail" | "teams" | …], schedule: {text, kind: "time" | "timer" | "event"}, next_run (ISO,
  or null), next (in words, for one that runs on events, or null), paused_until (ISO while paused at night, or null),
  running, last (a run, or null), model ({value, default}, or null when it has none to choose), month: {runs, failed,
  tokens, cached, cost, daily: [{day: "YYYY-MM-DD", runs, failed, cost}] × 30, oldest first, ending today}, settings
  (what it lets the user change: `{key, label, kind: "number" | "night" | "model", value, default, range, set}`)}`.
- **Run:** `{started, ended, duration_ms, state: "done" | "empty" | "failed" | "running", ok, summary?, error?, model?,
  calls?, input?, output?, cache_read?, cache_write?, cost_usd?, sent: [...], trial}` — `summary` or `error` one line;
  `sent` where its result went; `trial` a "Try it" run. A run with no model has no tokens or cost. An Echo job's
  `summary` / `error` is Echo's own words and counts, never a call's, thread's or person's name nor a model's text.
- Echo's jobs: `enabled` is false while what turns a job off elsewhere is off (Recall, live notes, automatic minutes,
  "Keep threads' work current"; for minutes, off means not automatic: minutes the user asks for still run and show); `running` is true while a run Echo's app started on its own thread is under way
  (Needs you now, prep, Recall, a live notes summary, a minutes run the app writes) — a thread's sync, the fetch, a
  live notes session and a minutes run in another process never show running. `ctx.echo.routines.list()` → `{routines, night: {from, to, default: {from, to}}}` and
  `ctx.echo.routines.runs(id, {limit})` (newest first, 20 by default, 500 at most), for a plugin with `uses.echo.jobs`;
  changes through `ctx.echo.jobs.set` (§ Client half). Echo keeps a job's runs 31 days; a pass that did nothing leaves
  none. `echoctl jobs routines [--runs <job>]` prints the same. Evidence: `jobs::routines`, `jobs::routine_runs`
- **A plugin's own routines:** `"routines": [{id, name, what, prompt, every, night?, tools?, timeout?}]` in its
  manifest, at most 4, with `uses.claude`: `id` a–z 0–9 `-` (≤ 40, once), `name` one line ≤ 60, `what` one line ≤ 200,
  `prompt` a `.md` file of its package (UTF-8, not empty, ≤ 16 KB), `every` minutes 15–1440 (the default the user may
  change), `night` paused at night by default (`true` when not said), `tools` `auto` / `skip` / `none` or left out (as
  `ctx.claude.run`'s: § Claude runs), `timeout` minutes 1–60 (20). Each is a line of its sheet (§ `uses` and the
  permission sheet). Its id among the routines is `<plugin>.<routine>`, `source` `"plugin:<id>"`, with `plugin` its
  plugin's name, `to` `[]`, `schedule` `{text: <the interval in plain words: "Every hour", "Every 45 min"…>, kind: "timer"}`, `enabled` true only while the user
  switched it on and its plugin is on, `next_run` then (else null), and `settings`: `on` (kind `"switch"`, false by
  default), `every`, `night`, `model` (default "the profile's minutes model"). It is off until the user switches it on —
  on its install or update sheet (`--routines` in the CLI: § `uses` and the permission sheet), on the sheet its plugin's
  setup opens (`ctx.echo.routines.offer`, § Client half), its row in Settings → Plugins, or `echoctl jobs set
  <plugin>.<routine> on=on` — and tuned as Echo's jobs are
  (`echoctl jobs set … every=30 night=off model=sonnet`, never inside a run Echo started); a plugin with
  `uses.echo.jobs` tunes `every`, `night` and `model` on the user's click (`ctx.echo.jobs.set`, which then resolves to
  the routine) but never `on`: "a plugin can't switch a routine on or off: you do, in Settings → Plugins or with
  echoctl jobs set". The plugin's own code neither switches nor starts it. Evidence: `plugins::routines`, `jobs::set_from_plugin`
- It runs while Echo runs, one run at a time across processes, `every` minutes after its last run began (at once when
  it never ran), not in a night it pauses in, while it is on and its plugin is on; `echoctl jobs run <plugin>.<routine>`
  runs it now, on or off (its plugin on; refused inside a run Echo started). A run is `ctx.claude.run`'s (§ Claude
  runs) on the user's default AI profile, its `model` setting as that profile maps it, with the prompt file read from
  the installed package after Echo's lines: "This is the routine “<name>” of the plugin “<plugin>”, run by Echo for the
  user (every N min while it is on). Its last run started <time>. | It hasn't run before. Its answer is kept for the
  user with its run; nobody is sent it." It sends nothing itself; a send its run makes through Echo is noted.
- Its runs are kept as Echo's jobs' (§ Routines' run, `jobs-runs/<plugin>.<routine>.jsonl`): `summary` "done" or "done:
  N changes and M sends through Echo", `error` "the run failed: its log says why", with `run` (its folder),
  `changes`, `sends`, tokens and cost — never the model's text. The run's folder (`<config dir>/jobs-runs/<id>/<run>/`:
  `prompt.md`, `log.txt`, `answer.md`, `actions.jsonl`, `drafts.jsonl`, `permissions.json`; the newest 20 kept) is the
  user's: `echoctl jobs outcome <id> [--run <run>]` → `{id, run, record, answer, actions, drafts, permissions, log}`. A
  run that fails before Claude Code starts (its prompt gone, no profile) is recorded too, its log saying why. Removing the
  plugin switches its routines off (an install again never switches one on); with its data, their settings and runs
  go too. Evidence: `plugins::routines::run`, `plugins::routines::outcome`, `jobs::forget_routines`

## Delivery

`ctx.deliver.provide(kind, {label, send(msg)})` for a kind in `uses.deliver_provider.kinds` (never `teams` or `mail`);
`ctx.deliver.ready(kind, true)` once it can send. Evidence: `jshost::provider_ready`, `plugins::may_send`

## Echo's data

`ctx.echo.read(kind, arg)` for kinds in `uses.echo.read`: `clients`, `threads`, `people`, `dictionary`, `sources`,
`source`, `calls`, `calendar`, `me`, `recording`; a `threads` row is `{id, name, client, stage, group, profile}` of an open thread (`group`:
`pursuit`, `delivery`; `profile`: the id of the AI profile its jobs and sync run on, the one pinned to it, else the default's (a call runs on its own pin, which may differ); never more of the profile). `ctx.echo.dictionary.add(...)` under `uses.echo.write: ["dictionary"]`. Evidence:
`plugins::ECHO_READS`, `share::dictionary_add`

| Read | Sheet ("Reads … in Echo") | Gives |
|---|---|---|
| `source` (`sources` too) | your calls' minutes, transcripts, meeting chat and who was invited, and the facts of your threads' mail and channels | `ctx.echo.read('source', 'call:<folder>' \| 'call:<stamp>' \| 'mail:<source>', opts?)` → `{id, kind, title, date, text, link, threads, client, profile}` (a call needs its minutes): `threads` where Echo put it, each `{id, name, client, client_name}`; `client` `{id, name}` when its threads have one client between them, else `null`; `profile` the AI profile its text may go to (a call's own; a mail's threads' when they share one, else `null`). **A call's** also `stamp` (a call is read by its folder's name or by its stamp, the call it names now; `id` is always its folder's), `started_at`, `ended_at`, `duration_min`, `app`, `platform`, `memo`, `attendees` (the invite's `{name, email}`, organizer first), `participants` (the same with `person`: their key in `echo.read('people')` — never someone it doesn't list, like a person seen only in closed threads — by address, by name only when the invite gives no address; `null` when Echo doesn't know them; 50 at most), `minutes`, `transcript` (as Echo writes it) and `chat` (`[{at, from, text, files}]`, `[]` when none) — `opts` `{transcript: false}` / `{chat: false}` leave those two out. `minutes`: `{summary, decisions, actions, questions, revision, body}` — `summary` the text under the heading with "summary" in it, `decisions` / `questions` the bullets under headings with "decision" / "question" — or their Ukrainian and Russian words, any case: "підсум…", "резюм…", "итог…", "сводк…"; "рішен…", "решени…", "решено"; "питан…", "вопрос…" (50 at most, 500 characters each), `actions` the call's action items as Echo keeps them `{id, text, owner, due, done}`, `revision` as `minutes_written`'s, `body` the minutes' Markdown from their first `## ` heading (without the title, date and who lines). **A mail conversation's or channel post's** (`kind` `mail` / `channel`): what Recall decided when it indexed it — `subject`, `at` (its last message's day, = `date`), `from` (who wrote the messages it holds), `people` (a mail's addresses but yours), `trust` `"trusted"` (Recall indexes only trusted senders' messages; one waiting for your decision, ignored or unauthenticated stays out), `url` (Outlook / Teams), and a mail's Microsoft 365 ids as Recall keeps them — `m365_id` (its last message's Graph id: `ctx.m365.mailThread(m365_id)` reads the whole thread, § Microsoft 365) and `conversation_id` (Graph's `conversationId`), each `null` for a channel post and for what Recall didn't keep (`m365_id` of a conversation it indexed before Echo kept it, until a new mail in it) — with `text` and `messages` (`[{from, at, text, link}]`, each message's section): the facts Recall's reader kept, never a message's own words (read those with `ctx.m365`, § Microsoft 365) |
| `sources` (`source` too) | as `source` | `ctx.echo.read('sources', {kind, thread, client, q, since, limit})` → newest first, no text: a call with minutes `{id, stamp, kind: 'call', title, date, threads, client}`; a mail conversation or channel post what `source` gives of it but `text` and `messages`, with `date` (= `at`) — `threads` here their ids. `kind` `call` / `mail` / `channel`, `thread` / `client` an id, `q` every word in its title (a mail's subject and senders too), `since` `YYYY-MM-DD`, `limit` 200 (1000 at most) |
| `calls` | the list of your calls (titles, times, who was invited with their addresses, their threads) | `ctx.echo.read('calls', {q, since, until, thread, client, limit})` → newest first, with or without minutes: `{id, stamp, title, date, started_at, ended_at, duration_min, app, memo, minutes, transcript, chat, people, participants, threads, client, profile}` (`minutes`, `transcript`, `chat`: whether it has them; `people`: the invite's names; `participants`, `threads`, `client` as `source` gives them); `q` every word in its title, folder, subject or people, `since` / `until` `YYYY-MM-DD` (both days in), `thread` / `client` an id, `limit` 50 (500 at most) |
| `calendar` | the meetings Echo has read from your calendar | `ctx.echo.read('calendar', {from, to})` → the agenda Echo last read (no request to Microsoft 365), by start: `{id, subject, start, end, organizer, attendees, join_url, response, threads, client}` (`threads`: the ids of the threads Echo puts it in; `client` `{id, name}` when they have one between them, else `null`); `from` / `to` `YYYY-MM-DD` by its start's day in your time zone (as Today's), both days in |
| `people` | who the people in your threads are (their names, addresses and organisations) | `ctx.echo.read('people')` → `{key, name, aliases, email, also, organisation, client, title, threads, places, reports_to, reports, link}` of everyone in your open threads: `aliases` the other names Echo knows them by (merged in People), `also` their other addresses, `client` their organisation's id; `key` is what a call's `participants[].person` names; `places` `[{thread, role, stance}]` where they stand in each of their open threads (`role`, `stance` as People keeps them, `null` when unset); `reports_to` `{key, name}` of the listed person they report to, else `null`, `reports` `[{key, name}]` of the listed people who report to them (never themselves) — People's own, no facts or quotes from calls (`share::people`) |
| `me` | your name and email | `ctx.echo.read('me')` → `{name, email}` (About me's; the email else Microsoft 365's account; `null` each when unknown) |
| `recording` | whether Echo is recording or transcribing a call now, and which | `ctx.echo.read('recording')` → `{state, call, stamp}`: `state` `recording` while Echo records a call or memo, else `transcribing` while a call's transcription runs (the newest such call), else `idle`; `call` the call's folder name and `stamp` its stamp (§ Events), both `null` when idle. Evidence: `share::recording` |

**About me** (`uses.me`, the host half only): `await ctx.me({kinds})` → `null` when About me (Settings → About me) is
missing, empty or still the template; else `{text, name, parts, hash}`. `kinds` are About me's parts: `identity` (name,
role, company, what the user works on), `people`, `terms`, `style` (how they like notes and follow-ups written),
`patterns` (how they work); none named: all but `patterns`, as Echo's own readers. A part is a block under a heading or
a `Label:` line, by its words; an unknown kind throws. `text` is the asked blocks in About me's order (20,000 characters
at most), `parts` each asked kind's blocks (a kind About me hasn't is left out), `name` its `Name:` line (else Echo's
name for the user, as `echo.read('me')`) when `identity` is asked, else `null`, and `hash` 8 hex characters of `text`:
another hash, an edit to read again. No event: read it when it is needed. `echo.read('me')` stays the way to name and
mail the user; `ctx.me` is the one to read what they wrote about themselves. Evidence: `share::about_me`

## Microsoft 365

Under `uses.m365.read` (`mail`, `chats`), through Echo's own Microsoft 365 sign-in — no sign-in of the plugin's, no
send; without Echo's sign-in each rejects "Microsoft 365 isn't connected: sign in to it in Echo's Settings → Calendar &
chat"; a kind not asked for rejects "… didn't ask to read your Microsoft 365 <kind> when it was installed" (`files` and
any other kind are named on the sheet as not had yet, never granted). The sheet: "Reads your Microsoft 365 mail and
Teams chats through Echo's sign-in when it asks: it searches them and reads a mail's thread with its attachments' names
or a chat's messages over some days — it never sends" (each part only for what it names).

- `await ctx.m365.search(query, {kinds, days, limit})` → `{query, results, errors}` as `echoctl m365 search`: its own
  kinds by default; `days` 90 (1–365), `limit` 25 a kind (1–50); an empty query gives no results.
- `await ctx.m365.mailThread(message)` (`mail`) → `{subject, picked, messages}`: the thread of a mail by its Graph id,
  oldest first, each `{id, from, to, at, text, url, …, files}` — `files` its attachments' names, inline pictures left
  out, never their contents; a list Graph refused is said in `files_error`.
- `await ctx.m365.chat(chat, {since, until})` (`chats`) → `{id, topic, members: [{name, email}], url, since, until,
  messages: [{id, at, from, text, files}]}`: what was written on those local days, both included (`until` today,
  `since` six days before by default; `YYYY-MM-DD`), oldest first, about 400 messages at most (whole pages of 50).

Evidence: `jshost::m365`, `plugins::describe_m365`, `graph::conversation_with_files`

What `uses.echo.write` may name, and the sheet's line for each (`plugins::ECHO_WRITES`):

| Write | Sheet | Calls |
|---|---|---|
| `dictionary` | Adds names to your Echo dictionary | `ctx.echo.dictionary.add` |
| `thread_stage` | Changes your threads' stage | `ctx.threads.setStage` |
| `tasks` | Adds and updates tasks in your threads | `ctx.tasks.upsert`, `ctx.tasks.close` |

Each is refused in a Yolo run on another thread than its own.

- `ctx.threads.setStage(thread, stage, reason)` → `{applied, why?}`. `stage` one of `lead`, `proposal`, `negotiation`,
  `kickoff`, `active`, `on_hold`, `wrap_up`, `done`, `lost`, `cancelled`; the stage history keeps the plugin's id,
  name and reason (≤ 300 chars). Once the user changes the stage away from what this plugin set last (a stage set before
  its first write doesn't count), `{applied: false, why: "the user set this thread's stage themselves"}` until the user
  lets it set the stage again (`echoctl project set <id> --stage-back-to <plugin>`, the Profile's button). The same
  stage again: `{applied: true}`, nothing written. A closed thread's stage is refused (it isn't among the threads it
  reads). Evidence: `projects::plugin_set_stage`, `projects::stage_held_from`
- `ctx.tasks.upsert(thread, {key, title, due?, owner?, state?, url?, reason?, propose?})` →
  `{task, applied, proposed, ignored, why?}`: the plugin's task of that `key` (≤ 120 chars, its own id), created in a
  story named after the plugin in the thread's first workstream (where the user merged that story, if they did; a new
  one if they deleted it), or updated. `title` ≤ 200 chars; `due` `YYYY-MM-DD` or
  null; `owner` a name ≤ 80 chars; `state` `open`, `in_progress`, `blocked`, `done`, `cancelled`; `url` https, shown as
  the task's link titled with the plugin's name. A field left out stays as it is. The same values again write nothing;
  `ignored` lists the fields the user set (kept as they are), and a task the user deleted or merged takes no write
  (`why`). Closing a key never added: `{task: null, applied: false, why: "no open task of that key"}`.
- `ctx.tasks.close(thread, key, reason, {propose})` — the task done, with why.
- `propose: true`: the write waits in the thread's Work → To review and the Inbox for the user's Yes (`task: null`
  until then for a new one); a newer proposal of the key replaces the pending one; after No the same write answers
  `{proposed: false, why: "the user said no to this"}` (a change proposes again); closing a key never added withdraws
  its proposal.

Each write is in the task's History as the plugin's, with its reason; the user can't undo a plugin's write (they
change or delete the task), and decides only the proposal still waiting. Evidence: `ledger::plugin_upsert`,
`ledger::plugin_close`

## Folders and files

`await ctx.system.pickFolder(prompt)` (needs `uses.folders`) gives a folder, read as `(root, relative path)`, in bulk
through `ctx.fs.readMany/walk`; with `folders.write`, `ctx.fs.write/move/remove` change it (never through a link,
never over another file). The picker lets the user make a new folder in it ("New Folder" on macOS, "Make New Folder" on
Windows), so a plugin that wants a new folder (a new vault) just asks for one; the folder made and picked is the one
given. A hidden folder or one inside it (`~/.claude`, a vault's `.claude/skills`) is never given ("… is a hidden
folder or inside one …: pick a folder that isn't hidden"); a folder holding them is. Every hidden folder is refused on
purpose, `<repo>/.claude/worktrees/x` or `~/.dotfiles` too; one granted by an older Echo is still read, never written
("… nothing in it is changed"). In a debug build
`ECHOCTL_PICK_FOLDER=<dir>` answers the picker. Other apps' files: as
named in `uses.files.read`, with `ctx.system.readFile(entry)`. Evidence: `app::choose_folder`, `grants::grant`, `plugins::app_file`

Hidden files and folders (a name starting with `.`) are read, never written — but for these, by their exact names
(`.Claude` is not `.claude`; a link or a Windows short name checked as what it reaches), which `ctx.fs.write`,
`append`, `move` (to and from), `remove` and `copyIn` and `ctx.git.commit` take as files, `ctx.fs.mkdir` and an empty
folder's `remove` as folders. A name refused (`community-plugins.json`, `core-plugins.json`) is refused in any case, as
APFS and NTFS find it. No sheet line of their own: `folders.write` covers them. Evidence: `grants::hidden_ok`
- files: `.gitignore` in any folder that isn't hidden; `.obsidian/<name>.json` in any folder that isn't hidden (a vault in a subfolder too: `knowledge/.obsidian/app.json`), the file right inside `.obsidian/`, its name ASCII; not `community-plugins.json`, `core-plugins.json` or `core-plugins-migration.json`, which switch Obsidian plugins on; Markdown in a
  skill of the folder's own Claude Code project, `.claude/skills/<name>/<file>.md` (`<name>` 1–64 of `a-z`, `0-9`, `-`;
  `<file>` not hidden, ending in `.md`), each Markdown that runs nothing (`plugins::claude_skills::skill_md_ok`): a frontmatter — YAML between leading `---` lines, also after a BOM or blank space — holds only `name`, `description`, `when_to_use`, `argument-hint`, `disable-model-invocation` and `user-invocable`, each a value on its line or a `|` / `>` text block (no other key: `hooks`, `allowed-tools`, `shell`, `model`, `context`, `agent`…; no nested map, list, tag or anchor; never malformed or unclosed), and nowhere a `` !` ``, a code block opened with `!` (```` ```! ````, `~~~!`), a `!$` or a CR alone: refused, never stripped ("a skill can't run commands: …"); only written whole by `ctx.fs.write` — `append`,
  `copyIn` and a `move` into `.claude/skills/**` are refused, and `ctx.git.commit` refuses a file there that fails the
  check;
- folders: `.obsidian` (in any folder that isn't hidden), `.claude`, `.claude/skills`, `.claude/skills/<name>`; a folder is never moved into a hidden
  place;
- everything else hidden is refused ("is hidden"): `.claude/settings*.json`, `.claude/hooks`, `agents`, `commands`,
  anything under `.claude` that isn't such Markdown (`SKILL.md.js` too), `.obsidian/plugins`, `themes`, `snippets`, `community-plugins.json`, `core-plugins.json`,
  `.git/**`, `.mcp.json`, `.gitattributes`, `.env`, a hidden note.

Files the user hands a plugin come as **handles** `{file, name, size}`: `file` an opaque id of that plugin's alone (another
plugin's is refused as one that isn't there), never the path; kept in the process's memory for an hour (1,000 at most).
No line on the sheet: handing a file over is the user's own act. Evidence: `jshost::files::make`
- From: `await ctx.system.pickFiles(prompt, {multiple, ext})` — the system's file picker (titled "<plugin> — <prompt>"),
  `[]` when cancelled; one file unless `multiple`; `ext` up to 20 extensions, letters and digits (`["md", "pdf"]`). Its
  window half's `ctx.onFileDrop` (§ Client half). `echoctl plugin call <id> <cmd> '<json>' --file <path>` (repeatable;
  refused where the CLI's limits for runs hold — a minutes run, a thread's sync, Yolo — never in a routine,
  `notes::run_limited`). No picker opens inside a run Echo started, nor from its
  `apply`, a timer's or an event handler's own call, or a widget's or work rows' provider. File names come from the user's disk: untrusted text, put into the page only
  through `ctx.ui.esc`. Evidence: `app::choose_files`, `cli::plugin::file_handles`
- `ctx.files.read(file, {max})` → its text (1 MB by default, 4 MB at most); `file` a handle or its id.
- `await ctx.fs.copyIn(file, root, path, {createNew})` → `{path, size}`: its bytes as they are (50 MB at most) into a folder
  it was given, with `folders.write`, as `ctx.fs.write` (never through a link, never a hidden file but those above); never over a file there — unless `createNew: false` (the default is `true`). Evidence:
  `jshost::files::copy_in`
- `ctx.fs.read(root, path, max)` → its text, `max` bytes (1 MB by default, 4 MB at most), bytes that aren't UTF-8 as
  U+FFFD; `ctx.fs.read(root, path, {max, strict: true})` throws "not UTF-8: <path>" instead (a character `max` cuts at
  the end is dropped, not refused). `await ctx.fs.readMany(root, paths, max | {max, strict})` → `[text | null]` in
  order (5,000 paths at most), `null` for a file it can't read — with `strict` also one that isn't UTF-8.
  Evidence: `jshost::folders::read_text`
- More of a folder it was given (each needs `uses.folders`; the writes `folders.write`, checked as `ctx.fs.write`). Evidence: `jshost::folders`
  - `ctx.fs.write(root, path, text, {ifSha, createNew})`: `ifSha` writes only while the file's sha256 is still that
    (a file gone or changed: "“path” changed since you read it: read it again", nothing written; a file over 64 MB
    refused before it is hashed); `createNew: true` never over a file, also when writers race ("is there already").
    `ifSha: null` said (not left out) means `createNew`, never an unconditional write; `ifSha` that isn't hex or
    null, or with `createNew`, is refused. Every `ctx.fs.write` takes the plugin's lock across processes.
  - `ctx.fs.append(root, path, text)`: `text` added at the end (a newline added when it has none; 1 MB at most), the
    file made when it isn't there, under the plugin's lock; never through a link, never into a FIFO.
  - `ctx.fs.mkdir(root, path)`: the folder and those above it.
  - `ctx.fs.stat(root, path, {sha})` → `{kind: file | dir | link | other, size, mtime, real, sha?}` or `null` when
    nothing is there. The last name is never followed: a link is `kind: link`; `real` is the canonical place relative to
    the root (the folders above followed, inside the root only), `null` for a link that leads out; `sha` (asked) for a
    file of 64 MB at most. No absolute path is ever given: the roots are those `ctx.fs.folders()` names (canonical),
    so nested or overlapping roots are compared on those.
  - `await ctx.fs.walk(root, {sha: true})` adds each file's `sha` (64 MB at most).
  - `await ctx.fs.search(root, query, {path, ext, hidden, max, caseSensitive})` → `[{path, line, text}]`: the lines
    holding `query` as typed (1–1,000 characters, no pattern; any case unless `caseSensitive`), in Rust off the
    plugin's time slice; 200 by default, 1,000 at most; 4 MB read of a file, a line's text cut at 300 characters;
    dot names (unless `hidden`) and links left out, 100,000 files looked at most.
  - `await ctx.fs.snapshot(root, {exclude, hidden, maxFiles, maxBytes})` → `{path, files, bytes, skipped, why:
    {large, unreadable}}`: the folder's files copied into the store's `cache/snapshots/<id>/` (`path`, a store path) to
    work on with `ctx.store`; no link followed, never a `.git` folder or `.obsidian/plugins` at any depth (with
    `hidden` too), nor the paths under the root `exclude` names (more than 100: refused); dot names only with
    `hidden`. Its bytes are capped at the smallest of 512 MB, `maxBytes` and 2 GB less what the plugin's snapshots
    already hold; its files at 20,000 or the lower `maxFiles`. Over either cap it is refused and nothing is left. A
    file over 50 MB, or one grown past it while it was copied (`large`), or one that can't be read — gone since it was
    listed, not allowed, locked (`unreadable`) — is left out, nothing of it kept, and counted in `skipped`. One
    snapshot is made at a time per plugin, and it keeps 3 at most: a fourth, or one when its snapshots already hold
    2 GB, is refused until it removes one.
    `ctx.store.remove(path)` removes it; Echo sweeps it with `cache/` (Host § Store).
- Git in a folder it was given (each needs `uses.folders`; `init` and `commit` also `folders.write`; all async, off its time slice). No line on the sheet beyond the folder's. Evidence: `jshost::git`
  - `await ctx.git.info(root)` → `{repo: false}` or `{repo: true, branch, head, detached, busy, subfolder}`: `branch`
    null when detached; `head` the commit's id, null before the first commit; `busy` `merge`, `rebase`,
    `cherry-pick`, `revert`, `bisect` or null; `subfolder` true when the folder sits inside a larger repository.
  - `await ctx.git.status(root, {paths})` → `[{path, code, from?}]`: the folder's changed and untracked files (or those
    of `paths`, 1–1,000), relative to the root; `code` git's two letters (porcelain v1: `" M"`, `"A "`, `"??"`, …), `from`
    a rename's old path. Ignored files left out.
  - `await ctx.git.log(root, {path, max})` → `[{rev, author, email, date, subject, echo, path?}]`, newest first: the
    commits touching the folder, or the file at `path` through its renames (each with its `path` then); 50 by default,
    1,000 at most; `date` ISO 8601; `echo` the id of the plugin that committed it through Echo, else null.
  - `await ctx.git.show(root, rev, path, {max, strict})` → `{text, size, truncated}` (bytes that aren't UTF-8 replaced, or with `strict: true` refused as "not UTF-8", as `ctx.fs.read`) or null when the file wasn't there: its
    text at that version as the repository holds it (no filter), cut at `max` (1 MB by default, 4 MB at most); `rev`
    `HEAD`, `HEAD~N` or a commit's id (4–64 hex), nothing else (the id of a file or a folder is refused); never a file
    of the repository outside the folder.
  - `await ctx.git.init(root, {branch})` → its `info`: a new repository in the folder, on `main` unless `branch`
    (letters, digits, `.` `_` `-` `/`), with no template; refused inside a repository.
  - `await ctx.git.commit(root, {paths, message})` → `{rev}`, or `{rev: null}` when they hold no change: one commit of
    exactly `paths` (1–1,000 files, each checked as `ctx.fs.write` checks it) as they are on the disk, a file gone
    committed as removed, the user's other staged and unstaged work left as it is; `message` plain text, 1–5,000
    characters. Committed by `Echo (<plugin id>) <<plugin id>@plugins.echo.invalid>`; its author the user's git name
    and address when git knows both, else that too. No hook runs, nothing is signed. Refused while a merge, rebase,
    cherry-pick, revert or bisect is under way, and for a file the repository ignores.
  - Refused: no git where the system installs it or on the user's `PATH` ("git isn't installed on this computer"; a
    `PATH` folder inside a folder given to a plugin or inside Echo's own is skipped), git older than 2.31, a repository
    whose history lies outside its folder (a `gitdir:` file, a linked worktree or submodule, a link as or in `.git`),
    one setting `core.worktree`, a bare repository (`HEAD`, `objects` and `refs` not in a `.git` folder) or one
    another user owns, one that borrows objects (alternates), is a partial clone, whose settings can't be read
    whole (over 16 MB, a name that isn't UTF-8), or whose settings read or include a file a plugin could change (in a
    folder given to a plugin, in Echo's own, or in the work tree outside a hidden folder — there or not); git taking over 20 s is stopped. Nothing is fetched. Nothing a repository's settings, hooks or attributes name
    runs (Host § Git).
- Its window half shows images of a folder it was given (with `uses.folders`) from
  `plugin/<id>/folder/<encodeURIComponent(root)>/<path>` (`root` as `ctx.fs.folders()` names it): png, jpg / jpeg, gif and
  webp only (no svg), 10 MB at most, inside the folder once links are followed. Evidence: `window::plugins::folder_image`

## Thread values and shared values

- `ctx.threads.set(thread, key, value)` — `key` 1–80 chars, `value` JSON of 16 KB at most, 64 keys a thread, 4 MB a
  plugin in all; `null` removes it; refused for a thread that doesn't exist. `ctx.threads.get(thread, key)` (null when none), `ctx.threads.list(key)` (`[{thread, value}]`,
  threads gone left out). No grant needed. A `threads.json` that isn't JSON makes `get` and `list` fail and is moved
  aside as `threads.json.bad-<time>` at the next change; a thread id used again inherits nothing. A thread's changes
  arrive as `threads_changed` (§ Events). Evidence: `plugins::thread_values::changed`
- `ctx.clients.set(client, key, value)` — the per-client twin of `ctx.threads` values, by a client's id (as
  `ctx.echo.read('clients')` and the `client.profile` slot give it): the same limits (`key` 1–80 chars, `value` JSON of
  16 KB at most, 64 keys a client, 4 MB a plugin in all); `null` removes it; refused for a client that isn't there (an
  organisation that isn't a client included). `ctx.clients.get(client, key)` (null when none), `ctx.clients.list(key)`
  (`[{client, value}]`, clients gone left out). No grant needed. Only the plugin's own keys: a key with a dot is
  refused (no shared values per client), and Echo's own client fields (name, domains, …) are never reached through it.
  Kept when the client is renamed (its id stays); dropped when it is removed, so a client later under the same id
  inherits none; no event is told. The window half doesn't read them itself: it asks its host half (`ctx.call`).
  Evidence: `plugins::client_values`
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
  `null`. The sign-in grants no network: the API host goes
  in `network.hosts`. `ctx.signin.get(host, {fresh: true})` asks again (after a 401). `ctx.signin.sources(host)` →
  `{gh, git}` says which programs are here, no sign-in read. Evidence: `plugins::signin::get`, `plugins::may_use_signin`
- With `uses.signin.keep: true`, `ctx.signin.keep(host, token)` (one line, 8 KB at most) has Echo keep a sign-in the
  plugin made itself (its own client id and scopes, a device code or browser sign-in); `ctx.signin.get(host)` hands it
  back first as `{token, from: "echo"}`, to that plugin only; `ctx.signin.forget(host)` signs out. It goes when the
  plugin is removed; the same id from another marketplace, or a version that no longer says `keep`, starts without it;
  After a 401 on an `"echo"` sign-in,
  `{fresh: true}` gives the same one. Evidence: `plugins::signin::keep`

## Claude in Echo: skills, commands and `agent/`

- **Package skills**: a folder in the plugin (`skills/<name>/SKILL.md`, with `scripts/` and `references/` beside it as
  needed) named in `skills` (at most 10, with the runtime skills); the folder's name a–z 0–9 `-`, ≤ 40 chars, once;
  `SKILL.md` Markdown that runs nothing, as any skill a plugin writes (`skill_md_ok`: no `` !` ``, `!` code block, `!$`
  or CR alone), its frontmatter first and only `name` (the folder's name) and `description` (one line, ≤ 1024 chars). They need no `uses.sessions` (the sheet names them from `skills`). The folder session, routines and Yolo
  name each `<prefix>-<name>` through `.plugins`; a plugin not laid out keeps `echo-<id>:<name>` from its
  `--plugin-dir` copy. Evidence: `plugins::package_skills`, `grants::copy_package_skills`
- **Runtime skills**: text built at run time, `ctx.sessions.skills = [{name, description, body}]`; Echo writes the
  frontmatter and the plugin around them; the text runs nothing (`skill_md_ok`). One name is never both a package and a
  runtime skill.
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
  Claude reads before working with it; entries not given again are removed. Evidence: `plugins::agent::set_runtime`

## Scripts

- Declared in `uses.scripts: [{skill, script, timeout?}]`, 1–20 a plugin, each once: `skill` one of its
  package skills; `script` a file in that skill's folder, as a relative path of plain names (letters, digits, `.` `_`
  `-`, none starting with a dot), ≤ 200 chars; `readonly` (true or false), from before every routine acted on its own, is still
  accepted and changes nothing; `timeout` the seconds it may take, 1–3600, 300 by default (the
  sheet names one over 300). Evidence: `plugins::scripts::declared`
- Interpreter: the shebang (`#!/usr/bin/env node`); on Windows a `.ps1` runs with PowerShell, another script with the
  interpreter its shebang names. Echo ships no interpreter; a missing one fails the run with its message.
- Environment: the user's environment minus Echo's secrets (`ECHOCTL*`, but for `ECHOCTL_HOME`, `ECHOCTL_CONFIG_DIR`
  and `ECHOCTL_BIN` set to the running Echo's, so an echoctl it starts acts on the same data, and inside a routine's or
  Yolo's run its markers, so that echoctl is limited and recorded as the run) and every `CLAUDE*` / `ANTHROPIC*` key;
  a PATH of the app's plus the usual program folders (Homebrew, `~/.local/bin`, nvm's newest version);
  `ECHO_PLUGIN_SCRIPT=1` (nobody is there to answer a prompt; an echoctl it starts refuses every change to Echo plugins:
  installs, updates, removals, switching on or off, their settings, marketplaces, certificates) and `ECHO_PLUGIN_CACHE` (the store's `cache/`, never the
  store itself). No stdin; it runs in its copied skill folder. Evidence: `plugins::scripts::run`,
  `plugins::scripts::run_markers`
- Arguments: at most 100, each ≤ 32 KB with no NUL, passed as plain words, never through a shell.
  Evidence: `plugins::scripts::run`
- In a session: `SKILL.md` names a script by its path relative to the skill (`scripts/read.mjs <args>`); in Echo's
  sessions that path is Echo's launcher of it, allowed only on its own by its full path. A session's run is bounded by Claude Code's Bash time (10 min at most).
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

- `ctx.widgets.provide(id, ({size, height, fits, params}) => data)` — Echo asks it in the background, answered within
  5 s, again after `refresh` and when its size or height changes; `height` is the rows of the grid it takes there (1,
  2) and `fits` what Echo draws of it at that size and height, `{rows, items, bars}` (§ Data by kind): send that many
  — the most important first for a table or list, the last for bars — and count the rest yourself; or `ctx.widgets.set(id, data)` pushes it, and `ctx.widgets.clear(id)` takes the widget off Today and
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

No widget scrolls inside the page: the wheel over it always scrolls Today or the dashboard. A table or list draws what
its size and height hold (below), then "+N more" — a click shows all of it in place, "Show less" folds it back; a
`fill` widget is at least its line's height and grows past it with what it shows; a wide table scrolls only sideways
(hide columns at M with `hide`). At height 2 every count doubles.

Text is plain (Echo escapes it); a text field given as anything but a string or a number is refused, text past its
limit cut. `tone` is `good`, `warn` or `bad` (a table's cell and a chart's series also `muted`). A list's or card's
`link` goes only to `echo://`; a table row's and a `stats` item's `link` also to `https://`.
Evidence: `pluginWidgetHtml`, `pluginTableHtml` (window script)

| Kind | Fields | At S / M / L | Characters kept | Refused |
|---|---|---|---|---|
| `stat` | `{value, unit?, label?, tone?, note?, progress?: {used, of}}` | `note` hidden at S | value 24, unit 12, label 80, note 200 | no `value`; `used` or `of` not a number |
| `stats` | `{items: [{label, value, tone?, trend?, link?}]}`; `trend`: numbers, the last 30 drawn as a sparkline under the value (none with fewer than 2) | first 2 / 4 / 6 items | value 24, label 60 | `items` not a list; a `trend` not a list of numbers |
| `chart` | § Charts | § Charts | § Charts | § Charts |
| `list` | `{items: [{title, sub?, link?}], empty?}` | the first 3 / 5 / 10 items a row of height (`fits.items`), then "+N more" (100 at most) | title 200, sub 200 | `items` not a list |
| `status` | `{state, title, text?}`, `state` `ok`, `warn` or `bad` | the same | title 80, text 300 | another `state` |
| `card` | `{title?, text?, link?}` | the same | title 120, text 2000 | neither `title` nor `text` |
| `table` | `{columns: [{key, label, kind?, hide?}], rows: [{id, cells: {<key>: text, number or {text, tone?}}, link?, group?, detail?: [{text, sub?}]}], groups?: [{id, label}], note?, empty?}`; column `kind` `text` (default), `number`, `pill`, `date`, `spark`, `heat` or `mix` (§ Scorecard tables), `group?` a band over a run of its heads; `total?: {label?, cells}` a bold row under them; rows under their group's header in `groups`' order, the rest last; a row's `link` lands on its first text column | M and L only; a column with `hide: "M"` left out at M; the first 8 rows a row of height (`fits.rows`; in the order drawn, a group's header and count kept whole), then "+N more" | cell 200, column label 40, group label 120, detail line 200, `note` 120 (beside the label) | 0 or over 10 columns (and over 12 `heat` columns besides); a column key not `[A-Za-z0-9_-]{1,40}`, or twice; over 200 rows; a row `id` missing, over 80 chars or twice; a `group` not in `groups`; over 50 groups; over 6 detail lines; a `number` cell not a number; a `date` cell not `YYYY-MM-DD` |
| `page` | the plugin's, drawn by its client half (§ Client half) | the draw gets `size` | the plugin's | — |

### Scorecard tables

A `table`'s columns may also be:

- `spark`: a cell `{values: [2–30 numbers], text?}` — a small trend, its last point dotted, `text` (24) beside it.
- `heat`: a number, shaded by its share of the column's largest (of all its rows, shown or behind "+N more"); 0 or
  none a muted "·"; below 0 refused. Up to 12
  `heat` columns besides the 10 (a heat map's weeks); give them one `group` for a band over their heads.
- `mix`: a cell `{parts: [1–6 {key?, value ≥ 0, tone?}], text?}` — a thin 100% bar of its parts (a zero not drawn),
  `text` beside it, its parts in its hover; colours from the column's `series: [≤ 6 {key, label, tone?}]` (Echo's
  palette in order, or the tone; keys `[A-Za-z0-9_-]{1,40}`, unique), named in a legend under the table; a part's own
  `tone` wins, and with `series` given a part names one of them (or has a `tone`).
- Numbers in all three: finite, at most 10^15 either way.

`total: {label?, cells}`: one bold row under the rows, never folded away ("Total" in its first column unless that
cell is given); its `heat` cells plain numbers. Evidence: `pluginTableHtml` (window script)

### Charts

`{type, points, series?, unit?, note?, percent?, center?, headline?}`; values are numbers up to 10^15 (larger is
refused). All series share one y-axis and the `unit`. Evidence: `pluginChartHtml` (window script)

| `type` | Draws | Points drawn | Series | Negative values |
|---|---|---|---|---|
| `line` | a line | the last 60 | 1–6 | below zero |
| `area` | a filled line | the last 60 | 1–6 | refused |
| `bar` | upright bars | the last 24 | 1–6 | below zero |
| `stacked` | upright stacked bars | the last 24 | 1–6 | refused |
| `bars` | horizontal bars | the last 4 / 6 / 12 at S / M / L a row of height (`fits.bars`) | 1–6 | refused |
| `stacked-bars` | horizontal stacked bars | the last 4 / 6 / 12 at S / M / L a row of height (`fits.bars`) | 1–6 | refused |
| `pie`, `donut` | slices | all read; up to 6 drawn in the given order, those under 3% and past the 6th folded into one Other after them (all zero: "Nothing to show yet") | one | refused |
| `funnel` | stages, drawn as a funnel: each centred, as wide as its share of the widest, narrowing to the next (at S plain centred bars) | the first 8 in the given order, each with its share of the one before | one | refused |

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

### Dashboards it offers

`dashboards: [{id, kind, name, about?, for?, items: [{widget, size?, height?, row?, params?}]}]` — at most 6, each
offered in the "+" beside a thread's (`kind: "thread"`) or a client's (`"client"`) dashboards ("From plugins", "<name> —
<plugin>") and in `echoctl dashboard presets`; the user adds one (`echoctl dashboard add-preset <plugin> <id>`), once.
Echo never adds one itself, and the install sheet only says so ("Offers dashboards: Staffing (clients)"): not a
permission, an update adding one doesn't ask again.

- `id` a–z 0–9 `-`, unique across the plugin's dashboards; `name` 1–40 chars (the dashboard's; when the user has one of
  that name, the next free "<name> 2", "<name> 3"…); `about` ≤ 160 (the row's tooltip).
- `for` (a thread's only): the stage groups it shows on — `pursuit`, `delivery`, `closed`; none: every thread.
- `items`: 1–24, each widget once — the plugin's own by its id, or `plugin:<id>:<widget>` of a plugin of its
  marketplace. A widget has a setting of the page's kind (`thread` / `client`), which the page sets: never in `params`.
  `size`, `height` and `params` are ones the widget takes (unset: its first size, 1 row, its defaults); `row`: it starts
  a row. Its own widgets are checked at install; others' by `plugin check` beside them, and on a page a widget of a
  plugin not installed draws nothing.
- Updated: a dashboard added from a preset whose widgets or `for` the plugin's new version changed says "<plugin> updated this dashboard —
  Update / Keep mine" ("— Update replaces your changes" when the user changed it); Update takes the new layout (and
  `for`), Keep mine keeps theirs and that version isn't offered again (`echoctl dashboard preset-update <id> [--keep]`).
- Its plugin off or removed: the dashboard stays the user's, saying so, with Remove.
- `echoctl dashboard export <id> --thread|--client` prints a dashboard as such a block (widgets in full, their thread /
  client settings left out) — a preset for a manifest, or a file for `echoctl dashboard import <file>` (the user's own
  then; its widgets named in full: `plugin:<id>:<widget>` or Echo's `core:…`). Evidence: `plugins::presets::check`, `boards::add_preset`, `boards::status`

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

- The client half exports `apply(ctx)`; its `ctx`: `call(command, args, {files})` (the host half's commands; `files`:
  handles it was given, passed to the command as its own; sent as `POST /api/plugin/<id>/call/<command>` with
  `{args, files}` as its JSON body, 4 MB at most — more is refused, "too much for one plugin call: 4 MB at most"; any
  other method is refused; the plugin installed and on), `onFileDrop(fn)` (macOS: files dropped on the window while its
  activity or its Knowledge tab is shown go to `fn(files)` as handles — names and handles, never paths; a name is untrusted text, drawn only through
  `ctx.ui.esc`; with no such view
  shown, or on Windows, the drop is Echo's own), `listen`
  (a document listener), `listenWindow` (a window listener), `onUnload(fn)` (its own cleanup: timers, observers),
  `slots.list` / `slots.add` on a known slot, `panels.add`, `showPanel(view)` (one of its panels),
  `claude.session(opts)` (Claude Code in a folder it was given, on the user's click: § Claude runs),
  `claude.installSkill(name, md)` / `claude.removeSkill(name)` (a skill in the user's own Claude Code, on the user's click: below),
  `system.reveal(root, path)` (a file of a folder it was given shown in Finder / Explorer, on the user's click: below),
  `openEchoPanel(view)` (one of Echo's), `activities.add`, `settings.add` / `section`, `palette.add`, `widgets.draw`,
  `where()` (the activity and Settings section shown, and `recall`: whether Recall is on), `go(hash, {replace}?)` (`replace: true`: the address changes without a history step, so Back skips it, and a hashchange fires, so the window shows the page as on a real one; without it a new history step; the hash it is already at only redraws), `render()`, `markdown(md, opts)` and
  `markdown.links(md, {detail})` (§ Host half; `async`, by `POST /api/plugin/<id>/markdown` and `/markdown/links` with `{md, opts}`,
  4 MB at most, as `call`) and `ui` (`esc`, `patch(el, html)`
  (replaces a box's HTML unless it is what `patch` last wrote there: redrawn the same, its focus, typed text and scroll
  stay), `setRow`, `setCard`, `sw`, `ICON`,
  `relDay`, `ago`, `notice`, `copy`, `style(css)`, `stylesheet(path)` (a `.css` file of its package by a plain relative
  path whose parts don't start with a dot, resolved once loaded; anything else is refused; what a plugin adds after it was
  unloaded is refused), `graph`, `newGraph`, `sheet(html, opts)`, `closeSheet()`). Echo takes out what a plugin added when
  it unloads, and runs its `onUnload` cleanups first (one that throws is logged, the rest still run). Evidence:
  `pluginCtx`, `loadPluginClients`, `pluginStylesheetHref`, `unloadPlugin` (window script)
- `ctx.echo.routines` (manifest `uses.echo.jobs`): Echo's jobs in the routines' one shape (§ Routines).
- `ctx.echo.routines.offer(ids?)` (no capability: its routines are on its sheet already): Echo's sheet of the plugin's own
  routines over the window, in a plugin sheet's frame — those `ids` name (its manifest's routine ids, shown in its
  order) or all — each with Echo's switch, name, when it runs (in plain words, as on the install sheet) and `what`, the line "Each runs Claude Code on its own
  while it is on, as its sheet in Settings → Plugins says; you can switch it off there any time." and Done. `async`: it
  rejects, before anything is shown, unless the page has the user's click or key (`copyAllowed`), and on an id it
  hasn't ("the plugin has no routine “x”") or when it has none. Only the user switches them there (a click the page's
  script makes moves nothing), each saved as its row in Settings → Plugins saves it; the plugin never switches one. It
  resolves once the sheet closes (Done, Esc, a click beside it, another sheet, the address changing) to `[{id, name,
  on}]` (`id` the routine's own), each as Echo has it then; the plugin switched off, updated or removed closes it
  without resolving. Evidence: `routineOffer`, `routineOfferPick` (window script)
- `ctx.echo.jobs` (manifest `uses.echo.jobs`, the plugin on): `list()` — what `echoctl jobs` prints (every job, where
  it shows, its fields with value, default and range, its last run, next run or what it waits for, the last 30 days by
  day with cost, today's counts, and the night with Echo's default); `runs(job, {limit})` — its runs, newest first, as
  `echoctl jobs runs` (20 by default, 500 at most); `set(job, ["key=value", …])` — only on the
  user's click or key (`copyAllowed`, else it rejects before anything is sent), sent as the page's IPC command
  `plugin_jobs_set` (never a GET; Echo answers by `window.echo.jobsSet`), checked as `echoctl jobs set` (`key=` back to
  its default, `night` with `from` / `to`; a plugin's routine's `on` refused), resolving to the list after (a plugin's
  routine: that routine in the routines' shape). The list is `/api/plugin/<id>/jobs`. Both check
  the plugin the request names (on, `uses.echo.jobs`); the click is checked in the page, which every client half shares
  with the user's rights. Echo's jobs run on their own settings with or without the plugin. Evidence:
  `jshost::may_tune_jobs`, `window::plugins::handle`, `jobs::set`
- `ctx.microphone` (manifest `uses.microphone`, the plugin on): `open(audio)` → a `MediaStream` (`audio`: getUserMedia's
  audio constraints, `true` by default; no video); `close()`; `isOpen()`. `open` throws, before anything is sent, unless
  the page has the user's click or key (`copyAllowed`) and the address shows the plugin's own view (its activity
  `#<plugin>:<id>…` or its Knowledge tab `#brain/<plugin>:<id>…`), nothing records and no other plugin has the
  microphone; then the page asks Echo (`POST /api/plugin/<id>/microphone`, any other method refused), which checks the
  plugin (on, its sheet naming `uses.microphone`), that no recording runs (a call or a memo: "Echo is recording — a
  plugin gets the microphone once the recording stops") and that no other plugin holds it ("<name> has the microphone
  now"), and answers `{value: <its name>}` or `{error}`; only then does the page call `getUserMedia`. While it is open the window's top
  pill reads "<name> hears your microphone" with Stop, and the tray shows the recording icon and "● <name> hears your
  microphone". It stops — tracks stopped, the page's IPC command `plugin_mic` — on Stop, `close()`, the address leaving its view,
  the plugin switched off, updated or removed (`unloadPlugin`), the page loaded again, every track ended, or a
  recording starting (the page says "Echo started recording, so the plugin's microphone stopped"). One plugin at a
  time; calling `open` again while open returns the same stream. The native window refuses the camera and a page's own
  `getUserMedia` outside an opening; screen sharing (`getDisplayMedia`) is not one of Echo's capabilities and Echo's
  permission gate doesn't decide it (Echo's side: `echo-host.md`, a plugin's microphone). Evidence: `micRefusal`, `micOpen`, `micClose`
  (window script), `window::plugins::mic_open`
- Slots: `call.menu`, `calendar.event`, `task.actions`, `thread.profile`, `client.profile`, `call.row`, `call.header`,
  `call.minutes`, `knowledge.tab` and `knowledge.settings` (the last two change with Knowledge's rework). A
  `knowledge.tab` is all the Recall activity shows: the activity is there only while a plugin adds one, and an address
  `#brain…` loaded before the tab came is routed again when it comes; its last tab gone and none back within 1.5 s (a plugin updating), Recall opens Calls. Evidence:
  `SLOTS` (window script)
- A slot draws only the HTML its plugin gives (its look ships in the plugin: its own CSS by `ctx.ui.style` /
  `stylesheet`, or Echo's generic classes such as `btn`, `menu`, `menu-item`); every text in it through `ctx.ui.esc`.
- `ctx.slots.add('call.menu', {label, run(target), when?(target), icon?})`: an item in a call's ⋯ menu, `target`
  `{kind: 'call', call}` (the call as the page has it, with its `stamp`). `label` is a text or a function of the target (``(t) => `Open in ${…}` ``,
  synchronous; a `ctx.render()` inside it ignored; one that throws or gives nothing leaves the item out).
  Each plugin's items are a group after a separator, under Echo's own items and above Delete. `calendar.event` and
  `task.actions` take a label function too. Evidence: `slotItems`, `slotLabel` (window script)
- `ctx.slots.add('call.header', {id, label, render(call) → HTML, after?(box, call)})`: a box of the plugin's own on a
  call's header, before its My notes and ⋯ buttons — a chip, a button and a popup it draws itself (the box is
  `position: relative`, so a popup anchors to it). `call` is `{name, stamp, title, started_at, notes, profile, memo, threads,
  client}`: the call's folder name, its stamp (`YYYY-MM-DD_HHMMSS`, kept when the call is retitled: key a record by it), its title, its start, whether it has minutes, its AI profile (or null), whether it is
  a memo, its threads' ids and the first of their clients (or null) — only what the page has; the plugin keeps its own
  record of the call (its store, by `name`). As `thread.profile`: `render` synchronous, `after` once per box drawn
  afresh (again for another call), one per id, one that throws left out (logged) and the header drawn without it.
  Evidence: `slotCallPage`, `callHeadBoxes`, `callSlotAfter` (window script)
- `ctx.slots.add('call.minutes', {id, label, render(call) → HTML})`: a line in the box that says the call's minutes are
  coming, drawn only while they are (a run queued or going; Echo writing them by itself: due now or at a time, the call
  not over or not transcribed yet, a Claude Code session holding it) — not when they are off, held ("Until I say"), too
  thin, can't start, failed, or written. `call` as `call.header`; a line with nothing is not drawn; one that throws is
  left out. Evidence: `minutesComing`, `callMinutesLines` (window script)
- `ctx.ui.sheet(html, {label, onClose})` opens a sheet: the plugin's HTML in Echo's dialog frame (`td-dialog`, a
  `role="dialog"` with `aria-modal` named `label`) over the window, the keyboard moved into it (an `[autofocus]`
  element, else the first it can reach, else the sheet) and Tab going round inside it. One sheet at a time: opening
  one closes the sheet open (its plugin's or another's). Esc, a click beside it, or `ctx.ui.closeSheet()` (its own
  only) close it and call `onClose` once; its plugin switched off, updated or removed closes it without `onClose`.
  Closed, the keyboard goes back where it was (from a menu's item: to the menu's button). It returns the sheet's box;
  its clicks go through `ctx.listen`. Echo's redraws never replace it (its own holder after `#r-over`); the palette
  and Echo's own dialogs open over it. Evidence: `sheetShow`, `sheetShut`, `sheetOpen`, `sheetClose`, `sheetTab`
  (window script)
- `ctx.activities.add({id, label, icon, render() → HTML, after?(box), badge?(), editing?()})`: `badge()` gives a count
  (shown on its icon as Threads shows its count: 1–99, more as "99+") or `'!'` (Settings' attention mark); anything
  else shows none. `editing()` true: Echo leaves the activity's box as it is on its redraws (a refresh, any plugin's
  `ctx.render()`) — no `render`, no `after` — until it says otherwise; otherwise the box is replaced only when its HTML
  changed (`patch`). Asked on every draw, synchronously (a `ctx.render()` inside ignored); one that throws is logged once
  for that activity and taken as no badge / not editing. Evidence: `actBadgeText`, `actAsk`, `actHeld`, `actBadgeHtml`
  (window script)
- `ctx.ui.copy(text)` puts text on the clipboard on the user's click or key (without the page's user activation, or
  where the engine can't tell, it throws), 10,000 characters at most (the rest cut).
- `ctx.system.reveal(root, path)` (manifest `uses.folders`, the plugin on; client half only — the host half's
  `ctx.system` has no `reveal`): shows a file or folder of a folder the user gave the plugin selected in the file manager — `root` as
  `ctx.fs.folders()` names it, `path` relative inside it (`''` the folder itself; hidden files and folders too, as
  they may be read). `async`: it rejects, before anything is sent, unless the page has the user's click or key
  (`copyAllowed`); then the page asks Echo (`POST /api/plugin/<id>/reveal` with `{root, path}`, any other method
  refused), which checks the plugin (installed and on, its sheet naming `uses.folders`), resolves the path by
  `grants::resolve` (a folder it was given; relative, plain names; inside it once links are followed — a link out
  refused) and that it is there, and answers `{value: true}` or `{error}`. Echo runs, never through a shell, macOS
  `/usr/bin/open -R <path>`, Windows `explorer.exe` with its one argument `/select,"<path>"` (a path holding `"` or a
  control character, or a `\\?\` path, is refused; a closing `\` left off). On test data
  (`ECHOCTL_TEST_OPEN_LOG_ONLY`) it opens nothing and logs `reveal <path>` to the open log. Evidence:
  `window::plugins::reveal_target`, `app::reveal_command`
- `ctx.claude.installSkill(name, md)` / `ctx.claude.removeSkill(name)` (manifest `uses.skills: true`, the plugin
  on; client half only — the host half's `ctx.claude` has neither): a skill in the user's own Claude Code, not Echo's
  sessions — `<Claude Code folder>/skills/<name>/SKILL.md`, the folder `CLAUDE_CONFIG_DIR` when Echo has it, else
  `~/.claude`. `async`: each rejects, before anything is sent, unless the page has the user's click or key
  (`copyAllowed`); then the page asks Echo (`POST /api/plugin/<id>/install-skill` with `{name, md}`,
  `/remove-skill` with `{name}`; any other method refused), which answers `{value: true}` or `{error}`. `name` 1–64 of
  `a-z`, `0-9`, `-`; `md` the whole `SKILL.md`, not empty, 256 KB at most, Markdown that runs nothing (`plugins::claude_skills::skill_md_ok`): a frontmatter — YAML between leading `---` lines, also after a BOM or blank space — holds only `name`, `description`, `when_to_use`, `argument-hint`, `disable-model-invocation` and `user-invocable`, each a value on its line or a `|` / `>` text block (no other key: `hooks`, `allowed-tools`, `shell`, `model`, `context`, `agent`…; no nested map, list, tag or anchor; never malformed or unclosed), and nowhere a `` !` ``, a code block opened with `!` (```` ```! ````, `~~~!`), a `!$` or a CR alone: refused, never stripped ("a skill can't run commands: …"), written whole (replacing the file, never
  through a link); nothing else of a skill is written. A skill folder that is there already is never changed unless
  this plugin added it — Echo keeps who added which in `<config dir>/plugins/claude-skills.json` (`{name: plugin id}`),
  never in the skill; a link in its place, or as its `SKILL.md`, is refused. `removeSkill` takes out only a skill this
  plugin added: its `SKILL.md`, and its folder when nothing else is in it. Removing the plugin (`echoctl plugin
  remove`, with `--delete-data` or not) takes out every skill it added. On test data (`ECHOCTL_TEST_OPEN_LOG_ONLY`) it
  writes only to the test's own `CLAUDE_CONFIG_DIR`, refused when that isn't set. Evidence: `plugins::claude_skills`
- `ctx.widgets.draw(widget, ({data, size, height, fits, theme}) => HTML)` draws a `page` widget from `data` (given by `provide` /
  `set`). Evidence: `pluginWidgetView` (window script)
- `ctx.slots.add('thread.profile', {id, label, render(thread) → HTML, after?(box, thread)})`: `thread` is
  `{id, name, client}`, drawn under the thread's Sources; `render` is synchronous (a `ctx.render()` inside it is
  ignored), `after` runs once per box drawn afresh (fetch there, then `ctx.render()`), one row per id, and one that
  throws is left out, the Profile drawn without it. Evidence: `threadSlotRows`, `threadSlotAfter` (window script)
- `ctx.slots.add('client.profile', {id, label, render(client) → HTML, after?(box, client)})`: as `thread.profile`, on a
  client's Profile under its Threads; `client` is `{id, name}`. Evidence: `threadSlotRows`, `slotClient` (window script)
- `ctx.slots.add('call.row', {id, label, chip(call) → text})`: a chip on each call's row in the calls list, in its
  second line, `call` `{name, stamp, title, started_at, status, memo}`; `chip` is synchronous and gives a short text (24
  characters, the rest cut with …) or nothing (no chip), drawn as text (Echo escapes it) with `label` as its tooltip;
  a click on it opens the call as the row does. One per id; one that throws is left out, the row drawn without it; a
  `ctx.render()` inside it is ignored. Evidence: `callRowChips` (window script)
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
- In a development build `ECHOCTL_TEST_GRAPH=http://127.0.0.1:<port>` (or `http://localhost:<port>`) is a fake
  Microsoft 365: Echo counts as signed in (the account About me's address) with a made-up token, sends every Graph
  request of `ctx.m365` (and the rest of Echo's) to `<base>/<path after /v1.0>`, and refuses the real sign-in. It fails
  closed: a value that isn't a local address is no sign-in ("Microsoft 365 isn't connected"), and a request to any
  other Microsoft host is refused. `tests/fixtures/fake_graph.py <port>` serves made-up acme.example mail and a chat for
  `search`, `mailThread` and `chat`. A release build never reads it. Evidence: `graph::test_graph`
- Made-up `gh` / `git` first on `PATH` (`gh.cmd` / `git.cmd` on Windows) answer `ctx.signin.get` in any build
  (`tests/app_profile.md` → Plugins against fake services).
- `ECHOCTL_ASK_ADDRESS=<address>` answers `ctx.http.askAddress` in a debug build (no dialog; checked as typed).
- `ECHOCTL_PICK_FOLDER=<dir>` answers the folder picker in a debug build; `ECHOCTL_PICK_FILES=<path>[:<path>…]` (`;` on
  Windows) the file picker (the files there, as `multiple` and `ext` take them).

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
| using folders, sessions or `echo.read` | `jshost::a_plugin_gives_echos_sessions_its_skill_and_the_folders_it_was_given` | exemplar is a test; calls, calendar, me: `jshost::a_plugin_reads_calls_the_calendar_and_who_you_are_only_as_its_sheet_said` |
| adding Today widgets | `jshost::a_plugins_widgets_are_in_the_catalog_and_give_their_data_by_provide_and_set` | exemplar is a test; a plugin with one widget of each kind: `tests/fixtures/plugins` |
| giving work rows | `jshost::a_plugins_work_rows_come_by_scope_from_its_last_answer_and_go_with_it` | exemplar is a test |
| keeping values per thread | `jshost::a_plugin_keeps_values_per_thread_and_hears_when_a_thread_goes` | exemplar is a test |
| using the user's sign-in, or testing against fake services | `jshost::a_plugin_is_handed_a_sign_in_only_for_the_hosts_its_sheet_named` | `jshost::a_plugins_named_host_reaches_the_fake_service_set_for_tests` |
| publishing an update | `plugins::install_update_and_remove_from_a_local_marketplace` | exemplar is a test |
