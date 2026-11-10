---
id: TC-014
title: A new meeting type is created, shown as Yours, and deleted again
priority: medium
type: functional
requirements: [F-TPL-05]
module: templates
run: agent
platform: any
tags: [templates, meeting-types]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-014: A new meeting type is created, shown as Yours, and deleted again

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/meeting-types.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click the tab "Meeting types 6" and click "New type" | An editor "New meeting type" opens with the boxes "Name" and "How to tell it" |
| 2 | Type `Steering committee` in "Name" and `Sponsors and leads reviewing a programme monthly.` in "How to tell it", click "Create" | A seventh row "Steering committee" with the tag "Yours"; the tab reads "Meeting types 7" |
| 3 | Run `cat "$ECHOCTL_HOME/.echo/meeting-types.json"` in Bash (`. <dir>/env` first) | The JSON has `"id": "steering-committee"` and the sentence |
| 4 | Click "Delete" in the editor | The row is gone; the tab reads "Meeting types 6" |
| 5 | Run the `cat` again | The file holds `[]` |

## Expected Final State
The type was created and removed; six types remain.

## Teardown
- If a row "Steering committee" remains, open it and click "Delete".
