---
id: TC-006
title: Items are grouped by client then Everything else; the blocking item stands first outside the groups
priority: high
type: functional
requirements: [F-INBOX-03]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, groups]
---

# TC-006: Items are grouped by client then Everything else; the blocking item stands first outside the groups

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Wait 10 seconds after the server starts: the sign-in check of the AI profiles settles in the background, and only then the blocking item "Sign in to Claude · out" is in the Inbox (the very first answer lists 24 open items and none blocking)

## Test Data

| Part | Header | Count | Kinds in order |
|---|---|---|---|
| Blocking | (none) | 1 | signin |
| Acme | Acme | 15 | people (10), work, found, sender (2), transcript |
| Duff | Duff | 1 | found |
| Rest | Everything else | 8 | minutes, found (4), health, plugin-update, plugin |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Read the first row of the Open list | "Sign in to Claude · out" (class `urgent`), above any group header |
| 3 | Read the group headers (`.ny-grp .cap`) in order | "Acme" with 15, "Duff" with 1, "Everything else" with 8; then the separator "Done this week" with 2 |
| 4 | Read the kinds of the Acme group | In this order: ten people questions (five shown), "3 to review in Acme rollout's work", "New thread? “Acme calls”", the senders "Acme News's mail isn't authenticated" and "Dana Lee isn't read yet", "Teams transcript: Acme design review" |
| 5 | Read the Everything else group | "Minutes failed: Acme security review", four "New thread?" items (“Northwind Analysts calls”, “Umbrella lead notes 1”, “Initrode calls”, “Design guild”), "Claude Code has an older Echo plugin", "Crm Sync 1.1.0 is available", "Review today's standup list" |
| 6 | Read the Duff group | "New thread? “Duff calls”" |

## Expected Final State
The Inbox shows its groups.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Creates or changes nothing.
