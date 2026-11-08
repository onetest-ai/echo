# Window UI

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the webview UI: escaping, IPC actions, compiled-in assets, the script and styles in parts per area, testable app.js blocks

## The Echo window

### Rules
- R1. Escape text you put into HTML strings in `app.js` with `esc(...)` when it is not Echo's own literal (transcripts, chat, mail, names, model and plugin output). Evidence: `esc` (window script)
- R2. Add a page action as a `cmd` arm of its area's `handle` in `crates/echoctl/src/window/<area>.rs`, and an `/api/...` request as an arm of the area's `route` (`window::handle` and `window::respond` ask each area in turn, in the order listed there, and the first answer wins: an area that matches a whole prefix (threads: `/api/client/`, `/api/project/`; call: `/api/call/`, `/minutes/`, `/file/`; plugins: `/plugin/`) takes every path under it, so a new path under such a prefix goes in that area; a new area is a new file and one line in each; an item another area needs is imported by name, `use super::<area>::{item}`, and helpers every area uses live in `window/mod.rs`); work that can take more than a moment runs on its own thread and reports through state the page polls (as `brain_taxonomy`). Evidence: `window::settings::handle`
- R3. Ship page assets compiled in with `include_str!` from `crates/echoctl/ui/` (the script and styles joined by build.rs, R6) and third-party JS vendored in `ui/vendor/` with its licence and README entry; the page loads nothing from npm or a CDN (the CSP in `window::reply` allows only `'self'`/`echo:`). Evidence: `window::reply`
- R4. Put window logic you want tested in a no-DOM block of `app.js` between a `// ── <name>` marker and `// end of the <name> block`, and test it with a `ui/<name>.test.mjs` that slices that block out of the joined script (`appSource()` from `ui/source.mjs`; there is no `ui/app.js` file) and adds itself to `scripts/check-mod.sh`. Evidence: `workbench.test.mjs::ACTIVITIES`
- R5. Save a plain setting through `window::settings_set` (one key, the value shown); a flow that needs a dialog, sign-in or download runs the tray's `Action` by name through `window::settings_action`. Evidence: `window::settings::settings_set`
- R6. The window's script and styles are parts per area: `crates/echoctl/ui/app/NN-<area>.js` and `ui/css/NN-<area>.css`, joined in name order at build time into one `app.js` and one `app.css` (one classic script: functions hoist across parts; top-level code and the CSS cascade keep the parts' order). Put an area's code in its own part; a new area is a new part with a free two-digit prefix, not more lines in another area's part. "app.js" in these guides means the joined script. Each part ends with a newline and is named `NN-<area>.js` (lowercase, digits, dashes); the build and `ui/source.mjs` stop on anything else. Evidence: `crates/echoctl/build.rs::join_parts`
- R7. A part over 1,500 lines is split before it grows (architecture.md R13, `scripts/size-check.sh`); a feature's view, styles and IPC go in its own part and `window/<area>.rs`, not in `90-render.js` or `window/mod.rs` (architecture.md R14). Evidence: `scripts/size-check.sh`
- R8. Accept IPC only from the top page, never from a frame (`window::from_page`), and take the minutes frame's messages only from its own `contentWindow`, each shape checked. Evidence: `window::from_page`
- R9. Replace a region of the page only when its HTML changed (`render`/`patch`), and update a focused editor in place. Evidence: `patch` (app script)
- R10. A palette action runs as its button's click (`press`), never its own copy of the code. Evidence: `press` (app script)
- R11. A setting is saved as the value shown, never toggled from a stored one. Evidence: `window::settings_set`
- R12. Never write `my-notes.md` when its text is unchanged, and when a save meets an outside change keep the other version as a file beside it, never drop it. Evidence: `calls::save_my_notes`
- R13. The user's notes reach the minutes frame as a message, never written into the minutes page or its file. Evidence: `minutes::EMBED_JS`
- R14. `<echo_view>` goes only to Echo's own terminals (`ECHOCTL_TERMINAL`) and only when it changed. Evidence: `view.rs`
- R15. Claude sessions run in the data folder, never a call's folder; their PTYs belong to the app and outlive the window. Evidence: `terminal.rs`
- R16. One leave (close, quit, update) at a time, and the page saves first. Evidence: `tray.rs` `Leaving`
- R17. Never rebuild the macOS menu while it is open. Evidence: `tray::menu_open`
- R18. Microsoft 365 is called only from the async protocol handler and cached (the calendar, 2 min), never on the UI thread. Evidence: `window/calendar.rs`
- R19. An Inbox item becomes Done only when its source was read. Evidence: `attention.rs`
- R20. A meeting type is kept per calendar series (a one-off per its event), never per subject. A user's Stop never re-arms a meeting. Evidence: `calls::meeting_type`, `watch::arm`
- R21. A template or skill file is addressed by its path in its folder, never by an id or path the page sent. Evidence: `calls::templates_view`
- R22. One scroll per card, never the page: the view's head stays put, each card scrolls inside with a least height and is never squeezed. Evidence: `ui/css/`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Rename or remove a block marker such as `// end of the workbench block` | Keep the markers and move code inside them | `workbench.test.mjs::ACTIVITIES` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| adding a page action | `window::settings::handle` | `window::only_the_windows_own_page_reaches_its_actions` |
| adding tested window logic | `Palette` (window script) | `palette.test.mjs::Palette` |
