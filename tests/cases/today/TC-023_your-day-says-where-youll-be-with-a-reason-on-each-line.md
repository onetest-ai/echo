---
id: TC-023
title: Your day says where you'll be with a reason on each line — overlaps folded into the one you go to, the rest a count to show; your work in a free gap; ⋯ or right-click opens Calendar's menu, a click the meeting's card on the right
priority: high
type: functional
requirements: [F-TODAY-02]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, your-day, agenda]
---

# TC-023: Your day says where you'll be, a reason on each line

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (a busy day: many meetings at once, some not answered, some maybes); the window served at `{{base_url}}`, `{{base_url}}/` loaded first
- The browser window 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today`; read Your day's list "Where you'll be" | A line a place in time order, each: its time, a bar (solid green: going; dashed grey: no reply; dashed violet: maybe), its subject and threads, a reason under it; the first you go to that isn't over says "next" and its time is violet; the line "now" between past and coming; past lines dimmed |
| 2 | Find a line whose reason says "N others at once" | Its time amber: you accepted two or more meetings at that time (a choice still to make); the others aren't lines of their own |
| 3 | Find a line whose reason starts "instead of" (if any on this day) | It names one meeting at the same time you haven't answered or marked maybe, and "· N skipped" |
| 4 | Click "Show N skipped" at the end | The folded meetings appear, dimmed, each saying "skipped"; "Hide the skipped" folds them again |
| 5 | Find a line "maybe" and one "no reply" | Their bars dashed (violet, grey); neither is ever the next |
| 6 | On the small set (`serve.sh up <dir2> <port2>`): read Your day | A line "Send Acme the SSO checklist" with "your work · 1 h" (or "· N min"), its bar dashed green, in the first free gap of 45 minutes or more after now; a click opens the task's Details on the right |
| 7 | Over a coming meeting's line, click ⋯ | Calendar's menu for it (Record when it starts or Don't record, Prepare / Show prep, Add to a thread…, Copy the Teams link, Show details); Escape or a click elsewhere closes it; a right-click on the line opens the same |
| 8 | Click the line itself (not a link on it) | The meeting's card on the right ("Meeting"): its time, chips, its one action strip, People — as in Calendar; × closes it |
| 9 | A past meeting with minutes not opened: read its reason | "new minutes · N decisions · N actions" in violet; a click opens its Minutes |

## Notes
- The reason line's links (minutes, prep, import the Teams transcript, try again) act as the old buttons did; the rest is in the menu (`TC-002`, `calendar/TC-001`).
- The rules are the node test `today-agenda.test.mjs`'s table (`agendaRows`).
