---
id: TC-023
title: Your day says where you'll be with a reason on each line — overlaps folded into the one you go to, the rest a count to show; your work in a free gap; ⋯ or right-click opens Calendar's menu, a click the meeting's card as a popover beside the line
priority: high
type: functional
requirements: [F-TODAY-02]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, your-day, agenda]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-023: Your day says where you'll be, a reason on each line

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (a busy day: many meetings at once, some not answered, some maybes); the window served at `{{base_url}}`, `{{base_url}}/` loaded first
- The browser window 1440 × 1000
- Step 6 needs a free 45 minutes before 18:00 (where the working day ends for Your day): run it before about 17:00

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today`; read Your day's list "Where you'll be" | A line a place in time order, each: its time, a bar (solid green: going; dashed grey: no reply; dashed violet: maybe), its subject and threads, a reason under it; the line holding the first meeting you go to that isn't over says "next" ("now" when it is under way) and its time is violet (the same meeting Next meeting shows, folded into that line or heading it — a line whose first meeting is over shows the one still on); the line "now" between past and coming; past lines dimmed |
| 2 | Find a coming line whose reason says "N others at once" and that isn't the next (on standard data a later one, e.g. "Initech architecture board") | Its time amber: you accepted two or more meetings at that time (a choice still to make; once past, or when it is the next — violet wins — it isn't amber); the others aren't lines of their own |
| 3 | Find a line whose reason starts "instead of" — standard data may have none (the rule is the node test `today-agenda.test.mjs`): then this step is skipped | It names one meeting at the same time you haven't answered or marked maybe, and "· N skipped" |
| 4 | Click "Show N skipped" at the end | The folded meetings appear in time order, dimmed, each saying "skipped"; "Hide the skipped" folds them again |
| 5 | Find a meeting you marked maybe and one you haven't answered — on the busy standard day most are folded: open "Show N skipped" | Their bars dashed (violet, grey) — a line of its own says "maybe" / "no reply", a folded one "skipped"; neither is ever the next |
| 6 | On the small set (`serve.sh up <dir2> <port2>`): read Your day | A line "Send Acme the SSO checklist" with "your work · 1 h" (or "· N min"), its bar dashed green, in the first free gap of 45 minutes or more after now; a click opens the task's Details on the right |
| 7 | Over a coming meeting's line, click ⋯ | Calendar's menu for it (Record when it starts or Don't record, Prepare / Show prep, Add to a thread…, Copy the Teams link, Show details); Escape or a click elsewhere closes it; a right-click on the line opens the same |
| 8 | Click the line itself (not a link on it) | Calendar's card for the meeting (the one Calendar shows when its block is clicked) as a popover beside the line: to the line's right inside Today (or its left when there's no room; with room on neither side, over the right part of it, as in Calendar), its top level with the line; its title, "Today · HH:MM–HH:MM", chips, its one action strip, People. The right panel doesn't open (no "Meeting" tab). A click on the same line again closes it; open it again and × closes it |
| 9 | Over the same line click ⋯, then Show details | The same popover beside that line; × closes it |
| 10 | A past meeting with minutes not opened: read its reason | "new minutes · N decisions · N actions" in violet; a click opens its Minutes |

## Notes
- The reason line's links (minutes, prep, import the Teams transcript, try again) act as the old buttons did; the rest is in the menu (`TC-002`, `calendar/TC-001`).
- The rules are the node test `today-agenda.test.mjs`'s table (`agendaRows`).
