---
id: TC-006
title: A client without threads is removed after a confirmation; its link then says "This client is gone"
priority: medium
type: functional
requirements: [F-CLIENT-06]
module: clients
run: agent
platform: any
tags: [clients, remove]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-006: A client without threads is removed after a confirmation; its link then says "This client is gone"

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `echoctl client add Hooli --domains hooli.example`

## Test Data

| Field | Value |
|-------|-------|
| Client | Hooli (`hooli`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#client/hooli/profile` | Heading "Hooli"; the Threads row has "+ New thread" and the button "Remove this client…" |
| 2 | Click "Remove this client…" | "Remove Hooli?" with the buttons "Cancel" and "Remove" |
| 3 | Click "Cancel" | The question is gone; the button "Remove this client…" is back |
| 4 | Click "Remove this client…" then "Remove". Wait up to 3 s after an action for the window to redraw before reading. | The URL ends with `#projects`; the left list has no group "HOOLI" |
| 5 | Navigate to `{{base_url}}/#client/hooli/overview` | "This client is gone" and a link "Back to the threads" |

## Expected Final State
Hooli no longer exists.

## Teardown
- None needed; if it failed: `echoctl client remove hooli`

## Changes
- Creates and removes client `hooli`.
