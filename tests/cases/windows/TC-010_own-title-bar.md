---
id: TC-010
title: Echo's own title bar on Windows — one strip with min, max and close; drag, double-click, resize and the keyboard work; on by default, off in Settings → Layout
priority: medium
type: functional
requirements: [F-WIN-07]
module: windows
run: person
platform: windows
tags: [windows, window, title-bar]
---

# TC-010: Echo's own title bar on Windows

**Module:** Windows | **Priority:** Medium | **Type:** Functional

## Preconditions
- Windows 11 (and Windows 10 if at hand); made-up data: `python tests/fixtures/make_data.py <dir> --ai fake --now <today>T12:00`, its env loaded, `ECHOCTL_HOME` under `<dir>`
- `target\debug\echoctl app run` started from that shell (a fresh data set: its config has no `own_title_bar`); the window opened from the tray ("Open Echo…")
- On macOS there is no such row, and Settings search for "title bar" finds nothing: `node crates/echoctl/ui/own-title-bar.test.mjs` checks it (by the app's platform, also in `--serve` and on Linux)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Look at the window as it opens | No Windows title bar: Echo's strip is the top of the window, with − ▢ × at its right (46 px wide each, the height of the strip); the window keeps its shadow (and its rounded corners on Windows 11) |
| 2 | Open Settings → Layout; type "title bar" in Search settings, then clear it | A card "Window" with the row "Echo's own title bar", its switch on; the search keeps Layout with that row |
| 3 | Hover ×, then − and ▢ | × turns red with a white mark; − and ▢ a light grey (dark theme: a lighter grey); the icons follow the theme |
| 4 | Drag the strip's empty part | The window moves; dragging it to the top edge maximises it, to a side snaps it to half |
| 5 | Double-click the strip's empty part, then again | It maximises (filling the work area, the taskbar still visible; ▢ shows the restore mark and says "Restore"), then restores to where it was |
| 6 | Press ▢, then ▢ again; then − | Maximise, restore; − minimises (the taskbar button brings it back) |
| 7 | Drag each of the four edges (the top one at the strip's top pixels) and a corner | The window resizes from each (the top edge shows the up-down cursor); it stops at about 760 × 480 |
| 8 | Press Alt+Space (then Esc); Win+↑ / Win+↓ / Win+← / Win+→; Win+Z | The system menu opens at the top-left; the Win keys maximise, restore / minimise and snap left / right; Win+Z shows the snap layouts |
| 9 | Hover the taskbar button, then right-click it | Its preview shows the window; "Close window" goes through Echo's close (notes being typed are saved first) |
| 10 | Type a note in My notes, then press × | The window closes as with Alt+F4: the note is kept (open the window again to see it) |
| 11 | Turn the switch off in Settings → Layout | Windows' title bar is back at once; Echo's strip has no − ▢ × |
| 12 | Quit Echo and start it again | The window opens with Windows' title bar: off is kept |
| 13 | Turn the switch on again | Echo's own title bar at once, as in step 1 |

## Expected Final State
The switch is on (step 13); nothing else changed.

## Notes
- Snap layouts (Windows 11's flyout when hovering ▢) are not offered with Echo's own title bar (#76: WebView2 takes the mouse, so the window never answers `HTMAXBUTTON`); Win+Z and dragging to the screen's top still snap.
- Not checked by the agent run of #76: the taskbar's preview and right-click Close (step 9), a second monitor with another scale, Windows 10.
- macOS is unchanged: the strip there was always Echo's own, under the traffic lights; the setting doesn't exist there.
- After F5 / Ctrl+R while maximised, ▢ still shows the restore mark (the page asks the app at load, `window_sync`).
