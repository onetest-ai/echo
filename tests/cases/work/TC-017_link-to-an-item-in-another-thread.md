---
id: TC-017
title: "Link to an item in another thread" finds an open item and links it as the same work; Unlink undoes it
priority: medium
type: functional
requirements: [F-WORK-15]
module: work
run: agent
platform: any
tags: [work, details, cross-thread]
---

# TC-017: "Link to an item in another thread" finds an open item and links it as the same work; Unlink undoes it

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Item opened | Globex pilot T001 Draft the Globex pilot plan |
| Searched | SSO |
| Linked | Acme rollout T001 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work` and click the card T001 | Details shows "Draft the Globex pilot plan" and the button "Link to an item in another thread…" |
| 2 | Click that button | An input named "Find an item in another thread" with the placeholder "Words from its title, or its id" and a "Cancel" button |
| 3 | Type `SSO` | Two results, each with the buttons "Same work · done there", "Done here", "Waits on it": "Acme rollout · S001 SSO for the Berlin pilot" and "Acme rollout · T001 Send Acme the SSO checklist · Sam Rivera" (a task's result line ends with its owner) |
| 4 | In the result of T001, click "Same work · done there" (`[data-pj="xlink"][data-target="T001"][data-master="there"]`). Wait up to 3 s after an action for the window to redraw before reading. | Details has the row "Same as" with the link "Acme rollout · T001 Send Acme the SSO checklist" and a button "Unlink"; History has "you linked it to Acme rollout · T001 (the same work, done there)" |
| 5 | Click "Unlink". Wait up to 3 s after an action for the window to redraw before reading. | The row "Same as" is gone; History has "you unlinked Acme rollout · T001" |

## Expected Final State
The two tasks are separate again.

## Teardown
- If step 5 was not reached, click "Unlink"

## Changes
- Links then unlinks Globex pilot T001 and Acme rollout T001.
