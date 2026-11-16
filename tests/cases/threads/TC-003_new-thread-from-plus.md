---
id: TC-003
title: New thread (+) makes a thread with the chosen client and opens its Profile
priority: high
type: functional
requirements: [F-THR-04]
module: threads
run: agent
platform: any
tags: [threads, create]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-003: New thread (+) makes a thread with the chosen client and opens its Profile

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- No thread named "Initech hiring" exists (`echoctl project list` has no `initech-hiring`)

## Test Data

| Field | Value |
|-------|-------|
| Name | Initech hiring |
| Client | Initech |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#projects` and click "New thread" (`[data-pj="new"]`) | The URL ends with `#found`; the heading "Found for you" and an input named "New thread name" with the placeholder "New thread — name it, Echo finds its calls" are shown, next to a "Client" select (options None, Initech, Acme, Globex) and a "Create" button |
| 2 | Type `Initech hiring` into "New thread name" | The input holds `Initech hiring` |
| 3 | In the "Client" select choose `Initech` | The select shows Initech |
| 4 | Press Enter in the name input. Wait up to 3 s after an action for the window to redraw before reading. | The URL ends with `#project/initech-hiring/profile`; the heading "Initech hiring" with tag "Initech" and "Delivery · Active"; the facts line reads "0 calls" |
| 5 | Read the left list | Under "INITECH · YOUR COMPANY" a row "Initech hiring", "0 calls", "Active" |
| 6 | Run `echoctl project show initech-hiring` | JSON with `"name": "Initech hiring"`, `"client": "initech"`, `"stage": "active"` |

## Expected Final State
Thread Initech hiring exists under Initech, active, with no calls; its Profile is open.

## Teardown
- `echoctl project delete initech-hiring`; reload `{{base_url}}/#projects` and confirm the row is gone

## Changes
- Creates thread `initech-hiring` (client Initech).
