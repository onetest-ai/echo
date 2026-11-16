---
id: TC-010
title: Drop on a Won't do draft (a mail reply) leaves nothing; the item stays decided
priority: medium
type: functional
requirements: [F-TODAY-08]
module: today
run: agent
platform: any
data: standard
tags: [today, needs, wont-do, claude]
---

# TC-010: Drop on a Won't do draft (a mail reply) leaves nothing; the item stays decided

**Module:** Today | **Priority:** Medium | **Type:** Functional

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

The fake's answer for the comment `reply that legal reviews the redlines`: for "the legal team", why "legal reviews contract redlines", a mail draft "Hi Kim, our legal team reviews the redlines: please send them to legal directly."

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, press `ControlOrMeta+k`, type `Won't do: Kim Park`, press Enter (⌘K's NEEDS YOU NOW item for the row "Mail to you"), type `reply that legal reviews the redlines`, click the dialog's "Won't do" | The row leaves Needs you now; Asks shows a line "Won't do" for "Mail to you" |
| 2 | Wait up to 30 s for the line to change | "Won't do · for the legal team: legal reviews contract redlines"; "A draft by mail — nothing is sent until you press Send"; the draft "Hi Kim, our legal team reviews the redlines: please send them to legal directly."; "Send", "Edit", "Drop" |
| 3 | Click "Drop" | The line is gone; with no other line, no Asks section |
| 4 | Run `. <dir>/env && python3 -c "import json,os; h=os.environ['ECHOCTL_HOME']; print([x['id'] for x in json.load(open(h+'/.echo/task-asks.json')) if x.get('kind')=='wont'], json.load(open(h+'/.echo/needs.json'))['decided']['mail:globex-redlines']['how'])"` | Prints `[] wont`: nothing of the line is left; the item stays decided |
| 5 | Reload `{{base_url}}/#today` | No Asks section; "Mail to you" is not back in Needs you now |

## Expected Final State
Kim Park's mail is decided (Won't do); its Asks line and draft are gone; nothing was sent.
