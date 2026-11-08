---
id: TC-018
title: A note typed in My notes but not added is added to its own call when another call opens or Write minutes is pressed
priority: critical
type: functional
requirements: [F-NOTES-07]
module: window
run: agent
platform: any
data: standard
tags: [notes, data-loss]
---

# TC-018: A note typed in My notes but not added is added to its own call when another call opens or Write minutes is pressed

**Module:** Window | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); file reads run after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` and its folder `$ECHOCTL_HOME/calls/*-<slug>` (the folder name ends in the slug; its date follows the data's build date)
- My notes is open in the right panel (`[aria-label="My notes"]` pressed); the note box is `#note-input` ("New note", placeholder "Add a note — Enter to add")
- The data's `claude` is a fake that refuses every run: Write minutes fails at once and reaches no account
- **In S-29** My notes has been open since `window/TC-016` step 1, and the Calls list is already on screen, left by the stage before, so step 1's navigation is skipped and step 1 starts at its click. On its own: open a call, press `[aria-label="My notes"]`, then run step 1

## Test Data

| Call | Slug | Before |
|---|---|---|
| Umbrella discovery | `umbrella-discovery` | no `my-notes.md` |
| Duff discovery | `duff-discovery` | no `my-notes.md` |
| Coffee chat | `coffee-chat` | no `my-notes.md`; "No minutes written automatically" with "Write minutes" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="-umbrella-discovery"]`; in My notes type `Ask Zoe for the budget range` into the note box, do not press Enter | The panel's state reads "Not saved yet"; the list says "Add notes about this call. Select words in the transcript to comment on what was said." |
| 2 | Click the row `[data-call$="-duff-discovery"]`; wait 2 s | Duff discovery is open; its My notes is empty and its note box is empty |
| 3 | Run `cat "$ECHOCTL_HOME"/calls/*-umbrella-discovery/my-notes.md` | `- Ask Zoe for the budget range` |
| 4 | Click the row Umbrella discovery again | Its My notes lists "• Ask Zoe for the budget range"; the state "Saved" |
| 5 | Click the row `[data-call$="-coffee-chat"]`, tab "Minutes"; type `Coffee again next month` into the note box, do not press Enter; click "Write minutes" | The note "• Coffee again next month" is listed in My notes and the box is empty; within 15 s the Minutes tab reads "Couldn't write the minutes" and "test data: Claude Code is a fake here" |
| 6 | Run `cat "$ECHOCTL_HOME"/calls/*-coffee-chat/my-notes.md` | `- Coffee again next month` |

## Expected Final State
Both typed notes are in their own calls' `my-notes.md`; nothing typed was lost.

## Notes
- The third trigger of `F-NOTES-07`, the window closing (or Echo quitting), is the native window's `prepareClose`: not reachable in the served window. Person-run.
- Coffee chat's minutes run ends failed (the fake): later stages on the same data see that.
