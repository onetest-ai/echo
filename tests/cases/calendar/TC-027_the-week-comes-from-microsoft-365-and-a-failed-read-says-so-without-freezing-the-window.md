---
id: TC-027
title: Signed in, the Week view shows the week from Microsoft 365; a read that fails says "Can't read the calendar" and the window keeps answering
priority: high
type: functional
requirements: [F-M365-02]
module: calendar
run: person
platform: any
data: standard
tags: [calendar, m365, week]
---

# TC-027: Signed in, the Week view shows the week from Microsoft 365; a read that fails says "Can't read the calendar" and the window keeps answering

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- The person's own Microsoft 365 **test** account, with at least one meeting this week and one meeting next week in its calendar (made by the person; invite nobody)
- Echo's window served on the standard data with that account signed in to the run's own config (never `~/echo`): `tests/fixtures/serve.sh up <dir> <port> --set standard`, then in the window Settings → Calendar & chat → connect Microsoft 365 (as `TC-002`); `<dir>/config/graph.json` exists. Load `{{base_url}}/` first (the cookie)
- A way to switch the computer's network off and on (Wi-Fi switch)
- Why it is a person's case: the window reads the week from Microsoft 365 only with a real sign-in; without one it shows today's agenda only (`calendar/TC-011`)

## Test Data

| Field | Value |
|---|---|
| This week's meeting | the test account's own, e.g. "Echo QA this week" |
| Next week's meeting | the test account's own, e.g. "Echo QA next week" |
| Failure note | "Can't read the calendar: " followed by the reason (its words depend on the network error: match the start) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar` (a full page load) | "Week" is pressed: the rolling week of F-CAL-01 — yesterday, today and 5 days ahead — the heading naming that range (as `calendar/TC-011` reads it); no note above the grid reading "Showing today's meetings only" (signed in) |
| 2 | Look at the grid once it has drawn (a short row of grey bars may show first) | A block named "Echo QA this week" on its day at its time, from the account's calendar (the data's own agenda is not what this view draws once signed in) |
| 3 | While the grid is still drawing, or right after pressing "Later" (`[data-calnav="1"]`), click Calls in the activity bar | The Calls view opens at once (the heading of Calls shows), not after the calendar has loaded |
| 4 | Navigate to `{{base_url}}/#calendar`, press "Later" | The heading moves on to the following week; a block named "Echo QA next week" shows on its day |
| 5 | Switch the computer's network off. Press "Later" again (the following week, not opened a moment ago) | A note above the day headings in red: "Can't read the calendar: …" (`.cal-note.error`); the grid shows no "Echo QA next week" block |
| 6 | With the network still off, click "Today" in the activity bar, then "Calendar" again | Today opens at once and Calendar opens again: the window answers while the read fails |
| 7 | Switch the network on. In Calendar press "Today" (`[data-calnav="0"]`) | The note is gone; the current week's blocks, "Echo QA this week" among them, are back |

## Expected Final State
The Calendar shows the current week from the test account, with no note.

## Teardown
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` (the sign-in lived in `<dir>/config/graph.json` only); the network is on again
- Delete the two test meetings from the test account's calendar

## Notes
- Nothing is sent to anyone: the case only reads the person's own calendar; no invites.
- A full outage that makes Graph answer slowly instead of failing shows the grey bars longer: the activity bar must answer all the same (step 3, 6).
- Step 5 asks for a week not opened a moment before, so the answer has to be read from Microsoft 365 and not from what the window already holds.
