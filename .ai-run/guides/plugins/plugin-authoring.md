# Plugin Authoring

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: writing, testing, publishing and updating a third-party Echo plugin: manifest, host and client halves, marketplace

## Manifest

### Rules
- R1. Ship a plugin as one folder with `echo-plugin.json` at its root: `id` (1–40 chars, a–z 0–9 `-`, no leading `-`), `name` (≤ 60), `version` (`X.Y.Z[-pre]`), `echo` (`>=X.Y.Z`), optional `summary` (≤ 240), `uses`, `settings`, `main`, `client`, `widgets`. Evidence: `plugins::valid_id`
- R2. Point `main` and `client` at a `.js`/`.mjs` inside the plugin folder; read any other file it ships with `ctx.package.read(path)` / `ctx.package.list(dir)`. Evidence: `jshost::package_file`
- R3. Declare each setting as `{key, label, type, default?, help?}`: `key` unique (a–z 0–9 `-` `_`), `type` `string` (default), `bool`, `folder` or `secret`; read it with `ctx.settings.get(key)`. Evidence: `plugins::set_setting`
- R4. Name in `uses` only capabilities Echo knows, in their shapes (`claude`, `schedule`, `deliver.to`, `deliver_provider.kinds`, `network.hosts` or `network.hosts_from` (string settings without a default), `channel.plugin`, `sessions`, `folders.what` (read-only, or `folders.write` to change them), `files.read` (other apps' files, each named in full), `sessions.commands` (its commands Claude in Echo may run), `echo.read`/`write`); never `brain_source` or `export`. Evidence: `plugins::describe_folders`
- R5. Ask only for what the user can agree to line by line on the permission sheet; an update adding a line needs their agreement again (`echoctl plugin update --yes`). Evidence: `plugins::sheet`
- R6. Declare each Today widget in `widgets` as `{id, label, about?, kind, sizes?, heights?, height?, refresh?, params?}`: `id` unique (a–z 0–9 `-`), `label` 1–40, `about` ≤ 120, `kind` `stat`, `stats`, `chart`, `list`, `status`, `card` or `page` (a page needs a `client`), `sizes` of `S` `M` `L` each once (default `["M"]`), `heights` of `1` `2` each once, with `1` (default `[1]`: the rows of Today's grid it may take), `height` `hug` (default) or `fill`, `refresh` ≥ 60 s; at most 12, and only with a `main` to give their data; its settings in `params` as `{key, label, type, options?, default?}` — `type` `choice` (1–12 `options`), `bool`, `thread` or `client`, 6 at most. Evidence: `plugins::check_params`

## Host half

### Rules
- R1. Export `apply(ctx)` from the host half and register its commands, timers, event handlers and delivery ways inside it. Evidence: `jshost::load`
- R2. Write a command as `ctx.command(name, fn)`: `fn` takes JSON arguments, may be `async`, returns JSON and throws an `Error` to fail; keep a window-called command under 30 s. Evidence: `jshost::CALL_TIMEOUT`
- R3. Use `ctx.every(seconds, fn)` for periodic work (10 s minimum); never rely on timers in `echoctl plugin call` or `echoctl deliver`. Evidence: `jshost::no_timers`
- R4. Name each event in `uses.events.on` and subscribe with `ctx.on(event, fn)`; read a call's words with `ctx.echo.read('source', id)`, never from a payload; events are not replayed. Evidence: `plugins::EVENTS`
- R5. Keep a plugin's own state with `ctx.store.read/write/list/remove/path`, by relative path only. Evidence: `plugins::data_dir`
- R6. Check `status` in the `{status, body}` that `ctx.http.fetch(url, {method, headers, body})` resolves; it rejects only when the host can't be reached. Evidence: `jshost::fetch`
- R7. Run Claude with `await ctx.claude.run({prompt, title, profile, model, tools, dir, timeout})` only with `claude` in `uses`; `tools: "skip"` needs `"claude": {"skip": true}`. Evidence: `claudejob::Tools::parse`, `plugins::may_run_claude`
- R8. Provide a delivery kind with `ctx.deliver.provide(kind, {label, send(msg)})` only for a kind in `uses.deliver_provider.kinds` (never `teams` or `mail`), and call `ctx.deliver.ready(kind, true)` once it can send. Evidence: `jshost::provider_ready`, `plugins::may_send`
- R9. Call `ctx.echo.read(kind, arg)` only for kinds in `uses.echo.read`, and `ctx.echo.dictionary.add(...)` only under `uses.echo.write: ["dictionary"]`. Evidence: `share::dictionary_add`
- R10. Get a folder with `await ctx.system.pickFolder(prompt)` (needs `uses.folders`) and read it as `(root, relative path)`, bulk reads through `ctx.fs.readMany/walk`; with `folders.write` change it with `ctx.fs.write/move/remove` (never through a link, never over another file); in a debug build answer the picker with `ECHOCTL_PICK_FOLDER=<dir>`. Evidence: `app::choose_folder`
- R11. Set `ctx.sessions.skills`, `ctx.sessions.read` and `ctx.view.hints` from `apply` on every load, each as the whole set (`[]` removes it). Evidence: `grants::drop_plugin`
- R12. Ask the user for a decision with `ctx.inbox.set(key, {title, why, urgent, go, actions: [{label, command, args, primary}]})` (key ≤ 80 chars), and `ctx.inbox.clear(key)` once it is settled. Evidence: `attention::inbox_items`
- R13. Bundle the host half into one file with no `import` of other files; the client half may import its own modules. Evidence: `window::plugins::plugin_file`
- R14. Keep large data in the store, not in JavaScript memory (64 MB shared by all host halves). Evidence: `jshost::set_memory_limit`
- R15. Give a widget's data with `ctx.widgets.provide(id, ({size}) => data)` — Echo asks it in the background, answered within 5 s, again after `refresh` — or push it with `ctx.widgets.set(id, data)` and take the widget off Today with `ctx.widgets.clear(id)`; only ids the manifest declares, 64 KB of data at most; work that needs Claude runs as a job whose result you `set`. Evidence: `jshost::widget_ask`
- R16. Shape a widget's data by its kind — `stat` `{value, unit?, label?, tone?, note?, progress?: {used, of}}`, `stats` `{items: [{label, value, tone?}]}`, `chart` `{type: bar|bars|line, points: [{label, value}], unit?, note?}` (values numbers), `list` `{items: [{title, sub?, link?}], empty?}`, `status` `{state: ok|warn|bad, title, text?}`, `card` `{title?, text?, link?}`; `tone` `good`, `warn` or `bad`; text is plain (Echo escapes it), a `link` only to `echo://`; check it with `echoctl widget show plugin:<id>:<widget>`. Evidence: `pluginWidgetHtml` (window script)
- R18. Read a widget's settings from the `params` your provider gets (`ctx.widgets.provide(id, ({params}) => …)`: its defaults, then what the user set in Today's edit mode or with `echoctl today set`); Echo asks again when one changes, and never gives a value the setting doesn't take. Evidence: `today_layout::params_of`
- R17. Give any widget `open` (an `echo://` place, or an `https://` page Echo opens only after the user confirms it) and up to 3 `actions: [{label, command, args}]` — each runs your host half's `ctx.command(command)` with `args` on the user's click, then Today reads your widgets again; answer by `ctx.widgets.set` what changed. Evidence: `pluginWidgetHtml` (window script)
- R19. Save audio or a picture with `ctx.http.fetch(url, {save: "<store path>"})` and show it from `/plugin/<id>/store/<path>`; put what you can lose under `cache/` (Echo removes it after 7 days) and remove the rest yourself with `ctx.store.remove`. Evidence: `jshost::sweep_store`
- R20. Name a skill's Echo CLI `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`, and list in `uses.sessions.commands` the commands of yours it runs (`plugin call <id> <command>`); in a terminal (`ctx.terminal`) only your plugin is loaded, so skip start-up work there. Evidence: `plugins::session_commands`
- R21. Keep what belongs to a thread (its repositories, its project) with `ctx.threads.set(thread, key, value)` (JSON, 16 KB at most; `null` removes it; not in a read-only routine), read it with `ctx.threads.get(thread, key)` (null when none) and `ctx.threads.list(key)` (`[{thread, value}]`, threads gone left out) — no grant, never your own thread keys in the store; Echo drops a thread's values when it is deleted, and you hear it renamed, merged or deleted with `threads_changed` in `uses.events.on` (`{thread, change: renamed|merged|deleted, into?}`). Evidence: `plugins::thread_values::changed`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Write `settings.json`, `inbox.json`, `widgets.json` or `threads.json` with `ctx.store.write` (Echo refuses `settings.json` and `threads.json`) | Use `ctx.settings.set`, `ctx.inbox.set` / `clear`, `ctx.widgets.set` / `clear` and `ctx.threads.set` | `jshost::inbox_file`, `jshost::widgets_file`, `plugins::thread_values::FILE` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| writing a first host half | `jshost::a_host_half_gets_echos_services_and_answers_commands` | exemplar is a test |
| using folders, sessions or `echo.read` | `jshost::a_plugin_gives_echos_sessions_its_skill_and_the_folders_it_was_given` | exemplar is a test |
| adding Today widgets | `jshost::a_plugins_widgets_are_in_the_catalog_and_give_their_data_by_provide_and_set` | exemplar is a test; a plugin with one widget of each kind: `tests/fixtures/plugins` |

## Client half

### Rules
- R1. Export `apply(ctx)` from the client half and add to the window only through `ctx` (`call`, `slots.add` on a known slot, `panels`, `activities`, `settings`, `palette`, `listen`, `ui`, `go`, `render`). Evidence: `loadPluginClients` (window script)
- R2. Escape every text you put in HTML with `ctx.ui.esc` and render Markdown with `ctx.markdown`. Evidence: `slotItems` (window script)
- R3. Draw a `page` widget with `ctx.widgets.draw(widget, ({data, size, theme}) => HTML)` from `data` only (its host half gives it by `provide` / `set`): the same data must give the same HTML, and the draw neither calls `ctx.render` nor keeps state in the page. Evidence: `pluginWidgetView` (window script)
- R4. Add a row to each thread's Profile with `ctx.slots.add('thread.profile', {id, label, render(thread) → HTML, after?(box, thread)})`: `thread` is `{id, name, client}`, drawn under the thread's Sources; `render` is synchronous (draw from what you have, fetch in `after` and `ctx.render()`), and one that throws is left out, the Profile drawn without it. Evidence: `threadSlotRows` (window script)

## Publishing

### Rules
- R1. Host a marketplace as a folder or git repository with `echo-marketplace.json` at its root (`{name, featured, plugins: [{id, source, categories}]}`), each entry's manifest `id` equal to its `id`. Evidence: `plugins::CatalogEntry`
- R2. Give users a marketplace source as `owner/repo[@ref]`, a git address (`https://`, `ssh://`, `git@`, `#ref`) or a local folder; a ref is a plain branch, tag or commit. Evidence: `plugins::parse_source`
- R3. Bump `version` for every change you publish. Evidence: `plugins::update_for`
- R4. Restart the app after `echoctl plugin install | update | enable | disable` from a terminal. Evidence: `plugins::page_op`

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| publishing an update | `plugins::install_update_and_remove_from_a_local_marketplace` | exemplar is a test |
