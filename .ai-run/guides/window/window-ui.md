# Window UI

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: building the webview UI: escaping, IPC, assets, the script and styles in parts, testable blocks, layout

## The Echo window

### Rules
- R1. Put text that isn't Echo's own literal (transcripts, chat, mail, names, model and plugin output) into HTML only through `esc(...)`. Evidence: `esc` (window script)
- R2. A page action is a `cmd` arm of its area's `handle`, an `/api/...` request an arm of its area's `route`, in `window/<area>.rs`; a path under another area's prefix goes in that area. Work longer than a moment runs on its own thread and reports through state the page polls. Evidence: `window::settings::handle`
- R3. Accept IPC only from the top page (`window::from_page`), and the minutes frame's messages only from its own `contentWindow`, each shape checked. Evidence: `window::from_page`
- R4. Page assets are compiled in from `crates/echoctl/ui/`; third-party JS is vendored in `ui/vendor/` with its licence and README entry; nothing loads from npm or a CDN. Evidence: `window::reply`
- R5. The script and styles are parts per area, `ui/app/NN-<area>.js` and `ui/css/NN-<area>.css`, joined in name order at build time; an area's code goes in its own part, a new area is a new part with a free two-digit prefix. Each part is named `NN-<area>.js` (lowercase, digits, dashes) and ends with a newline. Evidence: `build.rs::join_parts`
- R6. Window logic you test lives in a no-DOM block between `// ── <name>` and `// end of the <name> block`, tested by a `ui/<name>.test.mjs` that slices that block from the joined script (`appSource()`) and is listed in `scripts/check-mod.sh`; never rename or remove a block's markers. Evidence: `workbench.test.mjs::ACTIVITIES`
- R7. A setting is saved as the value shown through `window::settings_set`, never toggled from a stored one; a flow that needs a dialog, sign-in or download runs the tray's `Action` by name through `window::settings_action`. Evidence: `window::settings::settings_set`
- R8. Replace a region of the page only when its HTML changed (`render`/`patch`), and update a focused editor in place. Evidence: `patch` (window script)
- R9. A palette action runs as its button's click (`press`), never its own copy of the code. Evidence: `press` (window script)
- R10. One scroll per card, never the page: the view's head stays put, each card scrolls inside with a least height and is never squeezed. Evidence: `ui/css/`

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| adding a page action | `window::settings::handle` | `window::only_the_windows_own_page_reaches_its_actions` |
| adding tested window logic | `Palette` (window script) | `palette.test.mjs::Palette` |
