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

| Widget | On the page |
|---|---|
| `core:needs.now` | Needs you now |
| `core:routines.cards` | a routine's card (none in the standard data: draws nothing) |
| `core:calendar.agenda` | Your day |
| `core:tasks.mine` | My work · this week |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today`; open `{{base_url}}/#today` | The four widgets of Test Data in that order, rows starting at Needs you now, the cards and Your day (`row: true`), My work in Your day's row; the page: Needs you now on top, then Your day and My work side by side — Today as it was before widgets |
| 2 | `target/debug/echoctl today move core:tasks.mine --above core:needs.now`; reload the page | The command prints My work first on a row of its own; the page: My work · this week on top, full width, then Needs you now, then Your day |
| 3 | `target/debug/echoctl today remove core:needs.now`; reload | Needs you now is gone from the page (its items stay: `echoctl widget show core:needs.now` still lists them) |
| 4 | `target/debug/echoctl today add core:needs.now --after core:tasks.mine`; reload | Needs you now beside My work, in one row |
| 5 | `target/debug/echoctl today size core:tasks.mine L` | Refused: "core:tasks.mine takes S, M, not L", exit code 1; `echoctl today` unchanged |
| 6 | `target/debug/echoctl today add core:calendar.strip` | Refused: "core:calendar.strip isn't on Today's page yet …", exit code 1 |
| 7 | `target/debug/echoctl today reset`; reload | As in step 1 |

## Expected Final State
Today as it comes; `today-layout.json` holds the default.

## Notes
- Phase 2 of the Today widgets (#323). Sizes are kept but don't change the page's widths yet: that comes with the layout engine.
