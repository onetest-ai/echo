---
id: TC-014
title: Import Teams transcript without Microsoft 365 ends in a clear message on the card, and the meeting stays unrecorded
priority: medium
type: functional
requirements: [F-CAL-15, F-CAL-08]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, card, import]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-014: Import Teams transcript without Microsoft 365 ends in a clear message on the card, and the meeting stays unrecorded

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A meeting's block in the Day view is `[data-ev="<id>"]`; clicking it opens its card (`aside.detail`) on the right. The data's calendar is the agenda file (not signed in to Microsoft 365), so all meetings are today
- Built less than 15 minutes ago: meetings "live" at the build (started 10 minutes before it) end 20 minutes after it and then read as past; "to come" ones start 37 minutes after it

## Test Data

| Field | Value |
|---|---|
| Meeting | Acme design review (`ev-m01`), past, not recorded |
| Expected message | not signed in to Microsoft 365 — `echoctl calendar login` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day", click the block `[data-ev="ev-m01"]` (Acme design review) | The card shows "Import Teams transcript" |
| 2 | Click "Import Teams transcript" and wait up to 10 seconds | The button is not offered as working any more: the card's line (`.why`) reads "not signed in to Microsoft 365 — `echoctl calendar login`" |
| 3 | Look at the block `[data-ev="ev-m01"]` (Acme design review) | It is still not green (no class `recorded`) |
| 4 | Run `ls "$ECHOCTL_HOME/calls" | grep -c acme-design-review` | `ls "$ECHOCTL_HOME/calls" | grep -c acme-design-review` would print 1 (only the data's own earlier call): nothing was added |

## Expected Final State
The card says why the import did not happen; no call was made.

## Teardown
- Click "Close" on the card

## Notes
- In S-24 `inbox/TC-012` runs just before and leaves this card open with the message already on it (the server keeps the import's error): step 1 reads the open card, and the message must still show after this case's own click.
- Nothing is created: the import fails before it writes. The same message shows on the Inbox item "Teams transcript: Acme design review" (`inbox/TC-012`). In the data a call "Acme design review" already exists (imported, 20:12-type time) and is not this meeting's: the meeting is at 22:28-type time.
