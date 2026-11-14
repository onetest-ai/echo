# Echo Plugin Host

**Reference**, not a guide: how Echo's side of plugins works — what it keeps in a plugin's store, how it runs scripts,
lays out `.plugins`, fills dashboards' settings and reaches sessions — as the code has it. It changes in the same PR
as the code it describes. The rules are in `.ai-run/guides/plugins/echo-plugins.md`; the API plugins see is in
`plugin-api.md` beside this.

## Store

- A plugin's store is the plugin's to clean; Echo removes only what lay in its `cache/` over 7 days and its own
  half-written downloads (`.part`) over an hour, when its host half starts. Evidence: `jshost::sweep_store`
- Echo keeps the plugin's `settings.json`, `inbox.json`, `widgets.json`, `threads.json` and `work.json` there;
  `threads.json` and `work.json` each change under its own lock across processes (`.locks/<id>.threads.lock`,
  `.locks/<id>.work.lock`); a `threads.json` that isn't JSON is moved aside as `threads.json.bad-<time>`. The plugin
  writes none of those five and removes none of `settings.json`, `threads.json`, `work.json` and the store folder
  itself; a name at the store's top level is plain ASCII. Evidence: `jshost::not_echos`
- A marketplace namespace's shared values (`<ns>.<name>`) live beside the stores in `@<ns>/` (`threads.json`,
  `shared.json`), each under its own lock (`.locks/@<ns>.threads.lock`, `.locks/@<ns>.shared.lock`), no plugin's store
  and never removed with one. Evidence: `plugins::shared`
- The window serves a store only as `/plugin/<id>/store/<file>`, media kinds only, kept inside the store.
  Evidence: `window::plugins::store_file`
- The addresses a plugin may reach by `network.hosts_from` are entered only by the user, in the window or the CLI, and
  kept outside the plugin's store. Evidence: `plugins::listed_addresses`

## Script runs

- Every run goes through `plugins::scripts::run`: the original from the installed package, made runnable by Echo, in
  its skill's session copy. The host half reaches it by `ctx.scripts.run`; Echo's sessions through the launcher Echo
  writes at the script's path in the session copy and in the laid-out skill (`echoctl plugin script`, so Claude Code's
  environment never reaches it) — the laid-out one allowed by its full path in the shared session arguments
  (`folder_args::build`): every declared one in the folder session and in a routine's or Yolo's run acting on its own,
  only one marked `readonly` in a run preparing and asking (which `echoctl plugin script` checks again), with no
  `RUN_PLUGIN_*` guard. Evidence: `plugins::scripts::run`
- A script runs without stdin, within its declared time (a call may ask for less, a routine uses it as declared), in
  its own process group, killed whole when its time is up or the plugin is unloaded, updated or removed, or Echo quits;
  its output capped (what is past the cap cut and flagged); one run of a script at a time across processes (a lock per
  script; whichever process holds it, Echo stops its group). `ctx.scripts.run` is an async service: the host half
  answers its other calls while a script runs. Evidence: `plugins::scripts::run`, `plugins::scripts::stop_plugin`
- Its environment: `plugin-api.md` § Scripts. Evidence: `plugins::scripts::run`, `plugins::scripts::run_markers`

## Sessions

- A plugin's session skills (its runtime text skills and its package skills) go only to Echo's sessions: the folder
  session, routines and Yolo get a laid-out plugin's skills through `.plugins/<id>/` (§ Layout) and a plugin not laid
  out by its session copy (`--plugin-dir`), once each; a `tools: "none"` run gets none.
  Evidence: `grants::set_session_skills`, `folder_args::build`
- Package skills are copied from the installed package into the plugin's session folder, beside its runtime text
  skills: checked by `plugins::package_skills` as a plugin tree (plain files, no links, inside the plugin), copied again
  on install, update and load, removed with the plugin. Evidence: `grants::copy_package_skills`
- A plugin's commands named in `uses.sessions.commands` are allowed by the arguments the folder session, routines and
  Yolo share (`folder_args::build`), with no `RUN_PLUGIN_*` guard. Evidence: `plugins::session_commands`

## Layout

Echo lays out each enabled plugin's `agent/` folder, package skills and runtime skills and entries at
`<data folder>/.plugins/<id>/` (`CLAUDE.md`, `.claude/skills`, `.claude/rules`, `guides/`, `scripts/echoctl`), built
beside it and swapped in, `${CLAUDE_PLUGIN_ROOT}` filled in; again on install, update, enable or a changed installed
copy; removed on disable and remove, and what isn't an enabled plugin's pruned at start. The folder session gets no
plugin skill up front: Echo's core (`echoctl core --hook`, a SessionStart hook) names each plugin with its summary and
its `CLAUDE.md` to Read first, and Claude Code loads the rest of that folder when Claude reads there; plugin text in the
core is one line each, `|` escaped, the core ≤ 9,000 chars (rows left out are counted). Each skill is laid out as
`<prefix>-<name>` (the manifest's `agent.prefix`, bound to its marketplace source like a shared namespace; a laid-out
name is unique across enabled plugins). The layout's record (installed files, folder, Echo's version, a hash of what
was written) lives in the config folder, and a folder whose record differs is laid out again (`sync_all`, the core's
hook). The pre-tool hook of the folder session, routines and Yolo (`echoctl plugins-hook`) refuses Edit and Write
under `.plugins/` and, best effort, a Bash line that writes there.
Evidence: `plugins::agent::lay_out`, `plugins::agent::render`, `guard`

## Dashboards

A thread's dashboard (its Brief) offers only widgets with a setting of type `thread`, a client's (its Overview) only
those with one of type `client`, and Echo's Filters on both; the page sets those settings, not editable there: on a
thread's page its `thread` setting is that thread, and a `client` setting beside it that thread's client (none: its
default); on a client's page its `client` setting is that client, and a `thread` setting beside it keeps the user's
value only for a thread of that client (else its default). Each place a widget is on is asked, and its answer kept, on
its own. Evidence: `today_layout::params_on`, `today_layout::Page::holds_thread`

## Where things are

| When you are | Exemplar | Tests |
|---|---|---|
| adding a host service | `jshost::service` | `jshost::a_host_half_gets_echos_services_and_answers_commands` |
| adding to the client-half `ctx` | `pluginCtx` (window script) | no covering test |
| changing install, update or remove | `plugins::install` | `plugins::install_update_and_remove_from_a_local_marketplace` |
| letting a plugin run Claude | `claudejob::Spec::from_value` | `claudejob::a_spec_stays_in_the_plugins_store` |
| changing package skills or scripts | `plugins::scripts::run` | `jshost::a_plugins_package_skill_reaches_the_sessions_and_its_declared_script_runs` |
