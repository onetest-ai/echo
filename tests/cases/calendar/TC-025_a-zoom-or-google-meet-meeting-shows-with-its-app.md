---
id: TC-025
title: A Zoom or Google Meet invite with its link only in the body shows in the calendar and Today, labelled with its app
priority: high
type: functional
requirements: [F-CAL-17, F-TODAY-02]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, today, zoom, google-meet]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-025: A Zoom or Google Meet invite with its link only in the body shows in the calendar and Today, labelled with its app

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run (and the run before 21:00 local: the two meetings below must still be to come): `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`
- The two invites are Microsoft 365 events that are not online meetings in Graph's terms: `isOnlineMeeting` false, no `onlineMeeting`, an empty location, the link only in `bodyPreview` (Zoom's invite text; Meet's link bare, as Google writes it). The data builder made them agenda meetings with Echo's own `echoctl agenda sync` (`other_app_invites` in `tests/fixtures/make_data.py`)
- A meeting's block in the Day view is `[data-ev="<id>"]`; its card is `aside.detail`; Today's rows are `.td-row`

## Test Data

| Meeting (id) | When | Link in its body | App label |
|---|---|---|---|
| Umbrella pricing review (`ev-zoom-pricing`) | about 1 h 45 min after the build, 30 min, Zoe Martin | `https://zoom.us/j/85550100123?pwd=TestOnly` | Zoom |
| Globex roadmap chat (`ev-meet-roadmap`) | about 2 h 30 min after the build, 30 min, Kim Park | `meet.google.com/tst-wxyz-abc` | Google Meet |
| Acme help desk sync (`ev-m29`, to come, a Teams link) | 37 minutes after the build | — | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl status` | JSON whose `agenda` holds `ev-zoom-pricing` with `join_url` "https://zoom.us/j/85550100123?pwd=TestOnly" and `ev-meet-roadmap` with `join_url` "https://meet.google.com/tst-wxyz-abc" (the bare link made https) |
| 2 | Navigate to `{{base_url}}/#calendar/day` and wait for "Day" | The blocks `[data-ev="ev-zoom-pricing"]` "Umbrella pricing review" and `[data-ev="ev-meet-roadmap"]` "Globex roadmap chat" are in today's column; their time lines read "Zoom" and "Google Meet" (a `.tag.app` each); `[data-ev="ev-m29"]` (Teams) has no `.tag.app`, and the whole grid has exactly 2 (`document.querySelectorAll('.cal .ev .tag.app').length`) |
| 3 | Click the block `[data-ev="ev-zoom-pricing"]` | Its card: heading "Umbrella pricing review", the chip "Zoom" first in its chips row, then the kind and thread chips; People: Zoe Martin, organizer |
| 4 | Click "Close" on the card, click `[data-ev="ev-meet-roadmap"]` | Its card: heading "Globex roadmap chat", the chip "Google Meet"; People: Kim Park, organizer |
| 5 | Click "Close" on the card; navigate to `{{base_url}}/#today` and wait for "Your day" | Your day has the rows "Umbrella pricing review" with the label "Zoom" and "Globex roadmap chat" with "Google Meet" beside the title, in time order after the meetings at 37 minutes after the build; "Acme help desk sync" has no label |

## Expected Final State
Both invites are on the day with their app named; nothing changed.

## Notes
- Creates or changes nothing. What the cards and rows offer instead of Record is `TC-026`.
- Before #228 the calendar sync dropped both invites (not online meetings in Graph's terms) and the data builder stops with "the calendar sync didn't keep the zoom invite".
- The link finding itself (each app, location before body, HTML, trailing punctuation, look-alike hosts) is Rust: `apps::tests`, `calendar::tests::a_zoom_or_google_meet_invite_is_kept_with_the_link_from_its_text`.
