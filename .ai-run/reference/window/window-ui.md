# Window UI — reference

How the Echo window is built: its files, parts, markers and functions. It changes with the code, in the same PR. The
rules are in `.ai-run/guides/window/window-ui.md`.

## Escaping

`esc(...)` (window script) escapes text for HTML.

## Page actions and routes

- A page action is a `cmd` arm of its area's `handle`; an `/api/...` request is an arm of its area's `route`; both in
  `window/<area>.rs`. A path under another area's prefix goes in that area.
- Exemplar: `window::settings::handle`; test `window::only_the_windows_own_page_reaches_its_actions`.

## IPC

- `window::from_page` accepts IPC only from the top page.
- The minutes frame's messages are accepted only from its own `contentWindow`, each shape checked.

## Assets

- Page assets are compiled in from `crates/echoctl/ui/` (`window::reply`).
- Third-party JS is vendored in `ui/vendor/` with its licence and README entry.

## Parts

- The script and styles are parts per area, `ui/app/NN-<area>.js` and `ui/css/NN-<area>.css`, joined in name order at
  build time (`build.rs::join_parts`).
- A part is named `NN-<area>.js` (lowercase, digits, dashes) and ends with a newline; a new area takes a free two-digit
  prefix.

## Tested blocks

- A tested block sits between `// ── <name>` and `// end of the <name> block`, with no DOM.
- Its test is `ui/<name>.test.mjs`, which slices the block from the joined script (`appSource()`) and is listed in
  `scripts/check-mod.sh`. Exemplar: `Palette` (window script), tested by `palette.test.mjs::Palette`; markers as in
  `workbench.test.mjs::ACTIVITIES`.

## Settings

- `window::settings::settings_set` saves a setting as the value shown.
- `window::settings_action` runs the tray's `Action` by name for a flow that needs a dialog, sign-in or download.

## Rendering

- `render`/`patch` (window script) replace a region only when its HTML changed; a focused editor is updated in place.
- `press` runs a palette action as its button's click.
- Card scroll: `.tp-list > .set-card` (window styles).

## Notes, leaving, Inbox

- `calls::save_my_notes`: skips an unchanged `my-notes.md`; a save that meets an outside change keeps the other version
  as a file beside it.
- `tray::Leaving`: one close, quit or update at a time.
- `attention::view`: an Inbox item becomes Done when its source was read.
