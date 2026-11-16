---
id: TC-014
title: Write again on failed minutes ends in a failure that names the fake Claude Code; the item stays with the new reason
priority: medium
type: functional
requirements: [F-INBOX-04, F-INBOX-06]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, minutes, fake-claude]
---

# TC-014: Write again on failed minutes ends in a failure that names the fake Claude Code; the item stays with the new reason

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), up to 120 seconds, reloading the page if it hasn't come after 60: the sign-in check of the AI profiles settles in the background and the window shows it at its next attention refresh, 25–90 s in all (#513); before it, the answer lists 24 open items and none blocking
- The data's `claude` is a fake that refuses every run

## Test Data

| Field | Value |
|---|---|
| Item | Minutes failed: Acme security review |
| Reason before | Claude Code exited with an error (exit 1) |
| Reason after | test data: Claude Code is a fake here |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Read the item "Minutes failed: Acme security review" | Its line reads "Claude Code exited with an error (exit 1)"; buttons "Write again", "Show log", "Open" |
| 3 | Click "Write again" and wait up to 15 seconds | The item is still listed (in Everything else) with the line "test data: Claude Code is a fake here" and its buttons back |
| 4 | Click "Open" on the item | The URL becomes `#call/<folder>/minutes`; the Minutes tab says "Couldn't write the minutes" with the same text |

## Expected Final State
The call shows the new failure.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Starts a run of the fake claude, which exits at once; no account is used. After this case `calls/TC-023`..`TC-025` would read the new message.
