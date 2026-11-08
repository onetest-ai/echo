---
id: TC-004
title: A person dragged on the Map to "Manage closely" gets high influence and interest
priority: medium
type: functional
requirements: [F-PPL-02]
module: people
run: agent
platform: any
tags: [people, map]
---

# TC-004: A person dragged on the Map to "Manage closely" gets high influence and interest

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- In S-07: `people-lab` is the scenario's shared thread, made by stage 10's precondition (people/TC-001) and kept; skip making it. On its own: make it as above
- Put Dana Lee in the thread: `+ Person` with `Dana Lee <dana@acme.example>` (or "Add" in the List). In S-07 she is in it since stage 11 (people/TC-003)
- The people view is Map: click "Map" in the view switch if another is pressed (the choice is kept in the browser for every thread)

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |
| Person | Dana Lee |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` | The Map has four quadrants "Keep satisfied", "Manage closely", "Monitor", "Keep informed"; a tray "NOT PLACED YET" holds the chip "Dana Lee" |
| 2 | Drag the chip "Dana Lee" (`[data-pp-drag="dana@acme.example"]`) onto the quadrant "Manage closely" (`[data-pp-drop="3/3"]`). Wait up to 3 s after an action for the window to redraw before reading. | The chip sits in "Manage closely"; the tray is gone |
| 3 | Run `echoctl project people people-lab` | Dana Lee's `places` entry for `people-lab` has `"influence": 3` and `"interest": 3` |
| 4 | Drag the chip onto the tray (`[data-pp-drop="tray"]`) — if no tray is shown, drag it onto "Monitor" (`[data-pp-drop="1/1"]`) | The chip is in "Monitor" (low influence, low interest) |

## Expected Final State
Dana Lee is placed in Monitor.

## Teardown
- `echoctl project delete people-lab`

## Changes
- Places Dana Lee in `people-lab`.

## Notes
Once placed, the tray only shows people who are not placed; the second drop therefore goes to Monitor.
