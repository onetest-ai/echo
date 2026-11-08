---
id: TC-002
title: + Person adds a named person with an address to the thread
priority: medium
type: functional
requirements: [F-PPL-01]
module: people
run: agent
platform: any
tags: [people, add]
---

# TC-002: + Person adds a named person with an address to the thread

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- In S-07: `people-lab` is the scenario's shared thread, made by stage 10's precondition (people/TC-001) and kept (other people may be in it already); skip making it. On its own: make it as above

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |
| Person | Pat Novak <pat@acme.example> |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people`, then click "List" in the view switch (`role=group[name="View"]`) | "List" is pressed |
| 2 | Click "+ Person" | An input named "New person" with the placeholder "Name, or name <email>" |
| 3 | Type `Pat Novak <pat@acme.example>` and press Enter. Wait up to 3 s after an action for the window to redraw before reading. | The table in the top card has a row "Pat Novak" with "pat@acme.example" and the organisation "Acme"; the count on the tab "People" goes up by one (to 1 on a thread with nobody else in it) |
| 4 | Run `echoctl project people people-lab` | `people` has an entry `"name": "Pat Novak"` with `"in_thread": true` |

## Expected Final State
Pat Novak is in the thread.

## Teardown
- `echoctl project delete people-lab`

## Changes
- Adds Pat Novak to `people-lab`.
