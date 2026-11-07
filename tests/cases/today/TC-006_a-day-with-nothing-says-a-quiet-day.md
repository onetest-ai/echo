---
id: TC-006
title: A day with no meetings, no calls and nothing on you says "A quiet day", and ⌘K on it offers no Today or My work items
priority: low
type: functional
requirements: [F-TODAY-06]
module: today
run: agent
platform: any
tags: [today, empty-state, palette]
---

# TC-006: A day with no meetings, no calls and nothing on you says "A quiet day", and ⌘K on it offers no Today or My work items

**Module:** Today | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh` (the `small` set); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Empty the day with the CLI (the small set's day holds 5 meetings, 2 calls and one task due today):
  - `target/debug/echoctl agenda clear` → "agenda: 0 meeting(s)"
  - `for c in $(ls "$ECHOCTL_HOME/calls" | grep "^$(date +%Y-%m-%d)_"); do target/debug/echoctl delete "$c"; done` → Acme weekly sync and Initech planning go to the trash
  - `target/debug/echoctl project ledger acme-rollout edit '{"op":"edit","node":"T001","set":{"state":"done"}}'` → note its `mid`
- Reload `{{base_url}}/` after these

## Test Data

| Before | After the preconditions |
|---|---|
| Your day: 4 meetings and 2 calls | none |
| My work · this week: Send Acme the SSO checklist (due today) | done; Draft the Globex pilot plan has no date, so it isn't this week's |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today` | The greeting and today's date; then "A quiet day" with "No meetings today and nothing on you in Tasks."; no "Your day", no "My work · this week" |
| 2 | Press `ControlOrMeta+k` | The dialog with the chip "On: Today ×"; no group TODAY and no group MY WORK; the first group is ECHO ("Record the next Teams call", "Record a memo", …) |
| 3 | Press Escape | The dialog closes |

## Expected Final State
Today says the day is quiet; the palette offers only Echo's own actions and Go to.

## Teardown
- `target/debug/echoctl restore <each deleted call>` (the names `delete` printed), and `target/debug/echoctl project ledger acme-rollout edit '{"op":"undo","target":"<the edit's mid>"}'`
- The agenda stays empty (it can't be put back without a file): run this case last on its data

## Notes
- The served window has no Microsoft 365, so the day's meetings are the agenda file's: emptied with `agenda clear`.
- A busy Today's ⌘K items are `TC-005`.
