---
id: TC-025
title: Edit Today — each widget framed (name, sizes, ×), dragged onto another's top / bottom edge for a new row or its left / right half beside it, dropped at the end for a last row; Add a widget on the right (search, parts, preview at a size, Add or drag in); Reset and Done
priority: high
type: functional
requirements: [F-TODAY-15]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, edit-mode, drag-and-drop]
---

# TC-025: Edit Today — move, size, remove and add widgets

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000; Today as it comes (`target/debug/echoctl today reset`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today`; click the pencil "Edit Today" in the head | The head shows "Reset to the default" and "Done"; each widget is framed: grip, its name, the sizes it takes (the current one on), ×; its content shown but not clickable; Routine cards (nothing today) shows what it is, "shows only while it has something"; on the right "Add a widget": search, parts (All, Calendar, Tasks, Needs you, Routines), a row a widget with "on Today" or Add; under the list the picked widget's preview |
| 2 | On My work click M | My work is M; `echoctl today` says `core:tasks.mine` size M |
| 3 | Drag My work by its frame onto the top edge of Needs you now | A line shows above Needs while over it; dropped, My work is on a row of its own above Needs (`echoctl today`: mine first, a row of its own) |
| 4 | In the panel pick "Next meeting"; drag its row onto the right half of My work | A line on My work's right edge while over; dropped, Next meeting sits right after My work in its row |
| 5 | Click × on Next meeting | It is off Today; in the panel it has Add again |
| 6 | Pick "Your day · strip" (S is greyed: it takes M and L); click "Add at M" | The strip is added on a new last row at M |
| 7 | Type "calen" in the search, a letter at a time | The search keeps the keyboard; Your day's widgets and Next meeting listed (label, about or part matches), the part back on All |
| 8 | Click Done | No frames, the panel closes; Today keeps the changes after a reload |
| 9 | Edit Today → "Reset to the default" | Today as it comes: Needs, Routine cards, Your day beside My work |

## Notes
- Where a drop goes is the node test `today-edit.test.mjs` (`dropSide`); the layout's rules are `today_layout::place` (Rust tests), the same as `echoctl today move/add`.
