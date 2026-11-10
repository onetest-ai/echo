---
id: TC-008
title: A Won't do comment without a request gives an Asks line "Won't do · for X: why" and no draft
priority: high
type: functional
requirements: [F-TODAY-04, F-TODAY-07]
module: today
run: agent
platform: any
data: standard
tags: [today, needs, wont-do, claude]
---

# TC-008: A Won't do comment without a request gives an Asks line "Won't do · for X: why" and no draft

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

The fake's answer for the comment `presales owns this deal`: for "the presales team", why "presales owns this deal", no draft.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, press `ControlOrMeta+k`, type `Won't do: Lena Vogel`, press Enter (⌘K's NEEDS YOU NOW item for the row "@you in Initech hiring"), type `presales owns this deal`, click the dialog's "Won't do" | The dialog closes; the row "@you in Initech hiring" leaves Needs you now at once; a section "Asks" appears with a line "Won't do" for "@you in Initech hiring" (saying "Claude is noting who it's for…" while the run works) |
| 2 | Wait up to 30 s for the line to change | The line reads "Won't do · for the presales team: presales owns this deal", under it "@you in Initech hiring · Lena Vogel"; it has no draft (`.wd-draft`), no "Send", "Edit" or "Drop" button — only × ("Drop") |
| 3 | Run `. <dir>/env && python3 -c "import json,os; h=os.environ['ECHOCTL_HOME']; n=json.load(open(h+'/.echo/needs.json'))['decided']; a=[x for x in json.load(open(h+'/.echo/task-asks.json')) if x.get('kind')=='wont']; print(n['teams:19:initech-hiring@thread.v2:m1']['how'], n['teams:19:initech-hiring@thread.v2:m1']['note']); print(a[0]['state'], a[0]['for'], a[0]['draft'])"` | Prints `wont presales owns this deal` and `done the presales team None`: decided with the comment kept, the run's outcome without a draft |
| 4 | Reload `{{base_url}}/#today` | The Asks line is still there; "@you in Initech hiring" is not back in Needs you now |
| 5 | Click × on the line | The line is gone; no Asks section |

## Expected Final State
The item stays decided (Won't do, with its comment); its Asks line was dropped.

## Notes
- The run has no tools at all: it can't send, post or mail; Echo only records its answer (`today.rs` `wont_do`, Rust tests for the prompt and the answer).
