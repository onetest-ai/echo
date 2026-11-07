---
id: TC-015
title: Profile → Since, Notes for Claude and Look are edited in place and saved at once
priority: medium
type: functional
requirements: [F-THR-15]
module: threads
run: agent
platform: any
tags: [threads, profile]
---

# TC-015: Profile → Since, Notes for Claude and Look are edited in place and saved at once

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |
| Since | 2026-11-01 |
| Notes | Call the pilot "GP". |
| Colour | the fourth swatch, `#f0a24a` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/profile` | Since reads "From its first call"; Notes for Claude reads "What the minutes and Claude should know — what people call things, what to leave out"; the AI row reads "The default · Claude Code (as configured)"; no History row (Recall is off) |
| 2 | Click "From its first call", set the date `2026-11-01` in the input named "Since" | Since reads "1 Nov 2026" (locale format with the day, month and year); `echoctl project show globex-pilot` has `"since": "2026-11-01"` |
| 3 | Click the Notes text, type `Call the pilot "GP".` and leave the field | The Notes row shows `Call the pilot "GP".`; `echoctl project show globex-pilot` has `"notes"` with that text |
| 4 | Click the fourth colour swatch (`[data-pj="color"][data-color="#f0a24a"]`). Wait up to 3 s after an action for the window to redraw before reading. | That swatch is selected (class `on`); `echoctl project show globex-pilot` has `"color": "#f0a24a"` |
| 5 | Read the AI row | A select "AI profile" with the options "The default (Claude Code (as configured))" and "Claude Code (as configured)"; a button "model: the profile's minutes model" |

## Expected Final State
Globex pilot has the since date, the note and the colour stored.

## Teardown
- `echoctl project set globex-pilot --since "" --notes "" --color "#5aa9ff"`

## Changes
- Changes since, notes and colour of `globex-pilot`.
