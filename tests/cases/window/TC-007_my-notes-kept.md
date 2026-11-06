---
id: TC-007
title: A note added in My notes is kept with the call
priority: high
type: functional
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
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | The right panel "My notes" says it has none yet |
| 2 | Click "Add a note — Enter to add", type `Ask Dana about the Berlin office date`, press Enter | The note is listed in My notes; the state reads "Saving…" then "Saved" |
| 3 | Reload `{{base_url}}/#calls` and open "Acme weekly sync" again | The note is still there |
| 4 | Run `cat "$ECHOCTL_HOME/calls/"*acme-weekly-sync/my-notes.md` | It contains `Ask Dana about the Berlin office date` |

## Expected Final State
The note is in My notes after a reload and in the call's `my-notes.md`.

## Teardown
- Run `rm "$ECHOCTL_HOME/calls/"*acme-weekly-sync/my-notes.md`
