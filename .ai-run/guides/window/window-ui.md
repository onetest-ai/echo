# Window UI

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the webview UI: escaping, IPC actions, compiled-in assets, testable app.js blocks

**Design**: `.ai-run/guides/design/window.md` — how it works and why; the rules here win.

## The Echo window

### Rules
- R1. Escape text you put into HTML strings in `app.js` with `esc(...)` when it is not Echo's own literal (transcripts, chat, mail, names, model and plugin output). Evidence: `crates/echoctl/ui/app.js::esc`
- R2. Add a page action as a `cmd` arm of `window::handle`; work that can take more than a moment runs on its own thread and reports through state the page polls (as `brain_taxonomy`). Evidence: `crates/echoctl/src/window.rs::handle`
- R3. Ship page assets compiled in with `include_str!` from `crates/echoctl/ui/` and third-party JS vendored in `ui/vendor/` with its licence and README entry; the page loads nothing from npm or a CDN (the CSP in `window::reply` allows only `'self'`/`echo:`). Evidence: `crates/echoctl/src/window.rs::reply`
- R4. Put window logic you want tested in a no-DOM block of `app.js` between a `// ── <name>` marker and `// end of the <name> block`, and test it with a `ui/<name>.test.mjs` that slices that block out of `app.js` and adds itself to `scripts/check-mod.sh`. Evidence: `crates/echoctl/ui/workbench.test.mjs::ACTIVITIES`
- R5. Save a plain setting through `window::settings_set` (one key, the value shown); a flow that needs a dialog, sign-in or download runs the tray's `Action` by name through `window::settings_action`. Evidence: `crates/echoctl/src/window.rs::settings_set`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Rename or remove a block marker such as `// end of the workbench block` | Keep the markers and move code inside them | the node tests slice `app.js` by these exact strings and fail otherwise | `crates/echoctl/ui/workbench.test.mjs::ACTIVITIES` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a page action | `crates/echoctl/src/window.rs::handle` | `cmd` dispatch, feature-gated arms, background threads | `crates/echoctl/src/window.rs::only_the_windows_own_page_reaches_its_actions` |
| adding tested window logic | `crates/echoctl/ui/app.js::Palette` | self-contained IIFE inside its markers | `crates/echoctl/ui/palette.test.mjs::Palette` |
