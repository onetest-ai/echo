---
id: TC-037
title: A widget two rows high (echoctl today height, or 1 / 2 in its edit header) takes its row and the next, the widgets beside it; one it can't take is refused
priority: high
type: functional
requirements: [F-TODAY-25, F-TODAY-11, F-TODAY-15, F-TODAY-10]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, layout, height, edit]
---

# TC-037: A widget two rows high takes its row and the next

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; viewport 1440 × 1000; CLI steps run as `target/debug/echoctl …` after `. <dir>/env`; no `today-layout.json` yet (the default layout)

## Test Data

| Widget | Before | After |
|---|---|---|
| My work · this week (`core:tasks.mine`) | M, 1 row, last row | S, 2 rows, first in Next meeting's row |
| Recorded this week (`core:calls.week`) | not on Today | — (refused at 2 rows) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `target/debug/echoctl widget list` | `core:tasks.mine` has `heights` `[1, 2]`; `core:calls.week` and `core:calendar.strip` `[1]` |
| 2 | `target/debug/echoctl today height core:tasks.mine 3`, then `target/debug/echoctl today height core:calls.week 2` | Exit 1: "core:tasks.mine is 1 or 2 rows high, not 3"; exit 1: "core:calls.week is 1 row high, not 2" |
| 3 | `target/debug/echoctl today height core:tasks.mine 2`, `target/debug/echoctl today size core:tasks.mine S`, `target/debug/echoctl today move core:tasks.mine --before core:calendar.next`, then `target/debug/echoctl today` | My work's entry has `"height": 2`, `size` "S", `row` true, just before Next meeting (which no longer starts a row); no other entry has `height` |
| 4 | Open `{{base_url}}/#today` | Your day holds the first column; in the other two, My work on the left as tall as what is beside it: Next meeting and Needs you now one under another, then The day's choices (the next row) — all three beside My work, none under it (`.td-pair`, My work in `.td-tall`) |
| 5 | Click Edit Today (the pencil) | The right panel opens, Today at 2 columns: My work, Next meeting, Needs you now… one under another (no room beside a widget two rows high: it is one row high). My work's frame has "Size" S M (S pressed) and "Height" ↕ 1 2 (2 pressed, label "My work · this week: 2 rows high"); Recorded this week isn't on Today; Your day · strip isn't on Today either — a widget that takes one height has no Height group (check The day's choices has one, Your day · agenda has one) |
| 6 | Click My work's "1" | `target/debug/echoctl today` shows My work without `height`; its "1" is pressed |
| 7 | Click My work's "2", then Done; close the right panel if it is open | Back to step 4's page |
| 8 | `target/debug/echoctl today reset`; reload | Today as it comes: no widget two rows high |

## Expected Final State
Today's layout is the default again.

## Teardown
- None needed (fresh data per run)

## Changes
- Changes Today's layout, then resets it.

## Notes
- Only the first widget two rows high in a row is; in the row it takes, one more is one row high (the node test `today-layout.test.mjs` covers it, and one column, nothing beside it, an empty widget).
