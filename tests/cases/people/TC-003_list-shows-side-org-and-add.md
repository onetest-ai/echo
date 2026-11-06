---
id: TC-003
title: The List shows each person's organisation, calls and what they owe; Add brings a seen person into the thread
priority: medium
type: functional
requirements: [F-PPL-04]
module: people
run: agent
platform: any
tags: [people, list]
---

# TC-003: The List shows each person's organisation, calls and what they owe; Add brings a seen person into the thread

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` and click "List" in the view switch (`role=group[name="View"]`) | The block "SEEN IN CALLS" has a table with the columns Person, Organisation, Role, Stance, Calls, Talk, Owes, Last seen |
| 2 | Read the row "Dana Lee" | "dana@acme.example", organisation "Acme", Calls "1/1", Last seen "today", a button "Add" |
| 3 | Read the row "Omar Haddad" | Organisation "Acme", Calls "1/1", Owes "1", a button "Add" |
| 4 | Click "Add" on the row "Dana Lee". Wait up to 3 s after an action for the window to redraw before reading. | "Dana Lee" is in the top card (the thread's people); "SEEN IN CALLS" now says "1 isn't in this thread yet" and holds only Omar Haddad |

## Expected Final State
Dana Lee is in the thread, Omar Haddad is only seen in its calls.

## Teardown
- `echoctl project delete people-lab`

## Changes
- Adds Dana Lee to `people-lab`.
