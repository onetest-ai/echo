---
id: TC-013
title: I'll do it adds the task in the chosen thread, and the dialog keeps what you typed and chose while Today is drawn again
priority: high
type: functional
requirements: [F-TODAY-04]
module: today
run: agent
platform: any
data: standard
tags: [today, needs, ill-do-it]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-013: I'll do it adds the task in the chosen thread, and the dialog keeps what you typed and chose while Today is drawn again

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie). `<dir>` is the data folder (`. <dir>/env` sets `ECHOCTL_HOME`)
- Microsoft 365 is not signed in: the Needs you now items come from the data's `.echo/needs.json`
- Needs you now is `section.nd` on Today; a row is `.nd-row`, Won't do and I'll do it are reached from ⌘K (`ControlOrMeta+k`), its NEEDS YOU NOW group ("Won't do: <sender> — …", "I'll do it: <sender> — …"); the row's own button is ✦ (Claude). The dialog: heading "A task of yours", box `#nd-title`, thread `#nd-thread`, button `[data-needs-todo-ok]`
- Runs alone, not as a stage of S-24: it takes "Mail to you", which S-24's Won't do stages need

## Test Data

| Item (row) | Id | Its thread |
|---|---|---|
| Mail to you — "Contract redlines for the Globex pilot" | `mail:globex-redlines` | Globex pilot |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Needs you now", press `ControlOrMeta+k`, type `I'll do it: Kim Park`, press Enter (⌘K's NEEDS YOU NOW item for the row "Mail to you") | A dialog "A task of yours": "What to do" holds "Contract redlines for the Globex pilot"; "In which thread" shows "Globex pilot"; "Cancel" and "Add to Tasks" |
| 2 | Replace the text in "What to do" with `Send Kim the signed redlines`; choose "Initech hiring" in "In which thread"; keep references to the button and to Today's page (`const b = document.querySelector('[data-needs-todo-ok]'), w = document.querySelector('#r-primary [data-wkey="core:calendar.agenda"]')`, Your day) | — |
| 3 | Wait until the clock's minute changes (up to 65 s; the line "now" on Your day shows the new minute) | Today was drawn again, only the widgets whose content moved: Your day's is replaced (`w.isConnected` is false); the dialog is still open with the same button element (`b.isConnected` is true); "What to do" still holds `Send Kim the signed redlines`; the thread is still "Initech hiring" |
| 4 | Click "Add to Tasks" | The dialog closes; the row "Mail to you" is gone from Needs you now and stays gone after a reload of `{{base_url}}/#today` |
| 5 | Run `. <dir>/env && tail -1 "$ECHOCTL_HOME/projects/initech-hiring/ledger/events.jsonl"` | A `create` of a `task` titled "Send Kim the signed redlines", due today, its description "From Kim Park (Mail to you): Contract redlines for the Globex pilot", under the story "From Teams and mail" (epic "Follow-ups") |
| 6 | Navigate to `{{base_url}}/#tasks`, find "Send Kim the signed redlines" | The task is listed in the thread Initech hiring |

## Expected Final State
The thread Initech hiring has the task (and the Follow-ups epic and its story if it had none); Needs you now holds two items.

## Notes
- Before the fix, Today's dialogs were part of Today's page, which is drawn again each minute (the line "now"): a click as it was drawn was lost, and what was typed went back to the item's text.
- Won't do's dialog: `TC-007`.
