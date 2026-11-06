---
id: TC-018
title: A recurring meeting's card shows what Echo does with every meeting of its series: Record or Don't record, and the language
priority: high
type: functional
requirements: [F-CAL-13]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, series]
---

# TC-018: A recurring meeting's card shows what Echo does with every meeting of its series: Record or Don't record, and the language

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today

## Test Data

| Meeting (id) | Series rule | Shown on the card |
|---|---|---|
| Acme weekly sync (`ev-acme-sync`) | always, English | Record pressed, language "English ▾" |
| Globex weekly (`ev-globex-weekly`, 23:50-type) | ask (CLI only), any language | Don't record pressed, language "Any language ▾" |
| Acme design review (`ev-m01`) | not in a series | no "Every meeting of this series" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-acme-sync"]` (Acme weekly sync) | In the card the heading "Every meeting of this series" with a group "Record every meeting": "Record" has `aria-pressed="true"`, "Don't record" `false`; the language chip reads "English ▾" |
| 2 | Click the block `[data-ev="ev-globex-weekly"]` (Globex weekly at the 23:50-type time) | The heading "Every meeting of this series"; "Don't record" is pressed, "Record" not; the chip reads "Any language ▾" |
| 3 | Click the block `[data-ev="ev-m01"]` (Acme design review) | No "Every meeting of this series" in its card |
| 4 | Run `echoctl series list` | JSON with the keys `id:S-ACME-WEEKLY` (record always, languages en), `id:S-GLOBEX-WEEKLY` (record ask), `id:S-INITECH-ALLHANDS` (record never) and `subject:umbrella pricing call` (record ask, languages uk) |

## Expected Final State
The cards agree with the series rules.

## Teardown
- Click "Close" on the card

## Notes
- Creates or changes nothing. The rule `ask` has no button: the card shows it as Don't record (the CLI keeps it). `never` and the subject-keyed rule have no meeting in the agenda to show them on (a subject rule shows on no card: only meetings with a series id carry one).
- Record putting each occurrence in "Record when it starts" (`F-CAL-09`/`F-CAL-13`) is done by the app's watcher; the served window only writes the rule (`TC-019`).
