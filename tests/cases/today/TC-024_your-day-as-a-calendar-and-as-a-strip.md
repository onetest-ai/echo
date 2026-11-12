---
id: TC-024
title: Your day as a calendar (the hours down, meetings side by side in lanes, opens on now) and as a strip (the day in one bar), put on Today with echoctl today; a block acts as Your day's line — click its card as a popover beside it, right-click its menu
priority: medium
type: functional
requirements: [F-TODAY-14]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, your-day, calendar, strip]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-024: Your day as a calendar and as a strip

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today remove core:calendar.agenda`, `today add core:calendar.calendar --size M --before core:tasks.mine`, `today add core:calendar.strip --size L --above core:needs.now`; open `{{base_url}}/#today` | Next meeting on top (now the first row), then a section "Your day, the strip": "Your day N meetings", one bar with a block a meeting you go to or maybe (dashed), two at once each half its height, the line now, hour ticks every 2 hours (9 to 18, wider for an early or late meeting); under Needs you now the calendar beside My work |
| 2 | Read the calendar | "Your day": the hours down (09 … 18 at least), a block a meeting at its time and length with its subject and time, overlapping ones side by side in lanes; not answered: dashed grey, maybe: dashed violet; past dimmed; the line now; at a first draw it is scrolled to a little before now when it doesn't all fit |
| 3 | Click a block in the calendar; × on the card; then click a block in the strip | Each time Calendar's card for that meeting as a popover beside the block clicked — to its right inside Today (or its left when there's no room; with room on neither side, over the right part of it, as in Calendar), its top level with the block (as far as Today's area lets it) — as from Your day's line; the right panel doesn't open; × closes it |
| 4 | Right-click a block (calendar or strip) | Calendar's menu for that meeting |
| 5 | `target/debug/echoctl today reset`; reload | Today as it comes: Your day's list holding the first column, no calendar or strip |

## Notes
- The rules are the node test `today-calendar.test.mjs` (`dayLanes`, `dayWindow`). Stepping down by height (calendar → agenda on now) comes with the fill heights' rules.
