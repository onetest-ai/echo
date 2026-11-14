# Window UI

**Project**: Echo · **Scope**: building the webview UI: escaping, IPC, assets, the script and styles in parts, testable blocks, layout

**Reference**: `.ai-run/reference/window/window-ui.md` — the window's files, parts, markers and functions these rules
rely on (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## The Echo window

### Rules
- R1. Put text that isn't Echo's own literal (transcripts, chat, mail, names, model and plugin output) into HTML only through `esc(...)`. Evidence: `esc` (window script)
- R2. A page action and an `/api/...` request each belong to their area's window file; work longer than a moment runs on its own thread and reports through state the page polls. Evidence: `window::settings::handle` (ref: Page actions and routes)
- R3. Accept IPC only from the top page, and the minutes frame's messages only from its own frame, each shape checked. Evidence: `window::from_page` (ref: IPC)
- R4. Page assets are compiled in; third-party JS is vendored with its licence; nothing loads from npm or a CDN. Evidence: `window::reply` (ref: Assets)
- R5. The script and styles are parts per area, joined at build time; an area's code goes in its own part, and a new area is a new part. Evidence: `build.rs::join_parts` (ref: Parts)
- R6. Window logic you test lives in a no-DOM block, tested by a node test that slices it from the joined script; never rename or remove a block's markers. Evidence: `workbench.test.mjs::ACTIVITIES` (ref: Tested blocks)
- R7. A setting is saved as the value shown, never toggled from a stored one; a flow that needs a dialog, sign-in or download runs the tray's action by name. Evidence: `window::settings::settings_set` (ref: Settings)
- R8. Replace a region of the page only when its HTML changed, and update a focused editor in place. Evidence: `patch` (window script) (ref: Rendering)
- R9. A palette action runs as its button's click, never its own copy of the code. Evidence: `press` (window script)
- R10. One scroll per card, never the page: the view's head stays put, each card scrolls inside with a least height and is never squeezed. Evidence: `.tp-list > .set-card` (window styles)
- R11. Never write the user's notes when their text is unchanged; a save that meets an outside change keeps the other version beside it. Evidence: `calls::save_my_notes`
- R12. One leave (close, quit, update) at a time, and the page saves first. Evidence: `tray::Leaving`
- R13. An Inbox item becomes Done only when its source was read. Evidence: `attention::view`
