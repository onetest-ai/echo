# Echo Plugins

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: plugin manifest, host half services, grants, client-half ctx and slots, marketplaces

**Design**: `.ai-run/guides/design/plugins.md` — how it works and why; the rules here win.

## Plugin host

### Rules
- R1. Add a capability plugins may ask for as a `Capability` in `plugins::CAPABILITIES` with its own `describe` function: the permission sheet's words are written in Echo, never taken from the plugin; keep `ready: false` until the host implements it (an install then says it needs a newer Echo). Evidence: `crates/echoctl/src/plugins.rs::CAPABILITIES`
- R2. Add a host-half service in three places: the `ctx` object in `jshost::PRELUDE`, then an arm in `jshost::service` (returns at once) or `jshost::service_async` (spawns a process, waits on the network or a dialog). Evidence: `crates/echoctl/src/jshost.rs::PRELUDE`
- R3. A host service that reaches Echo data, granted folders, sessions or the network checks the manifest first with `jshost::uses` / `jshost::uses_flag` (as `fs.*`, `sessions.*`, `view.hints`, `echo.read`) or the capability's own list (`network.hosts` in `jshost::fetch`, `channel.plugin`, `deliver_provider.kinds`). Evidence: `crates/echoctl/src/jshost.rs::uses_flag`
- R4. Resolve a plugin's own files with `plugins::inside_plugin` / `inside_plugin_dir`, its store with `jshost::store_path`, and granted folders with `grants::resolve`; each refuses absolute paths and `..`. Evidence: `crates/echoctl/src/plugins.rs::inside_plugin`
- R5. Validate a manifest only in `plugins::read_manifest` (id a–z 0–9 `-`, name 1–60 chars, semver version, summary ≤ 240, setting types `string|bool|folder|secret`, `main`/`client` a `.js`/`.mjs` inside the package, `uses` through `describe`); a new manifest field gets its check there. Evidence: `crates/echoctl/src/plugins.rs::read_manifest`
- R6. A host half's command or tick that runs JavaScript for over 5 s is interrupted (`jshost::SLICE`); move bulk file reads to the async services (`ctx.fs.readMany`, `ctx.fs.walk`), which run off the slice. Evidence: `crates/echoctl/src/jshost.rs::SLICE`
- R7. Add a client-half slot by listing it in `SLOTS` in `app.js` and drawing it with `slotItems`; register everything a plugin adds under its id so `unloadPlugin` takes it out (an update otherwise doubles every listener). Evidence: `crates/echoctl/ui/app.js::unloadPlugin`
- R8. A new `ctx.echo.read` kind goes in `plugins::ECHO_READS` (the sheet's words) and the `echo.read` arm of `jshost::service`, its data from `share.rs`; an unknown kind is named on the sheet and granted nothing. Evidence: `crates/echoctl/src/plugins.rs::ECHO_READS`
- R9. Develop a plugin against a local marketplace folder (`echoctl plugin marketplace add <folder>`, read in place) or point the built-in one elsewhere with `ECHOCTL_PLUGIN_MARKET`; drive the host half with `echoctl plugin call <id> <command> [json]`. Evidence: `crates/echoctl/src/plugins.rs::built_in_source`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Put a link or special file in a plugin or marketplace tree | Ship plain files within 400 files / 20 MB a plugin, 5000 / 64 MB a marketplace | `check_tree` refuses the whole tree | `crates/echoctl/src/plugins.rs::check_tree` |
| Fetch from a host the manifest doesn't name, or over http | `ctx.http.fetch` to `https://` hosts listed in `uses.network.hosts` | `jshost::fetch` refuses any other URL | `crates/echoctl/src/jshost.rs::fetch` |
| Give Echo's sessions a skill with frontmatter, hooks or `!` commands | Pass `[{name, description, body}]` to `ctx.sessions.skills`; Echo writes the plugin around them | `grants::set_session_skills` writes only name and description | `crates/echoctl/src/grants.rs::set_session_skills` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a host service | `crates/echoctl/src/jshost.rs::service` | sync ops with their `uses` checks in one match | `crates/echoctl/src/jshost.rs::a_host_half_gets_echos_services_and_answers_commands` |
| adding to the client-half `ctx` | `crates/echoctl/ui/app.js::pluginCtx` | every registration tagged with the plugin id | no covering test |
| changing install, update or remove | `crates/echoctl/src/plugins.rs::install` | review sheet agreed, tree checked and hashed, registry updated | `crates/echoctl/src/plugins.rs::install_update_and_remove_from_a_local_marketplace` |
| letting a plugin run Claude | `crates/echoctl/src/claudejob.rs::Spec.from_value` | prompt required, run folder kept inside the store, tools `none|read|folder|skip` | `crates/echoctl/src/claudejob.rs::a_spec_stays_in_the_plugins_store` |
