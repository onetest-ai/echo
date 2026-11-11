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
| 2 | Note the top of Needs you now; wait until the widget shows its number (a few seconds, no reload) | "12 open tickets" in the same place within a few seconds, with no reload or navigation; Needs you now's top is where it was (within 2 px) |
| 2a | `target/debug/echoctl today remove plugin:widget-demo:open`; `target/debug/echoctl today add plugin:widget-demo:builds --size M --above core:needs.now` (a list, never asked); open `{{base_url}}/#today` in a new tab, note Needs you now's top, wait for its rows | "Last builds" holds its place with the placeholder, then its rows within a few seconds, no reload; Needs you now within 2 px of where it was (a list's height depends on its rows: the placeholder is a few rows high) |
| 3 | Reload | "12 open tickets" at once, no placeholder (its last answer is drawn) |
| 4 | `target/debug/echoctl today add plugin:widget-demo:board --size M --above core:needs.now`; open `{{base_url}}/#today` in a new tab and snapshot at once | "Sprint board" holds its place with the same placeholder until its page is drawn; then three boxes ("5 To do", "2 Doing", "9 <i>Done</i>" as text) in that place; never "Couldn't draw this — its page isn't loaded" |

## Expected Final State
A widget waiting for its plugin's first answer, or for its window half, keeps its place and size with a placeholder; what is under it doesn't jump when the answer comes.

## Teardown
- `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove widget-demo`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The same holds for the other kinds of the test plugin (`sprint` stats, `trend` chart): each fills in a few seconds and Needs you now stays within 2 px.
- A plugin that never answers: its ask fails after a few seconds and the placeholder goes (no place, as for nothing to show).
- A widget whose plugin answered "nothing to show" still takes no place (F-TODAY-19); only one never answered holds it.
