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
| 1 | `target/debug/echoctl today add plugin:widget-demo:open --size S --above core:needs.now` (the plugin has never been asked for it; S, its smallest size); open `{{base_url}}/#today` and take a snapshot at once, before 2 s pass | "Open tickets" is already in its place, above Needs you now: its label on top and a grey placeholder (pulsing lines) where its number will be, marked busy for assistive tech (`aria-busy="true"`); no "Couldn't draw this" |
| 2 | Note the top of Needs you now; wait until the widget shows its number (a few seconds, no reload) | "12 open tickets" in the same place within a few seconds, with no reload or navigation; Needs you now's top is where it was (within 2 px) |
| 3 | Reload | "12 open tickets" at once, no placeholder (its last answer is drawn) |
| 4 | Close the earlier tab. `target/debug/echoctl today remove plugin:widget-demo:open`; `target/debug/echoctl today add plugin:widget-demo:builds --size M --above core:needs.now` (a list, never asked); open `{{base_url}}/#today` in a new tab, note Needs you now's top, wait for its rows | "Last builds" holds its place with the placeholder, then its rows within a few seconds, no reload; Needs you now within 2 px of where it was (a list's height depends on its rows: the placeholder is a few rows high) |
| 5 | Reload | "Last builds" at once with its rows, no placeholder (its last answer is drawn) |
| 6 | `target/debug/echoctl today add plugin:widget-demo:board --size M --above core:needs.now`; close the earlier tab (an open Today would ask for it first), open `{{base_url}}/#today` in a new tab and snapshot at once | "Sprint board" holds its place with the same placeholder until its page is drawn; then three boxes ("5 To do", "2 Doing", "9 <i>Done</i>" as text) in that place; never "Couldn't draw this — its page isn't loaded" |
| 7 | `target/debug/echoctl plugin install hang-demo --yes` (the same test marketplace: one widget whose plugin never answers); `target/debug/echoctl today add plugin:hang-demo:hang --size M --above core:needs.now`; close the earlier tab, open `{{base_url}}/#today` in a new tab and snapshot at once, then again after 10 s (no reload) | "Never answers" first holds its place with the placeholder; after its ask fails (about 5 s) it keeps its frame and label with one line "Couldn't get its data — hang-demo didn't answer __widget:hang within 5 s" (the error as text), not busy, no placeholder; it doesn't vanish |

## Expected Final State
A widget waiting for its plugin's first answer, or for its window half, keeps its place and size with a placeholder; what is under it doesn't jump when the answer comes.

## Teardown
- `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove widget-demo`; `target/debug/echoctl plugin remove hang-demo`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The same holds for the other kinds of the test plugin (`sprint` stats, `trend` chart): each fills in a few seconds and Needs you now stays within 2 px.
- A plugin that never answers (step 7): the frame stays with why, the user's decision (2026-11-11); it goes once its plugin answers: Echo asks again after the widget's refresh (60 s at least), and Today draws the answer at its next read.
- A widget whose plugin answered "nothing to show" still takes no place (F-TODAY-19); only one never answered holds it.
