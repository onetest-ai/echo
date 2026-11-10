---
id: TC-007
title: Won't do opens a dialog that needs a comment, why or who it's for; Cancel and Escape keep the item
priority: high
type: functional
requirements: [F-TODAY-04]
module: today
run: agent
platform: any
data: standard
tags: [today, needs, wont-do]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-007: Won't do opens a dialog that needs a comment, why or who it's for; Cancel and Escape keep the item

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie). `<dir>` is the data folder (`. <dir>/env` sets `ECHOCTL_HOME`)
- Microsoft 365 is not signed in: the Needs you now items come from the data's `.echo/needs.json`. Claude Code is the data's fake: a Won't do run answers from `<dir>/fake-answers.tsv` by a phrase of the comment (`manifest.json` `needs`), any other run is refused
- Needs you now is `section.nd` on Today; a row is `.nd-row`, its Won't do button `[data-needs-wont="<item id>"]`. The dialog: heading "Won't do", box `#nd-note`, button `[data-needs-wont-ok]`. Asks is `section.wd`, a line `.wd-row`, a draft `.wd-draft`

## Test Data

| Item (row) | Id | From |
|---|---|---|
| Chat with Dana Lee — "Can you send me the renewal price list for Acme?" | `teams:19:dana-lee@unq.gbl.spaces` | Dana Lee, Teams 1:1, thread Acme rollout |
| Mail to you — "Contract redlines for the Globex pilot" | `mail:globex-redlines` | Kim Park, mail, thread Globex pilot |
| @you in Initech hiring — "Can you review the support hours draft by Friday?" | `teams:19:initech-hiring@thread.v2:m1` | Lena Vogel, Teams group chat, thread Initech hiring |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Needs you now" | Three rows: "Chat with Dana Lee", "Mail to you", "@you in Initech hiring", each with "I'll do it" and "Won't do" |
| 2 | Click "Won't do" on the row "Chat with Dana Lee" | A dialog "Won't do" with "Chat with Dana Lee — Can you send me the renewal price list for Acme?", the box "Why, or who it's for — needed" (empty), the line "Claude notes who it's for. It drafts a reply only when you ask for one here, and sends nothing: you do, from Today.", "Cancel" and "Won't do"; the dialog's "Won't do" (`[data-needs-wont-ok]`) is disabled |
| 3 | Type three spaces in the box | The dialog's "Won't do" is still disabled |
| 4 | Clear the box and type `x` | It is enabled. Clear the box again: disabled |
| 5 | Click "Cancel" | The dialog closes; "Chat with Dana Lee" is still in Needs you now; no Asks section (`section.wd`) |
| 6 | Click "Won't do" on the same row, type `x`, press Escape | The dialog closes; the row is still there |
| 7 | Run `. <dir>/env && python3 -c "import json,os; print(json.load(open(os.path.join(os.environ['ECHOCTL_HOME'], '.echo/needs.json'))).get('decided', {}))"` | Prints `{}`: nothing was decided |

## Expected Final State
Nothing changed: all three items are still in Needs you now.

## Notes
- The run after a confirmed Won't do: `TC-008` (no request in the comment), `TC-009` (a reply asked for), `TC-010` (Drop).
