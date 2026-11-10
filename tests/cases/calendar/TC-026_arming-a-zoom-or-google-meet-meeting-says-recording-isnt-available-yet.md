---
id: TC-026
title: Arming a Zoom or Google Meet meeting says recording isn't available for that app yet, in the window and the CLI
priority: high
type: functional
requirements: [F-CAL-18, F-CAL-08, F-CAL-16, F-TODAY-02]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, today, cli, zoom, google-meet, recording]
---

# TC-026: Arming a Zoom or Google Meet meeting says recording isn't available for that app yet, in the window and the CLI

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run (and the run before 21:00 local: the two meetings below must still be to come): `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`
- The data's Zoom meeting Umbrella pricing review (`ev-zoom-pricing`) and Google Meet meeting Globex roadmap chat (`ev-meet-roadmap`), to come (`TC-025`); Acme help desk sync (`ev-m29`), to come with a Teams link, for comparison
- Right-click with Playwright's `click({ button: 'right' })` on the block; the menu is `.ev-ctx`, its items `.menu-item`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day`, wait for "Day", click the block `[data-ev="ev-zoom-pricing"]` | The card's action strip has no Record button (no `[data-schedule]`, `[data-join]` in `aside.detail`; a "Prepare" may stand there); its line (`.why`) starts "Recording isn't available for Zoom meetings yet." |
| 2 | Click "Close" on the card, click `[data-ev="ev-meet-roadmap"]` | No Record button; the line starts "Recording isn't available for Google Meet meetings yet." |
| 3 | Click "Close" on the card, click `[data-ev="ev-m29"]` | The Teams meeting still offers "Record when it starts" (as `TC-013` step 8); click "Close" |
| 4 | Right-click `[data-ev="ev-zoom-pricing"]` | The menu holds "Prepare", "Add to a thread…", "Copy the Zoom link" and "Show details": no "Record when it starts", no "Join & record"; press Escape |
| 5 | Navigate to `{{base_url}}/#today`, wait for "Your day"; open the ⋯ of "Umbrella pricing review", Escape; open "Show N skipped" if "Acme help desk sync" is folded, open its ⋯ | The line "Umbrella pricing review" says "Recording isn't available for Zoom meetings yet." and its menu has no "Record when it starts"; "Globex roadmap chat" says "Recording isn't available for Google Meet meetings yet."; Acme help desk sync's menu still has "Record when it starts" (Your day's lines carry no buttons: the actions are in ⋯) |
| 6 | Run `target/debug/echoctl arm --meeting - <<'EOF'` with the lines `{"id": "ev-zoom-pricing", "subject": "Umbrella pricing review", "join_url": "https://zoom.us/j/85550100123?pwd=TestOnly"}` and `EOF`, then `echo "exit $?"` | `Error: “Umbrella pricing review” is a Zoom meeting: recording isn't available for Zoom meetings yet` and `exit 1` |
| 7 | Run `target/debug/echoctl join - <<'EOF'` with `{"subject": "Globex roadmap chat", "join_url": "https://meet.google.com/tst-wxyz-abc"}` and `EOF`, then `echo "exit $?"` | `Error: “Globex roadmap chat” is a Google Meet meeting: recording isn't available for Google Meet meetings yet` and `exit 1`; no browser tab or app opens (refused before the link) |
| 8 | Run `target/debug/echoctl status` and `ls "$ECHOCTL_HOME/.echo/armed.json"` | `armed` is null; "No such file or directory": nothing was armed |

## Expected Final State
Nothing armed, scheduled or opened; Teams meetings offer recording as before.

## Notes
- Creates or changes nothing.
- The scheduler, a series set to Record, the "Record?" offer at a meeting's start, the tray's Upcoming and the Teams transcript offers skip such meetings: Rust tests (`schedule::tests::a_zoom_or_google_meet_meeting_is_never_scheduled`, `series::tests::always_skips_a_zoom_or_google_meet_meeting`, `teams_transcript::tests::a_zoom_or_google_meet_meeting_offers_no_teams_transcript`, `watch::tests::a_zoom_or_google_meet_meeting_is_not_armed_or_joined`); the offer and the tray need the app's clock and a person.
