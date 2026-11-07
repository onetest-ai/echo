---
id: TC-007
title: "Same person as…" merges two people; "Not the same" splits an address off
priority: low
type: functional
requirements: [F-PPL-07]
module: people
run: agent
platform: any
tags: [people, merge]
---

# TC-007: "Same person as…" merges two people; "Not the same" splits an address off

**Module:** People | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- Add the duplicate and the original to the thread: in the List "Add" on Dana Lee, then "+ Person" `Dana L <dlee@acme.example>`

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |
| People | Dana Lee <dana@acme.example> and Dana L <dlee@acme.example> |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` and click "List" in the view switch (`role=group[name="View"]`) | Rows "Dana Lee" and "Dana L" in the thread's table |
| 2 | Click the row "Dana L", then "Same person as…" in the panel | A select "Same person as" (first option "Same person as…") and buttons "Merge", "Cancel" |
| 3 | Choose "Dana Lee · dana@acme.example" and click "Merge". Wait up to 3 s after an action for the window to redraw before reading. | The list holds one row, "Dana Lee" (the one seen in more calls); the panel of Dana Lee shows two addresses `dana@acme.example` and `dlee@acme.example` with a button "Not the same" beside the second |
| 4 | Click "Not the same". Wait up to 3 s after an action for the window to redraw before reading. | Two rows "Dana Lee" and "Dana L" are listed again |

## Expected Final State
Dana Lee and Dana L are two people again.

## Teardown
- `echoctl project delete people-lab`

## Changes
- Merges and splits two people of `people-lab`.
