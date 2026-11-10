---
id: TC-028
title: Recorded this week — since Monday the calls recorded (memos counted), the time recorded and how many have minutes; at M a bar a day, Monday to Sunday, today marked; put on Today with echoctl today; a click opens Calls; echoctl widget show gives the same data
priority: medium
type: functional
requirements: [F-TODAY-18]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, calls]
---

# TC-028: Recorded this week

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl widget show core:calls.week` | JSON: `since` (this week's Monday, local date), `today`, `calls`, `memos`, `recorded_min`, `with_minutes`, `days` (7, Monday to Sunday, each `date` and `calls`); no `empty` (it isn't temporal) |
| 2 | List `<dir>/home/calls/`; for each folder whose name starts with a date from `since` to `today`, read its `meta.json` (`started_at`, `ended_at`, `kind`) and whether it holds `notes.md` | `calls` = those folders (memos — `kind: memo` — included), `memos` = the memos among them, `with_minutes` = those with `notes.md`, `recorded_min` = the sum of `ended_at − started_at` (one with no `ended_at` adds nothing) in minutes, rounded; each day's `calls` = that day's folders, the days after today 0 |
| 3 | `target/debug/echoctl today add core:calls.week --size S --above core:needs.now`; open `{{base_url}}/#today` | On top a region named "Recorded this week": the heading with a "Calls" button, three numbers — the calls ("calls", and "· N memo(s)" when there are memos), the time ("minutes recorded" under an hour, else hours to one decimal, "hours recorded"), "with minutes" — as in step 1; no bars at S |
| 4 | `target/debug/echoctl today size core:calls.week M`; reload | The same numbers, and under them a bar a day, Mon to Sun, each as tall as its calls (the tallest full height, an empty day a flat line), today's marked (its day name bold), each bar named "<Weekday>: N call(s)" (`aria-label`); nothing overflows its frame |
| 5 | Click the widget (its numbers) | Calls opens (`#calls` or `#call/<folder>/…`) |
| 6 | Back on Today, click the heading's "Calls" | Calls opens |
| 7 | At 700 px wide, and in the dark theme (Settings → General → Theme), look at the widget at M | The numbers and the 7 bars inside the frame, readable; no text cut |
| 8 | `target/debug/echoctl today reset`; reload | The widget is not on Today (not in the default) |

## Notes
- The counts are `calls_week::view` (Rust tests: Monday start in local time, Sunday night vs Monday morning, hours summed from the calls' times, minutes only for calls with `notes.md`, memos counted, an empty week all zeros). A call's day is its folder's stamp (the local wall clock it started at), as Your day reads it.
- Standard data records most of its calls today (relative to the build): before Wednesday most days' bars are 0.
