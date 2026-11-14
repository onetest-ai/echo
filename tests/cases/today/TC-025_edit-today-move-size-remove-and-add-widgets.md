---
id: TC-025
title: Edit Today — each widget framed (name, sizes, ×), dragged onto another's top / bottom edge for a new row or its left / right half beside it, dropped at the end for a last row; Add a widget on the right (search, parts, the preview above a list that scrolls under it — the pointed row's, else the picked one's — at a size, Add or drag in); Reset and Done
priority: high
type: functional
requirements: [F-TODAY-15]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, edit-mode, drag-and-drop]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-025: Edit Today — move, size, remove and add widgets

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000; Today as it comes (`target/debug/echoctl today reset`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today`; click the pencil "Edit Today" in the head | The head shows "Reset to the default" and "Done"; each widget is framed: grip, its name, ← ↑ ↓ → for the moves it has (before / after in its row, a row above / below), the sizes it takes (the current one on), ×; the head also has "Add a widget"; its content shown but not clickable; Routine cards (nothing today) shows what it is, "shows only while it has something"; on the right "Add a widget": search, parts (All, Calendar, Tasks, Calls, Needs you, Routines), a row a widget with "on Today" or Add; above the list, under the parts, the picked widget's preview — its name and part; a small Today (3 columns × 2 rows, dashed, the same size for every widget) with the widget drawn, its real content, in the cells its size takes; then one line: its sizes S M L and what the size means, e.g. "M · 2 columns · fills its column"; no other buttons. Pick "Threads at a glance" (not on Today): its preview shows its bars in two of the three columns, the card as tall as before |
| 1a | Scroll the panel's list to its end; point at "Recorded this week" (don't click); then move the pointer onto Today's heading | Pointing: the preview above the list is Recorded this week's ("Recorded this week · Calls", its sizes with M on) while the list stays scrolled where it was (the preview always in sight); off the list: the preview is the picked one's again ("Your day · agenda", the highlighted row) |
| 1b | Click the preview's size M (Your day · agenda is on Today at S by default: it becomes M there at once, Today redrawn), then click S again; press Tab from S until the focus reaches the first row's "Add" in the list | Your day · agenda is M on Today, then S again (`echoctl today`: `core:calendar.agenda` size S); the focus moves button by button into the list and stays on that "Add" (never back to the page's top) |
| 1c | Move the pointer off the list. Make sure the preview's size that is on gets the focus on its press, as WebView2 on Windows gives it (Chromium does too; WebKit doesn't): in the page, `const b = document.querySelector('.tw-pick [data-tw-pick-size].on'); b.addEventListener('mousedown', () => b.focus(), { once: true })`. Click that size with the mouse; then point at the list's last row, then at the one before it. Move the pointer off the list again, run the same line, click that size again and press Tab until the focus reaches the first row's "Add" in the list | The click changes nothing (the same size on); pointing, the preview follows the row under the pointer: the last row's, then the one before's (#527: before the fix it stayed the picked one's). After the second click, Tab goes on from that size through the next sizes into the list, never from the window's top |
| 1d | Move the pointer off the list; Tab to the preview's size that is on (or Shift+Tab back to it) and press Space; then point at the list's last row | The focus stays on that size, and the preview stays the picked one's while the pointer is over the list (the keyboard on the preview's sizes holds it) |
| 2 | On My work click S, then M | My work is S, then M again (M is its default); `echoctl today` says `core:tasks.mine` size M |
| 3 | Drag My work by its frame onto the top edge of Needs you now | A line shows above Needs while over it; dropped, My work is on a row of its own above Needs (`echoctl today`: mine first, a row of its own) |
| 4 | In the panel pick "Threads at a glance" (not on Today; Next meeting is, by default); drag its row onto the right half of My work | A line on My work's right edge while over; dropped, Threads at a glance sits right after My work in its row |
| 5 | Click × on Threads at a glance | It is off Today; in the panel it has Add again |
| 6 | Pick "Your day · strip" (S is greyed: it takes M and L); click L under the preview, then the strip's "Add" in its row | The strip is added on a new last row at L (the size picked under the preview) |
| 7 | Type "calen" in the search, a letter at a time | The search keeps the keyboard (also when Today is refreshed from the app meanwhile, #391); Your day's widgets and Next meeting listed (label, about or part matches), the part back on All |
| 8 | Click Done | No frames, the panel closes; Today keeps the changes after a reload |
| 9 | Edit Today → "Reset to the default" | Today as it comes, still in edit mode with Add a widget open (so the page is narrower: Next and Needs you now one under another beside Your day): Your day holding the first column; beside it Next meeting, Needs you now, the choices, My work |
| 10 | Edit Today again; Tab to My work's "↑" ("Move My work · this week to a row above") and press Enter | My work moves to a row of its own above the row before it; the focus stays on its move button; each frame ends inside its row (no frame under the next row) |
| 11 | Tab to a widget's × and press Enter | It is off Today; the focus is on the next widget's × (not the page) |
| 12 | Resize to 700 × 1000 (the panel may close as the window narrows; if it is open, close it with its ×); click "Add a widget" in the head | The panel opens again (over the page at this width); resize to 1440 × 600: the preview (its buttons too) and the list both fit — the list scrolls, the panel itself does not; Done, then `target/debug/echoctl today reset` |

## Notes
- Where a drop goes is the node test `today-edit.test.mjs` (`dropSide`); the layout's rules are `today_layout::place` (Rust tests), the same as `echoctl today move/add`.
