---
id: TC-017
title: The Open view ends with a Done this week strip of two rows and All; the Done chip lists them with the day
priority: medium
type: functional
requirements: [F-INBOX-02, F-INBOX-07]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, done]
---

# TC-017: The Open view ends with a Done this week strip of two rows and All; the Done chip lists them with the day

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), up to 120 seconds, reloading the page if it hasn't come after 60: the sign-in check of the AI profiles settles in the background and the window shows it at its next attention refresh, 25–90 s in all (#513); before it, the answer lists 24 open items and none blocking
- Before any decision is taken in the window (`TC-009` to `TC-015` add rows to Done)

## Test Data

| Done row | Day |
|---|---|
| Dana Lee: supporter → champion | yesterday |
| New thread? “Old standup” | 2 days ago: a date, not "today" or "yesterday" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Scroll to the end of the Open list | A strip "Done this week" with 2 and a button "All ▸", then two rows: "Dana Lee: supporter → champion" and "New thread? “Old standup”" |
| 3 | Click "All ▸" | The chip "Done 2" is selected and the same two rows are listed alone |
| 4 | Read the day beside each row | "yesterday" for the first; a date (not "today", not "yesterday") for the second |
| 5 | Read the rows' look | A green ✓, the title struck through (`<s>`), no buttons |

## Expected Final State
The Done chip is selected.

## Teardown
- Click the chip "Open" and then "Inbox" to close the panel

## Notes
- Creates or changes nothing.
- Items gone for 7 days stay in Done; a source that could not be read keeps its items open instead (`F-INBOX-07`): the data cannot make an unreadable source.
