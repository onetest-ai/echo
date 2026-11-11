---
id: TC-021
title: Today lays itself out by its width — 3 columns wide, 2 with a task on the right — each row filling it by its widgets' sizes; changing a size or the width never reorders
priority: high
type: functional
requirements: [F-TODAY-21, F-TODAY-12]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, layout, responsive]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-021: Today lays itself out by its width and the widgets' sizes

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; no `today-layout.json` yet
- The browser window 1440 × 1000, the menu open (the default)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today` | 3 columns: Your day holds the first (`.td-side` columns `1fr 2fr`), its column as tall as what is beside it, the list itself in view as Today scrolls (sticky, the view's height at most), opened on now and scrolling inside; beside it Next meeting and Needs you now side by side, then The day's choices across both, then My work |
| 2 | Click My work's first card | Its Details open on the right; Today narrows to 2 columns: Your day keeps the first (`1fr 1fr`), still on now (or where you scrolled it), everything else in the second, one under another in the same order |
| 3 | Close the right panel (its ×) | Back to step 1's widths, nothing moved |
| 4 | `target/debug/echoctl today size core:calendar.agenda M`; reload | Your day at M no longer holds a column: it is a row of its own two columns wide (its size: the third column stays empty), then Next and Needs you now, the choices, My work; `today size core:calendar.agenda S` puts it back |
| 5 | Resize the browser window to 700 × 1000 | Today one column: Your day as the strip (the day in one bar) on top, then Next meeting, Needs you now, the choices, My work, each across, in that order |
| 6 | Back to 1440 × 1000 at 900 px high; `target/debug/echoctl today move core:tasks.mine --before core:needs.now`, `today size core:tasks.mine M`; reload | My work in Next meeting's row, beside it (Needs you now wraps to the next line). When its list is longer than its card (it depends on the data and the height), a fade at its bottom shows there is more and goes when scrolled to the end; when it fits, there is no fade |
| 7 | `target/debug/echoctl today reset`; reload | As in step 1 |

## Expected Final State
Today as it comes.

## Notes
- Columns come from Today's own width: fold under 340 px a column, unfold only at 380 (no jumping while a panel is dragged) — Workbench `TodayLayoutRules`, `TodayPanels`, `TodayResponsive`; the node test `today-layout.test.mjs` holds the table.
