---
id: TC-003
title: echoctl attention prints the same empty lists as the Inbox
priority: medium
type: functional
requirements: [F-INBOX-08]
module: inbox
run: agent
platform: any
tags: [inbox, cli]
---

# TC-003: echoctl attention prints the same empty lists as the Inbox

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Nothing waits for Sam and nothing was decided this week. Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Command | `echoctl attention` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl attention` | Exit code 0; JSON with `open` an empty list, `done` an empty list and `urgent` equal to 0 |
| 2 | Read `clients` in the output | It maps `acme` to "Acme", `globex` to "Globex" and `initech` to "Initech" |
| 3 | Navigate to `{{base_url}}/#today`, click "Inbox", and read the panel | "Nothing needs you": the window agrees with the JSON |

## Expected Final State
The CLI and the window both show no open item and nothing done.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Creates or changes nothing.
