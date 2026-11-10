---
id: TC-021
title: Today lays itself out by its width — 3 columns wide, 2 with a task on the right — each row filling it by its widgets' sizes; changing a size or the width never reorders
priority: high
type: functional
requirements: [F-TODAY-12]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, layout, responsive]
---

# TC-021: Today lays itself out by its width and the widgets' sizes

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; no `today-layout.json` yet
- The browser window 1440 × 1000, the menu open (the default)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today` | Needs you now across the page; under it Your day and My work side by side, Your day about twice as wide (the grid's columns `2fr 1fr`) |
| 2 | Click My work's first card | Its Details open on the right; Today narrows to 2 columns: Your day and My work still side by side, now equal (`1fr 1fr`), in the same order; Needs you now still across |
| 3 | Close the right panel (its ×) | Back to step 1's widths, nothing moved |
| 4 | `target/debug/echoctl today size core:tasks.mine M`; reload | My work 2, Your day 2 don't fit 3 columns: the widest first narrows — Your day `1fr`, My work `2fr`, side by side, Your day still first |
| 5 | Resize the browser window to 700 × 1000 | Today one column: Needs you now, then Your day, then My work, each across, in that order |
| 6 | Back to 1440 × 1000 at 900 px high; `target/debug/echoctl today move core:tasks.mine --before core:needs.now`, `today size core:tasks.mine M`; reload | My work beside Needs you now, its list cut by the card's height: a fade at its bottom shows there is more; scrolled to its end, the fade goes |
| 7 | `target/debug/echoctl today reset`; reload | As in step 1 |

## Expected Final State
Today as it comes.

## Notes
- Columns come from Today's own width: fold under 340 px a column, unfold only at 380 (no jumping while a panel is dragged) — Workbench `TodayLayoutRules`, `TodayPanels`, `TodayResponsive`; the node test `today-layout.test.mjs` holds the table.
