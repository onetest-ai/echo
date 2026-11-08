---
id: TC-019
title: Choosing Record for a series is kept after a reload and is what echoctl series lists
priority: medium
type: functional
requirements: [F-CAL-13]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, series]
---

# TC-019: Choosing Record for a series is kept after a reload and is what echoctl series lists

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Field | Value |
|---|---|
| Meeting | Globex weekly (`ev-globex-weekly`), series `id:S-GLOBEX-WEEKLY`, rule ask |
| Rule written | record `always` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-globex-weekly"]` (Globex weekly, to come 37 minutes after the build) | "Don't record" is pressed in "Every meeting of this series" |
| 2 | Click "Record" | "Record" is pressed (`aria-pressed="true"`), "Don't record" is not |
| 3 | Run `echoctl series list` | `id:S-GLOBEX-WEEKLY` has `"record": "always"` |
| 4 | Reload `{{base_url}}/#calendar/day`, click the same block again | "Record" is still pressed |

## Expected Final State
The series is set to Record.

## Teardown
- Run `echoctl series set 'id:S-GLOBEX-WEEKLY' '{"record":"ask"}'` (back to the data's rule), reload the page

## Notes
- Writes only the rule file (`projects/series.json`). Arming each occurrence belongs to the app, not to `window --serve`: the block does not get "Will record" here.
