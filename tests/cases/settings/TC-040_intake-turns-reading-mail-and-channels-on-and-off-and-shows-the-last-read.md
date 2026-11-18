---
id: TC-040
title: Settings → Intake turns reading mail and channels on and off and shows the last read, Sync now and Keep it current
priority: high
type: functional
requirements: [F-KNOW-02, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, intake]
---

# TC-040: Settings → Intake turns reading mail and channels on and off and shows the last read, Sync now and Keep it current

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`): Intake (Recall) is off and Microsoft 365 is not signed in
- `{{base_url}}/` was loaded just before the first window step
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config>/brain.json` key `enabled`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/brain` and wait for the heading "Intake" | The left list has "Intake" selected; the page's first heading is "Intake" (level 2); a card "Intake" has the row "Use Intake" with its switch off; there is no "Sync now" button and no row "Keep it current" |
| 2 | Click the switch "Use Intake" | Within 2 s the switch is on; the card shows a row "Last read" saying "Not read yet · sign in to Microsoft 365 (Calendar & chat) to read mail and channels" with a button "Sync now", and a row "Keep it current" with its own switch |
| 3 | Run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/brain.json"` | It holds `"enabled": true` |
| 4 | Click the switch "Use Intake" again | Within 2 s the switch is off; "Sync now" and "Keep it current" are gone |
| 5 | Run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/brain.json"` | It holds `"enabled": false` |

## Expected Final State
Intake is off, as it started.

## Teardown
- None (step 4 turned it off again). Do not press "Sync now": it would index the test data's calls.

## Notes
- Who may speak, in the same section: `settings/TC-041`.
- Reading mail and channels into fact files needs a real Microsoft 365 mailbox; the fact files, their backup and the move from the old index are Rust tests (`brain::facts::tests`).
