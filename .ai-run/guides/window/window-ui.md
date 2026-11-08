# Window UI

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the webview UI: escaping, IPC actions, compiled-in assets, the script and styles in parts per area, testable app.js blocks

**Design**: `.ai-run/guides/design/window.md` — how it works and why; the rules here win.

## The Echo window

### Rules
- R1. Escape text you put into HTML strings in `app.js` with `esc(...)` when it is not Echo's own literal (transcripts, chat, mail, names, model and plugin output). Evidence: `crates/echoctl/ui/app/00-core.js::esc`
- R2. Add a page action as a `cmd` arm of its area's `handle` in `crates/echoctl/src/window/<area>.rs`, and an `/api/...` request as an arm of the area's `route` (`window::handle` and `window::respond` ask each area in turn; a new area is a new file and one line in each); work that can take more than a moment runs on its own thread and reports through state the page polls (as `brain_taxonomy`). Evidence: `crates/echoctl/src/window/settings.rs::handle`
- R3. Ship page assets compiled in with `include_str!` from `crates/echoctl/ui/` (the script and styles joined by build.rs, R6) and third-party JS vendored in `ui/vendor/` with its licence and README entry; the page loads nothing from npm or a CDN (the CSP in `window::reply` allows only `'self'`/`echo:`). Evidence: `crates/echoctl/src/window/mod.rs::reply`
- R4. Put window logic you want tested in a no-DOM block of `app.js` between a `// ── <name>` marker and `// end of the <name> block`, and test it with a `ui/<name>.test.mjs` that slices that block out of `app.js` and adds itself to `scripts/check-mod.sh`. Evidence: `crates/echoctl/ui/workbench.test.mjs::ACTIVITIES`
- R5. Save a plain setting through `window::settings_set` (one key, the value shown); a flow that needs a dialog, sign-in or download runs the tray's `Action` by name through `window::settings_action`. Evidence: `crates/echoctl/src/window/settings.rs::settings_set`
- R6. The window's script and styles are parts per area: `crates/echoctl/ui/app/NN-<area>.js` and `ui/css/NN-<area>.css`, joined in name order at build time into one `app.js` and one `app.css` (one classic script: functions hoist across parts; top-level code and the CSS cascade keep the parts' order). Put an area's code in its own part; a new area is a new part with a free two-digit prefix, not more lines in another area's part. "app.js" in these guides means the joined script. Evidence: `crates/echoctl/build.rs::join_parts`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Rename or remove a block marker such as `// end of the workbench block` | Keep the markers and move code inside them | the node tests slice the joined script (`ui/source.mjs::appSource`) by these exact strings and fail otherwise | `crates/echoctl/ui/workbench.test.mjs::ACTIVITIES` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a page action | `crates/echoctl/src/window/settings.rs::handle` | an area's `cmd` arms, feature-gated arms, background threads | `crates/echoctl/src/window/mod.rs::only_the_windows_own_page_reaches_its_actions` |
| adding tested window logic | `crates/echoctl/ui/app/05-palette-rank.js::Palette` | self-contained IIFE inside its markers | `crates/echoctl/ui/palette.test.mjs::Palette` |
