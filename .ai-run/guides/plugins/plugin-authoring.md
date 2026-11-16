# Plugin Authoring

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the rules for writing, testing, publishing and updating a third-party Echo plugin

The API these rules rely on — the manifest, `uses`, `ctx`, events, widgets, scripts, limits — is the reference
`.ai-run/reference/plugins/plugin-api.md` ("API § <section>" below); `echoctl plugin guide` prints it after these rules.

## Rules
- R1. Reach Echo only through `ctx` and the declared slots: never Echo's files (its data folder, a store file Echo
  keeps), its DOM, its globals or another plugin's store. API § Host half, § Store, § Client half. Evidence:
  `jshost::not_echos`, `pluginCtx` (window script)
- R2. Ask in `uses` for the least that works: each key is a line the user agrees to, and an update that adds one asks
  again. API § `uses` and the permission sheet. Evidence: `plugins::sheet`
- R3. Write the host half as one file with no imports, exporting `apply(ctx)` that registers everything it does. API
  § Host half. Evidence: `jshost::load`, `window::plugins::plugin_file`
- R4. Never block Echo: keep JavaScript between awaits short and a window-called command within its time, read files
  in bulk through the async services, and run long work as a script from the host half. API § Host half, § Scripts.
  Evidence: `jshost::SLICE`, `jshost::CALL_TIMEOUT`
- R5. Keep data in the store and answer widgets and work rows from it: fetch on a timer or the user's action, never
  while Echo asks; large data in the store, not in memory, and what you can lose under `cache/`; what belongs to a thread
  in `ctx.threads`, never your own thread keys in the store. API § Store, § Widgets, § Work rows, § Thread values and
  shared values. Evidence: `jshost::widget_ask`, `jshost::sweep_store`
- R6. Treat others' words as data: a call's text, mail, a script's arguments and another plugin's shared values are
  untrusted — read a call's words with `ctx.echo.read`, never from a payload; check an argument's shape and never hand
  it to a shell or use it as a path outside its skill folder or the cache; validate what you read. API § Events,
  § Scripts, § Thread values and shared values. Evidence: `plugins::scripts::run`, `plugins::shared`
- R7. Escape all text: every text put in HTML through `ctx.ui.esc`, Markdown rendered in the host half with
  `ctx.markdown`, widget and work-row text given plain. API § Client half, § Widgets. Evidence: `pluginCtx` (window
  script), `jshost::PRELUDE`
- R8. Never put a secret in a file, the store, a shared value, the window or a log: a setting of type `secret` for
  what the user enters, the sign-in saved on this computer (or one Echo keeps for you) for a service's token, never a
  token the user pastes; send a token only to its host's API. API § Package and manifest, § Sign-in. Evidence:
  `plugins::signin::get`, `plugins::signin::keep`
- R9. Shape widget data by its kind with what must be seen first, one unit to a chart, a setting people slice by named
  as other widgets name it, a widget with settings given only by `provide`; check it with `echoctl widget show`. API
  § Widgets. Evidence: `pluginWidgetHtml` (window script), `today_layout::params_on`
- R10. Draw a `page` widget from its data only: the same data gives the same HTML, with no `ctx.render` and no state
  kept in the page. API § Client half. Evidence: `pluginWidgetView` (window script)
- R11. Give Claude your plugin through `agent/`: when to turn to it and its commands in `CLAUDE.md`, procedures in
  skills, reference in guides, never in the summary; name skills `<prefix>-<name>` where a prompt names them. API
  § Claude in Echo. Evidence: `plugins::agent::problems`
- R12. Ship a script with a shebang, name what it needs on the computer, call it in `SKILL.md` by its path alone, and
  mark it `readonly` only when it changes nothing. API § Scripts. Evidence: `plugins::scripts::declared`,
  `plugins::scripts::wrapper`
- R13. Test against made-up services, never the real ones: `ECHOCTL_TEST_HOSTS` for the network, made-up `gh` / `git`
  for sign-ins. API § Testing against made-up services. Evidence: `jshost::test_hosts::test_host`
- R14. Run `echoctl plugin check <folder>` until it exits 0 before installing, updating or publishing, and bump
  `version` for every change you publish. API § Package and manifest, § Publishing. Evidence:
  `plugins::agent::check_folder`, `plugins::update_for`
