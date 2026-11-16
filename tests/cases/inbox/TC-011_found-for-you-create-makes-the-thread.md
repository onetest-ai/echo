---
id: TC-011
title: Create on a found thread makes the thread and takes the item out of the Inbox
priority: high
type: functional
requirements: [F-INBOX-04]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, found, decision]
---

# TC-011: Create on a found thread makes the thread and takes the item out of the Inbox

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), within 10 seconds, no reload: the sign-in check of the AI profiles settles in the background and the window reads the Inbox again as soon as it does (#513); before it, the answer lists 24 open items and none blocking

## Test Data

| Field | Value |
|---|---|
| Item | New thread? “Acme calls” (group Acme) |
| New thread | id `acme-calls`, client Acme, stage active |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Click "Create" on "New thread? “Acme calls”" | The item shows "Working…" or is gone |
| 3 | Wait up to 5 seconds | The item is no longer under Acme; the chip reads "Open 24" (or less if more items left with the new thread) |
| 4 | Run `echoctl project list` | A project `acme-calls` with the name "Acme calls", client `acme`, stage `active` and its calls (11) |
| 5 | Navigate to `{{base_url}}/#projects` and wait for "Acme rollout" | "Acme calls" is listed under the client Acme |

## Expected Final State
The thread Acme calls exists.

## Teardown
- Click "Inbox" to close the panel if open
- Run `echoctl project delete acme-calls` (moves the thread to the trash; its calls are untouched)

## Notes
- Creates a thread (no Claude Code, no network). The thread's calls are the 11 calls with people from acme.example.
