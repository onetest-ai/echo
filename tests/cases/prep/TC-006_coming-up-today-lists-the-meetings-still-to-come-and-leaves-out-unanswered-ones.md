---
id: TC-006
title: Coming up today counts live and coming meetings the user answered yes to, and leaves out tentative, unanswered and declined ones
priority: high
type: functional
requirements: [F-PREP-01]
module: prep
run: agent
platform: any
data: standard
tags: [prep, calls, response]
---

# TC-006: Coming up today counts live and coming meetings the user answered yes to, and leaves out tentative, unanswered and declined ones

**Module:** Prep | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A prep's id is the one in `<dir>/manifest.json` → `preps` (stable for a given meeting id); Coming up today rows are `[data-up="<prep id>"]`

## Test Data

| In the list (11) | Left out |
|---|---|
| Globex office hours · Vendor demo (live, accepted/none) · Initech architecture board (live organizer, and to come accepted) · Vendor demo (live, none) · Initech team lunch (accepted) · Globex sponsor update (none) · Acme help desk sync (organizer) · Acme renewal follow-up · Globex weekly (the series one) · Steering prep | Acme budget check (tentative) · Partner planning (not answered) · Initech town hall (declined) · Acme training dry run (not answered) · the second Initech team lunch (tentative) · the second Globex weekly (declined) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" | The first row of the list is "Coming up today" (`.up-fold`) with the number 11 (`.up-n`) |
| 2 | Read that row's right side | "Now" and "Globex office hours" (the first meeting already started and not over) |
| 3 | Click the row "Coming up today" | `aria-expanded` is true; 11 rows with `data-up` appear above the calls |
| 4 | Count the rows by title | "Vendor demo" 2, "Initech architecture board" 2, "Globex office hours" 1, "Initech team lunch" 1, "Globex sponsor update" 1, "Acme help desk sync" 1, "Acme renewal follow-up" 1, "Globex weekly" 1, "Steering prep" 1 (total 11) |
| 5 | Look for "Acme budget check", "Partner planning", "Initech town hall" and "Acme training dry run" | None of them is listed |
| 6 | Read the row "Initech architecture board" that starts latest (the one to come) | Its meta spans hold the time, "25 min" and "Alex Chen, Priya Nair +1"; no thread icon |
| 7 | Read the row "Globex office hours" | Its meta holds the time, "30 min", "Kim Park", and a thread icon (Globex pilot) |
| 8 | Read the `title` of every row | "Prep ready" for "Globex sponsor update" only; "No prep yet" for the other ten |

## Expected Final State
Coming up today is unfolded with 11 rows.

## Teardown
- Click "Coming up today" to fold it again

## Notes
- Creates or changes nothing. Applies the response rule of the agenda (accepted, organizer, none): tentative, not answered and declined meetings never show here, although the calendar draws them (`calendar/TC-015`).
- Run within 15 minutes of the build: the "live" meetings leave the list when they end (20 minutes after it).
