---
id: TC-027
title: My notes' timeline keeps where it was scrolled when the panel is drawn again, and each note's time is a small chip inline before its text
priority: high
type: functional
requirements: [F-NOTES-12]
module: window
run: agent
platform: any
data: standard
tags: [my-notes, live-notes, scroll]
---

# TC-027: My notes keeps its scroll through redraws and shows times inline

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)` (today's Acme weekly sync: kept live notes and the user's notes, more than the panel holds); `B=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -2 | head -1)` (last week's)
- Viewport 1201x600 (the list scrolls). My notes is the right panel `aside#r-right`, opened with `[data-panel-flip="notes"]`; if it is already open, leave it open

## Test Data

| Where | Holds |
|---|---|
| `$A/my-notes.md` | heading "During the call"; notes at 00:12, 00:31, 00:45, 00:58, 01:10, 01:30, one without a time |
| `$A/live-notes.json` | ✦ items at 00:45, 02:31, 04:10 |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#call/<A>/minutes`, open My notes, and run `browser_evaluate` with `() => { const a = document.querySelector('#r-right #nt-list .nt-entry .at'); const s = getComputedStyle(a); return { display: s.display, narrow: a.getBoundingClientRect().width < a.parentElement.getBoundingClientRect().width / 2, sameLine: Math.abs(a.getBoundingClientRect().top - a.parentElement.querySelector('.nt-text').getBoundingClientRect().top) < 6 } }` | `display` "inline", `narrow` `true`, `sameLine` `true`: the time (00:12) is a small chip on the line of its text, not a bar across the column; the same for a ✦ row's time (`.nt-livei .at`) |
| 2 | Run `browser_evaluate` with `async () => { const l = document.querySelector('#nt-list'); l.scrollTop = 300; document.querySelector('[data-note-kind="question"]').click(); await new Promise((r) => setTimeout(r, 300)); const l2 = document.querySelector('#nt-list'); const out = { redrawn: l2 !== l, top: l2.scrollTop }; document.querySelector('[data-note-kind="note"]').click(); return out }` | `redrawn` `true` (the panel was drawn again, as on a live update), `top` 300: the list stayed where it was scrolled |
| 3 | Run `browser_evaluate` with `async () => { const l = document.querySelector('#nt-list'); l.scrollTop = l.scrollHeight; document.querySelector('[data-note-kind="todo"]').click(); await new Promise((r) => setTimeout(r, 300)); const l2 = document.querySelector('#nt-list'); const out = l2.scrollTop + l2.clientHeight >= l2.scrollHeight - 2; document.querySelector('[data-note-kind="note"]').click(); return out }` | `true`: a list scrolled to its end stays at its end |
| 4 | Click in the note box `#note-input`, type "Ask Dana about Berlin", then run `browser_evaluate` with `async () => { const l = document.querySelector('#nt-list'); l.scrollTop = 250; const i = document.querySelector('#note-input'); i.setSelectionRange(4, 4); S.call.live.notes.now = 'A newer Now'; render(); await new Promise((r) => setTimeout(r, 200)); const i2 = document.querySelector('#note-input'); return { top: document.querySelector('#nt-list').scrollTop, value: i2.value, focused: document.activeElement === i2, caret: i2.selectionStart } }`; then press Escape in the box | `top` 250, `value` "Ask Dana about Berlin", `focused` `true`, `caret` 4: a live update neither scrolls the list nor disturbs typing; Escape empties the box |
| 5 | Scroll `#nt-list` to 300, then click the row `[role=option][data-call="<B>"]` and read `#nt-list`'s `scrollTop` | 0: another call's notes open at their top |

## Expected Final State
Nothing changed: no note was added (step 4's text dropped with Escape).

## Notes
- Read-only. While a call records, the panel is drawn again on each live update and with the time in the note box's hint; step 2's redraw (a kind chip) takes the same path.
