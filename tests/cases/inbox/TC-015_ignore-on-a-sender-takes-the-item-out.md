---
id: TC-015
title: Ignore on a sender to trust takes the item out of the Inbox and lists it as done
priority: medium
type: functional
requirements: [F-INBOX-04, F-INBOX-07]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, sender, decision]
---

# TC-015: Ignore on a sender to trust takes the item out of the Inbox and lists it as done

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), within 10 seconds, no reload: the sign-in check of the AI profiles settles in the background and the window reads the Inbox again as soon as it does (#513); before it, the answer lists 24 open items and none blocking

## Test Data

| Field | Value |
|---|---|
| Item | Dana Lee isn't read yet (Acme · sender), dana@acme.example · 3 messages · “SSO checklist” |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Click "Ignore" on "Dana Lee isn't read yet" | Within 5 seconds the item is gone; "Open" counts one less |
| 3 | Click the chip "Done" | "Dana Lee isn't read yet" is listed, struck through, with "today" |
| 4 | Read the other sender in Open | "Acme News's mail isn't authenticated" is still there with "Read anyway" and "Keep out" |

## Expected Final State
The sender is decided.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Writes the sender's decision (`brain_sender`); Recall is off in the data, no mail is read.
