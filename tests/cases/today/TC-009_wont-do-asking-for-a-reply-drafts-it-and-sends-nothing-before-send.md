---
id: TC-009
title: A Won't do comment that asks for a reply gives a draft with Send, Edit and Drop; nothing is sent before Send
priority: high
type: functional
requirements: [F-TODAY-07, F-TODAY-08]
module: today
run: agent
platform: any
data: standard
tags: [today, needs, wont-do, claude, m365]
---

# TC-009: A Won't do comment that asks for a reply gives a draft with Send, Edit and Drop; nothing is sent before Send

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie). `<dir>` is the data folder (`. <dir>/env` sets `ECHOCTL_HOME`)
- Microsoft 365 is not signed in: the Needs you now items come from the data's `.echo/needs.json`. Claude Code is the data's fake: a Won't do run answers from `<dir>/fake-answers.tsv` by a phrase of the comment (`manifest.json` `needs`), any other run is refused
- Needs you now is `section.nd` on Today; a row is `.nd-row`, Won't do and I'll do it are reached from ⌘K (`ControlOrMeta+k`), its NEEDS YOU NOW group ("Won't do: <sender> — …", "I'll do it: <sender> — …"); the row's own button is ✦ (Claude). The dialog: heading "Won't do", box `#nd-note`, button `[data-needs-wont-ok]`. Asks is `section.wd`, a line `.wd-row`, a draft `.wd-draft`

## Test Data

| Item (row) | Id | From |
|---|---|---|
| Chat with Dana Lee — "Can you send me the renewal price list for Acme?" | `teams:19:dana-lee@unq.gbl.spaces` | Dana Lee, Teams 1:1, thread Acme rollout |
| Mail to you — "Contract redlines for the Globex pilot" | `mail:globex-redlines` | Kim Park, mail, thread Globex pilot |
| @you in Initech hiring — "Can you review the support hours draft by Friday?" | `teams:19:initech-hiring@thread.v2:m1` | Lena Vogel, Teams group chat, thread Initech hiring |

The fake's answer for the comment `tell her to ask Priya`: for "Priya Nair", why "Priya owns the Acme renewal pricing", a Teams draft "Hi Dana, Priya Nair owns the renewal pricing now: please ask her for the price list."

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, press `ControlOrMeta+k`, type `Won't do: Dana Lee`, press Enter (⌘K's NEEDS YOU NOW item for the row "Chat with Dana Lee"), type `tell her to ask Priya`, click the dialog's "Won't do" | The row leaves Needs you now; Asks shows a line "Won't do" for "Chat with Dana Lee": "Claude is noting who it's for…" while the run works, or already step 2's line when the run was quicker than the look (either passes) |
| 2 | Wait up to 30 s for the line to change | "Won't do · for Priya Nair: Priya owns the Acme renewal pricing"; "A draft in Teams — nothing is sent until you press Send"; the draft "Hi Dana, Priya Nair owns the renewal pricing now: please ask her for the price list."; buttons "Send", "Edit", "Drop" |
| 3 | Run `. <dir>/env && python3 -c "import json,os; a=[x for x in json.load(open(os.environ['ECHOCTL_HOME']+'/.echo/task-asks.json')) if x.get('kind')=='wont']; print(a[0]['state'], a[0]['draft']['channel'], a[0]['to']['chat'])"` | Prints `done teams 19:dana-lee@unq.gbl.spaces`: a draft, not sent, for the item's own chat |
| 4 | Click "Edit" | The draft becomes a box (`#wd-text`) holding its text; "Edit" is gone; "Send" and "Drop" stay |
| 5 | Add ` Thanks!` at the end of the box's text, click "Send" | A notice says Microsoft 365 isn't signed in ("not signed in to Microsoft 365 …"): the send reached the app, which sends through Microsoft 365 only. The line keeps the box with your text (ending "Thanks!") and "Send"; it doesn't say "Sent in Teams" |
| 6 | Run the command of step 3 again | Prints `done teams 19:dana-lee@unq.gbl.spaces`: nothing was sent |
| 7 | Click "Drop" | The line is gone |

## Expected Final State
Dana Lee's item is decided (Won't do); nothing was sent; the line was dropped.

## Notes
- Send itself needs a Microsoft 365 sign-in: with one, the draft goes into the Teams chat (or as a reply to the mail) and the line says "Sent in Teams" / "Sent by mail". That part is person-run.
- The draft goes to the item's own chat or mail, kept by Echo; the model's answer never names where it goes.
