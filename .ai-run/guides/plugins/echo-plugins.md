# Echo Plugins

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: plugin manifest, host half services, grants, client-half ctx and slots, marketplaces

## Plugin host

### Rules
- R1. Add a capability plugins may ask for as a `Capability` in `plugins::CAPABILITIES` with its own `describe` function: the permission sheet's words are written in Echo, never taken from the plugin; keep `ready: false` until the host implements it (an install then says it needs a newer Echo). Evidence: `plugins::CAPABILITIES`
- R2. Add a host-half service in three places: the `ctx` object in `jshost::PRELUDE`, then an arm in `jshost::service` (returns at once) or `jshost::service_async` (spawns a process, waits on the network or a dialog). Evidence: `jshost::PRELUDE`
- R3. A host service that reaches Echo data, granted folders, sessions or the network checks the manifest first with `jshost::uses` / `jshost::uses_flag` (as `fs.*`, `sessions.*`, `view.hints`, `echo.read`) or the capability's own list (`network.hosts` in `jshost::fetch`, `channel.plugin`, `deliver_provider.kinds`). Evidence: `jshost::uses_flag`
- R4. Resolve a plugin's own files with `plugins::inside_plugin` / `inside_plugin_dir`, its store with `jshost::store_path`, and granted folders with `grants::resolve`; each refuses absolute paths and `..`. Evidence: `plugins::inside_plugin`
- R5. Validate a manifest only in `plugins::read_manifest` (id a–z 0–9 `-`, name 1–60 chars, semver version, summary ≤ 240, setting types `string|bool|folder|secret`, `main`/`client` a `.js`/`.mjs` inside the package, `uses` through `describe`); a new manifest field gets its check there. Evidence: `plugins::read_manifest`
- R6. A host half's command or tick that runs JavaScript for over 5 s is interrupted (`jshost::SLICE`); move bulk file reads to the async services (`ctx.fs.readMany`, `ctx.fs.walk`), which run off the slice. Evidence: `jshost::SLICE`
- R7. Add a client-half slot by listing it in `SLOTS` in `app.js` and drawing it with `slotItems`; register everything a plugin adds under its id so `unloadPlugin` takes it out. Evidence: `unloadPlugin` (window script)
- R8. A new `ctx.echo.read` kind goes in `plugins::ECHO_READS` (the sheet's words) and the `echo.read` arm of `jshost::service`, its data from `share.rs`; an unknown kind is named on the sheet and granted nothing. Evidence: `plugins::ECHO_READS`
- R9. Develop a plugin against a local marketplace folder (`echoctl plugin marketplace add <folder>`, read in place) or point the built-in one elsewhere with `ECHOCTL_PLUGIN_MARKET`; drive the host half with `echoctl plugin call <id> <command> [json]`. Evidence: `plugins::built_in_source`
- R10. A plugin's Claude runs only through `claudejob::run` on one of the user's AI profiles, with the Brain narrowed to it; never a plugin's own key or account. Evidence: `claudejob::run`
- R11. A plugin's session skills go only to Echo's sessions and `claudejob` runs, never to the user's own Claude Code. Evidence: `grants::set_session_skills`
- R12. Never let a channel's env set a `CLAUDE*` or `ANTHROPIC*` key. Evidence: `jshost::service`
- R13. Deliver only to the user and the addresses the user set on a routine, never to an address a model picked; anything a model wrote carries `ai_notice`. Evidence: `deliver::send`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Put a link or special file in a plugin or marketplace tree | Ship plain files within 400 files / 20 MB a plugin, 5000 / 64 MB a marketplace | `plugins::check_tree` |
| Fetch from a host the manifest doesn't name, or over http | `ctx.http.fetch` to `https://` hosts listed in `uses.network.hosts` | `jshost::fetch` |
| Give Echo's sessions a skill with frontmatter, hooks or `!` commands | Pass `[{name, description, body}]` to `ctx.sessions.skills`; Echo writes the plugin around them | `grants::set_session_skills` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| adding a host service | `jshost::service` | `jshost::a_host_half_gets_echos_services_and_answers_commands` |
| adding to the client-half `ctx` | `pluginCtx` (window script) | no covering test |
| changing install, update or remove | `plugins::install` | `plugins::install_update_and_remove_from_a_local_marketplace` |
| letting a plugin run Claude | `claudejob::Spec::from_value` | `claudejob::a_spec_stays_in_the_plugins_store` |
