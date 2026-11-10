---
id: TC-020
title: echoctl today moves, takes off, puts back and resizes Today's widgets, the page draws them in that order and rows, and reset brings Today back as it comes
priority: high
type: functional
requirements: [F-TODAY-11]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, layout, cli]
---

# TC-020: echoctl today arranges Today's widgets, and the page follows

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); CLI steps run after `. <dir>/env`; `target/debug/echoctl` is the debug build of this checkout
- No `today-layout.json` in the data's config folder (a fresh build has none)

## Test Data

| Widget | Size | On the page |
|---|---|---|
| `core:calendar.agenda` | S | Your day, holding the first column |
| `core:calendar.next` | S | Next meeting (while there is one) |
| `core:needs.now` | M | Needs you now, beside Next |
| `core:calendar.choices` | M | The day's choices (while there are some) |
| `core:routines.cards` | M | a routine's card (none in the standard data: draws nothing) |
| `core:tasks.mine` | M | My work · this week |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today`; open `{{base_url}}/#today` | The six widgets of Test Data in that order, rows starting at each but Needs you now (in Next's row); the page: Your day on the left the whole height (`.td-hold`), beside it Next meeting and Needs you now side by side, then The day's choices, then My work |
| 2 | `target/debug/echoctl today move core:tasks.mine --above core:calendar.agenda`; reload the page | The command prints My work first on a row of its own; the page: My work · this week on top, then the rows (nothing holds a column now: Your day is no longer first) |
| 3 | `target/debug/echoctl today remove core:needs.now`; reload | Needs you now is gone from the page (its items stay: `echoctl widget show core:needs.now` still lists them) |
| 4 | `target/debug/echoctl today add core:needs.now --after core:tasks.mine`; reload | Needs you now beside My work, in one row |
| 5 | `target/debug/echoctl today size core:tasks.mine L` | Refused: "core:tasks.mine takes S, M, not L", exit code 1; `echoctl today` unchanged |
| 6 | `target/debug/echoctl today add core:nope` | Refused: no widget "core:nope" (`echoctl widget list` names them), exit code 1 |
| 7 | `target/debug/echoctl today reset`; reload | As in step 1 |

## Expected Final State
Today as it comes; `today-layout.json` holds the default.

## Notes
- Phase 2 of the Today widgets (#323). Sizes are kept but don't change the page's widths yet: that comes with the layout engine.
