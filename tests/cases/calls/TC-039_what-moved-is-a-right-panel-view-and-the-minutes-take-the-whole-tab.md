---
id: TC-039
title: What moved is a right panel view with the count of changes, and the minutes take the whole Minutes tab
priority: medium
type: functional
requirements: [F-MIN-19, F-MIN-04, F-MIN-05]
module: calls
run: agent
platform: any
data: standard
tags: [calls, what-moved, minutes, panel]
---

# TC-039: What moved is a right panel view with the count of changes, and the minutes take the whole Minutes tab

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the top bar's chips can carry the same `data-call`)
- Viewport 1201x868. The right panel is `aside#r-right`; its tabs are `[data-panel-show="<view>"]`

## Test Data

| Call | Threads | What moved |
|---|---|---|
| Acme weekly sync (today) | Acme rollout (has read it) | 6 changes |
| Memo | Acme rollout, Globex pilot (neither has read it) | a line per thread with Sync now, no count |
| Initech planning | none | no view |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row Acme weekly sync; close the right panel if it is open (`[aria-label="Close the panel"]`); run `browser_evaluate` with `() => ({ side: !!document.querySelector('#minutes-side'), frame: Math.round(document.querySelector('#minutes-frame').getBoundingClientRect().width), pane: document.querySelector('#pane').clientWidth })` | The Minutes tab is selected; `side` `false` (no column beside the minutes); `frame` equals `pane` (within 2 px): the minutes take the whole tab |
| 2 | With the calls list shown, click the My notes button `[data-panel-flip="notes"]` in the call's header (the right panel opens beside the page); run the `browser_evaluate` of step 1 again; then close the panel (`[aria-label="Close the panel"]`) | `side` `false`; `frame` equals `pane` (within 2 px) and is at least 360 px: the minutes keep the whole tab with a right panel open, nothing beside them squeezes them (#102: they were 153 px, and 1 px with the calls list) |
| 3 | Read the header button `[data-panel-flip="moved"]` beside My notes | It shows its icon and the count "6"; its `aria-label` is "What moved · 6 changes" |
| 4 | Click it | The right panel opens on the view "What moved" (panel title); its tab `[data-panel-show="moved"]` carries "6"; the view `#r-right .mv` has the thread "Acme rollout" with "6 changes" and six cards |
| 5 | Read the first card | "Criterion met", `S001` "SSO for the Berlin pilot", the quote "The SSO checklist is done, we sent it to security on Monday" — Dana Lee, and the time chip "0:17" |
| 6 | Click that card's time chip (`#r-right .cc-row [data-at]`, the first) | The URL hash ends with `/transcript`: the transcript opens at that moment |
| 7 | Click the card's item `T001` "Send the SSO checklist" (`#r-right .cc-node`, the one with T001) | The right panel shows "Details" with "T001" and "Send the SSO checklist" |
| 8 | Click the panel tab `[data-panel-show="moved"]` | What moved is shown again, with its six cards |
| 9 | Click the row Memo (in the list, `[role=option][data-call$="-memo"]`) | The panel still shows What moved, now with two thread lines: Acme rollout "hasn't read this call yet" and Globex pilot "reads this call within a few minutes", each with Sync now (not pressed); no count on the tab or the header button |
| 10 | Click the row Initech planning | No What moved button in the header and no What moved tab in the panel (the call is in no thread); the panel shows another view |

## Expected Final State
Initech planning is open; nothing changed.

## Notes
- Read-only: Sync now is never pressed (it starts Claude Code).
- A call in no thread has no What moved view at all (as Also discussed has none without the Brain); a call in a thread always has it, saying per thread what changed, or why the thread hasn't read the call yet.
