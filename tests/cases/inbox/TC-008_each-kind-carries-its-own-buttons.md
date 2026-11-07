---
id: TC-008
title: Each kind of item carries its own buttons and an Open where it has a place
priority: high
type: functional
requirements: [F-INBOX-04, F-INBOX-05]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, buttons]
---

# TC-008: Each kind of item carries its own buttons and an Open where it has a place

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Wait 10 seconds after the server starts: the sign-in check of the AI profiles settles in the background, and only then the blocking item "Sign in to Claude · out" is in the Inbox (the very first answer lists 24 open items and none blocking)

## Test Data

| Item (title) | Kind | Buttons |
|---|---|---|
| Sign in to Claude · out | signin (blocking) | Sign in… (warn style), Open |
| Minutes failed: Acme security review | minutes | Write again, Show log, Open |
| Is Rita Gomez a champion? | people | Yes, No, Change…, Open |
| 3 to review in Acme rollout's work | work | Open → |
| New thread? “Acme calls” | found | Create, Not this, Open |
| Acme News's mail isn't authenticated | sender (unverified) | Read anyway, Keep out |
| Dana Lee isn't read yet | sender (trust) | Trust, Ignore |
| Teams transcript: Acme design review | transcript | Import, Not this, Open |
| Claude Code has an older Echo plugin | health | Update it, Open |
| Crm Sync 1.1.0 is available | plugin-update | Open → |
| Review today's standup list | plugin | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Read the buttons of "Sign in to Claude · out", "Minutes failed: Acme security review", "Is Rita Gomez a champion?" | As in Test Data; none is pressed |
| 3 | Read the buttons of "3 to review in Acme rollout's work" and "New thread? “Acme calls”" | "Open →" only; "Create", "Not this", "Open" |
| 4 | Read the buttons of the two senders | "Read anyway" and "Keep out"; "Trust" and "Ignore"; neither has an Open button |
| 5 | Read the buttons of "Teams transcript: Acme design review" and "Claude Code has an older Echo plugin" | "Import", "Not this", "Open"; "Update it", "Open" |
| 6 | Read the buttons of "Crm Sync 1.1.0 is available" and "Review today's standup list" | "Open →" only; no buttons at all |

## Expected Final State
The Inbox is open; no button was pressed.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Creates or changes nothing.
- The buttons that start Claude Code (Write again), open Terminal (Sign in…) or touch the machine (Update it) are only read; the ones that work offline are pressed in `TC-009` to `TC-015`.
- Sign-in, older-plugin and update items come from the real machine in a normal run (`F-INBOX-05`, person); here they are made by the data set.
