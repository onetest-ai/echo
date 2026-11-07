---
id: TC-005
title: Client Profile → + New thread makes a thread under that client
priority: medium
type: functional
requirements: [F-CLIENT-05]
module: clients
run: agent
platform: any
tags: [clients, profile, threads]
---

# TC-005: Client Profile → + New thread makes a thread under that client

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- No thread `globex-support` exists

## Test Data

| Field | Value |
|-------|-------|
| Client | Globex |
| Thread | Globex support |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#client/globex/profile` and click "+ New thread" in the Threads row | An input "New thread name" with the placeholder "New thread for Globex" |
| 2 | Type `Globex support` and press Enter. Wait up to 3 s after an action for the window to redraw before reading. | The URL ends with `#project/globex-support/profile`; the heading "Globex support" with the tag "Globex" |
| 3 | Run `echoctl project show globex-support` | `"client": "globex"` |

## Expected Final State
Thread Globex support exists under Globex.

## Teardown
- `echoctl project delete globex-support`

## Changes
- Creates thread `globex-support`.
