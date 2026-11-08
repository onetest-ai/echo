---
id: TC-016
title: A to-do's box in My notes ticks it done and back, and my-notes.md follows
priority: medium
type: functional
requirements: [F-NOTES-03]
module: window
run: agent
platform: any
data: standard
tags: [notes]
---

# TC-016: A to-do's box in My notes ticks it done and back, and my-notes.md follows

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); file reads run after `. <dir>/env`
- Today's Acme weekly sync holds notes of every kind; its folder: `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)`
- The My notes panel is `[aria-label="Right panel"]`; a to-do's box is a `role=checkbox` named "Mark done" (open) or "Mark to do" (done)
- **In S-29** the Calls list is already on screen, left by the stage before, so step 1's navigation is skipped and step 1 starts at its click; the My notes panel step 1 opens stays open for `window/TC-017`, `TC-021`, `TC-018` and `TC-019`. On its own, step 1 navigates

## Test Data

| To-do | Before |
|---|---|
| 00:58 Remind Omar about the access list | open: `- [ ] [00:58] Remind Omar about the access list` |
| 01:10 Send the agenda | done: `- [x] [01:10] Send the agenda` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the first row `[role=option][data-call$="-acme-weekly-sync"]` (today's), click "My notes" (`[aria-label="My notes"]`) if the panel isn't open | My notes lists "During the call" and 7 notes of yours (with the call's 3 live items, ✦, among them by time: 10 rows); "Remind Omar about the access list" has a box ☐ named "Mark done" (`aria-checked="false"`), "Send the agenda" a box ☑ named "Mark to do" (`aria-checked="true"`) |
| 2 | Click the box of "Remind Omar about the access list"; wait 2 s | Its box is ☑ "Mark to do"; "Send the agenda" is unchanged (☑); the state reads "Saved" |
| 3 | Run `grep -n 'Remind Omar\|Send the agenda' "$ECHOCTL_HOME/calls/$A/my-notes.md"` | `- [x] [00:58] Remind Omar about the access list` and `- [x] [01:10] Send the agenda` |
| 4 | Click the box of "Remind Omar about the access list" again; wait 2 s | Its box is ☐ "Mark done"; "Send the agenda" stays ☑ |
| 5 | Run the grep of step 3 again | `- [ ] [00:58] Remind Omar about the access list` and `- [x] [01:10] Send the agenda` |
| 6 | Click the box of "Send the agenda"; wait 2 s; grep again | Its box is ☐; the file has `- [ ] [01:10] Send the agenda`; Remind Omar stays `- [ ]` |
| 7 | Click the box of "Send the agenda" again; wait 2 s | ☑ again; the file has `- [x] [01:10] Send the agenda` |

## Expected Final State
Both to-dos are as they were; each tick and untick was saved to `my-notes.md` at once, changing only that line.

## Teardown
- None (step 7 restores the file)
