---
id: TC-007
title: A note added in My notes is kept with the call
priority: high
type: functional
requirements: [F-NOTES-01, F-NOTES-02, F-NOTES-06]
module: window
run: agent
platform: any
tags: [notes]
external_id: TC-WIN-011
---

# TC-007: A note added in My notes is kept with the call

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |
| Note | Ask Dana about the Berlin office date |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync"; if the right panel is closed, open it with the "My notes" button | The My notes panel is open for this call and has no notes yet |
| 2 | Click "Add a note — Enter to add", type `Ask Dana about the Berlin office date`, press Enter | The note is listed in My notes; the state reads "Saving…" then "Saved" |
| 3 | Reload the page in the browser (a full reload, not a hash change), then open "Acme weekly sync" and My notes again | The note is still there |
| 4 | Run `cat "$ECHOCTL_HOME/calls/"*acme-weekly-sync/my-notes.md` | It contains `Ask Dana about the Berlin office date` |

## Expected Final State
The note is in My notes after a reload and in the call's `my-notes.md`.

## Teardown
- Run `rm "$ECHOCTL_HOME/calls/"*acme-weekly-sync/my-notes.md`
