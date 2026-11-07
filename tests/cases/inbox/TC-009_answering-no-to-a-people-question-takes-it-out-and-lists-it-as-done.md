---
id: TC-009
title: No on a people question takes the item out of Open and puts it in Done
priority: high
type: functional
requirements: [F-INBOX-04, F-INBOX-06, F-INBOX-07]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, people, decision]
---

# TC-009: No on a people question takes the item out of Open and puts it in Done

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Wait 10 seconds after the server starts: the sign-in check of the AI profiles settles in the background, and only then the blocking item "Sign in to Claude · out" is in the Inbox (the very first answer lists 24 open items and none blocking)

## Test Data

| Field | Value |
|---|---|
| Item | Is Rita Gomez a champion? (Acme · people, first of ten) |
| Counts before | Open 25, Done 2, Acme 15, people 10 |
| Counts after | Open 24, Done 3, Acme 14, people 9 ("4 more like these") |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Click "No" on "Is Rita Gomez a champion?" | The item shows "Working…" or is already gone |
| 3 | Wait up to 5 seconds | "Is Rita Gomez a champion?" is no longer under Acme; the chip reads "Open 24"; the Acme header 14; the fold button under the people reads "4 more like these" |
| 4 | Click the chip "Done" | "Done 3"; the first row is "Is Rita Gomez a champion?" (struck through) with "today" |
| 5 | Run `echoctl attention` | `open` has 24 items (no key for that question); `done` has 3 entries and the first title is "Is Rita Gomez a champion?" |

## Expected Final State
The question is decided and listed under Done.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Writes the decision to the thread's people suggestions (`people_decide`); no Claude Code, no network. The data is disposable: it has no undo here.
