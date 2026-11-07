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
- My notes is open in the right panel (`[aria-label="My notes"]` pressed); the note box is `#note-input` ("New note", placeholder "Add a note — Enter to add")
- The data's `claude` is a fake that refuses every run: Write minutes fails at once and reaches no account

## Test Data

| Call | Folder | Before |
|---|---|---|
| Umbrella discovery | `2026-10-22_140000-umbrella-discovery` | no `my-notes.md` |
| Duff discovery | `2026-10-27_100000-duff-discovery` | no `my-notes.md` |
| Coffee chat | `2026-10-30_120000-coffee-chat` | no `my-notes.md`; "No minutes written automatically" with "Write minutes" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[data-call="2026-10-22_140000-umbrella-discovery"]`; in My notes type `Ask Zoe for the budget range` into the note box, do not press Enter | The panel's state reads "Not saved yet"; the list says "Add notes about this call. Select words in the transcript to comment on what was said." |
| 2 | Click the row `[data-call="2026-10-27_100000-duff-discovery"]`; wait 2 s | Duff discovery is open; its My notes is empty and its note box is empty |
| 3 | Run `cat "$ECHOCTL_HOME/calls/2026-10-22_140000-umbrella-discovery/my-notes.md"` | `- Ask Zoe for the budget range` |
| 4 | Click the row Umbrella discovery again | Its My notes lists "• Ask Zoe for the budget range"; the state "Saved" |
| 5 | Click the row `[data-call="2026-10-30_120000-coffee-chat"]`, tab "Minutes"; type `Coffee again next month` into the note box, do not press Enter; click "Write minutes" | The note "• Coffee again next month" is listed in My notes and the box is empty; within 15 s the Minutes tab reads "Couldn't write the minutes" and "test data: Claude Code is a fake here" |
| 6 | Run `cat "$ECHOCTL_HOME/calls/2026-10-30_120000-coffee-chat/my-notes.md"` | `- Coffee again next month` |

## Expected Final State
Both typed notes are in their own calls' `my-notes.md`; nothing typed was lost.

## Notes
- The third trigger of `F-NOTES-07`, the window closing (or Echo quitting), is the native window's `prepareClose`: not reachable in the served window. Person-run.
- Coffee chat's minutes run ends failed (the fake): later stages on the same data see that.
