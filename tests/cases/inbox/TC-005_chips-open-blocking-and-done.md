---
id: TC-005
title: The chips Open, Blocking and Done carry their counts; Blocking and Done list only theirs
priority: high
type: functional
requirements: [F-INBOX-02]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, chips]
---

# TC-005: The chips Open, Blocking and Done carry their counts; Blocking and Done list only theirs

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), within 10 seconds, no reload: the sign-in check of the AI profiles settles in the background and the window reads the Inbox again as soon as it does (#513); before it, the answer lists 24 open items and none blocking

## Test Data

| Chip | Count | Lists |
|---|---|---|
| Open | 25 | everything open |
| Blocking | 1 | Sign in to Claude · out |
| Done | 2 | Dana Lee: supporter → champion · New thread? “Old standup” |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Read the three chips (`.fchip`) and the text beside them | "Open 25" (selected), "Blocking 1", "Done 2", and "this week" |
| 3 | Click "Blocking 1" | Exactly one item: "Sign in to Claude · out" with the text "Minutes of 1 call wait for it" and the buttons "Sign in…" (do not press it) and "Open" |
| 4 | Click "Done 2" | Two rows with a ✓ and struck-through titles: "Dana Lee: supporter → champion" and "New thread? “Old standup”"; no buttons |
| 5 | Click "Open 25" | The full list is back |

## Expected Final State
The Inbox is on Open.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Creates or changes nothing.
