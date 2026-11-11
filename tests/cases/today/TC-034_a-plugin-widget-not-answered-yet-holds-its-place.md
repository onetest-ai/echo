---
id: TC-034
title: A plugin's widget its plugin hasn't answered yet holds its place on Today with a placeholder; the answer fills that place and nothing else moves; a page widget waits the same way for its window half
priority: medium
type: functional
requirements: [F-TODAY-19, F-TODAY-24]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, layout]
---

# TC-034: A plugin's widget not answered yet holds its place

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 900
- The test plugin installed as in today/TC-029 step 1 (`widget-demo` 0.4.0)

**Changes:** Today's layout (reset in Teardown)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add plugin:widget-demo:open --size M --above core:needs.now` (the plugin has never been asked for it); open `{{base_url}}/#today` and take a snapshot at once, before 2 s pass | "Open tickets" is already in its place, above Needs you now: its label on top and a grey placeholder (pulsing lines) where its number will be, marked busy for assistive tech (`aria-busy="true"`); no "Couldn't draw this" |
| 2 | Note the top of Needs you now; wait until the widget shows its number (a few seconds, no reload) | "12 open tickets" in the same place; Needs you now is still right under it, the same widgets in the same order: it moves at most by the difference between the placeholder's height and the number's (a few px, never by a whole widget) |
| 3 | Reload | "12 open tickets" at once, no placeholder (its last answer is drawn) |
| 4 | `target/debug/echoctl today add plugin:widget-demo:board --size M --above core:needs.now`; open `{{base_url}}/#today` in a new tab and snapshot at once | "Sprint board" holds its place with the same placeholder until its page is drawn; then three boxes ("5 To do", "2 Doing", "9 <i>Done</i>" as text) in that place; never "Couldn't draw this — its page isn't loaded" |

## Expected Final State
A widget waiting for its plugin's first answer, or for its window half, keeps its place and size with a placeholder; what is under it doesn't jump when the answer comes.

## Teardown
- `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove widget-demo`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A chart's height is known before its data: for `plugin:widget-demo:trend` added the same way, Needs you now stays within 2 px when the chart comes.
- A widget whose plugin answered "nothing to show" still takes no place (F-TODAY-19); only one never answered holds it.
