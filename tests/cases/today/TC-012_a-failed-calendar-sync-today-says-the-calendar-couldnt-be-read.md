---
id: TC-012
title: After last night's empty agenda, Your day says the calendar couldn't be read only once today's sync has failed
priority: medium
type: regression
requirements: [F-TODAY-09]
module: today
run: agent
platform: any
data: standard
tags: [today, calendar]
---

# TC-012: After last night's empty agenda, Your day says the calendar couldn't be read only once today's sync has failed

**Module:** Today | **Priority:** Medium | **Type:** Regression

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- A sync keeps only the meetings that haven't ended, so the last good sync of a day writes an empty agenda; if the sign-in expires overnight, the next day's syncs fail and keep it (#131). The case makes that agenda (`agenda clear`, its time moved back to yesterday) and a calendar command that fails (`calendar_command` in `config.json`, so no real calendar is read)

## Test Data

| Field | Value |
|---|---|
| Agenda | `$ECHOCTL_HOME/.echo/agenda.json`, kept as `<dir>/agenda.bak.json` and put back at the end |
| Config | `$ECHOCTL_HOME/.echo/config.json`, kept as `<dir>/config.bak.json` and put back at the end |
| Notice | "The calendar couldn’t be read today: today’s meetings may be missing." |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `S="$ECHOCTL_HOME/.echo"; cp "$S/agenda.json" <dir>/agenda.bak.json && cp "$S/config.json" <dir>/config.bak.json && target/debug/echoctl agenda clear && python3 -c 'import json,os,time;s=os.environ["ECHOCTL_HOME"]+"/.echo/";t=time.time()-36*3600;os.utime(s+"agenda.json",(t,t));c=json.load(open(s+"config.json"));c["calendar_command"]=["python3","-c","raise SystemExit(1)"];json.dump(c,open(s+"config.json","w"),indent=2)'` | "agenda: 0 meeting(s)", exit 0 |
| 2 | Reload `{{base_url}}/#today` and wait for "Your day" | No notice about the calendar (no sync has been tried today: just after midnight looks the same); Your day lists only today's recorded calls, no "Record when it starts" |
| 3 | Run `target/debug/echoctl agenda sync` | One JSON line with `"synced":false` and a `reason` starting "calendar command failed" |
| 4 | Reload `{{base_url}}/#today` and wait for "Your day" | Under "Your day" the notice "The calendar couldn’t be read today: today’s meetings may be missing."; the rows are as in step 2 |
| 5 | Run `S="$ECHOCTL_HOME/.echo"; cp <dir>/config.bak.json "$S/config.json" && cp <dir>/agenda.bak.json "$S/agenda.json" && rm -f "$S/calendar-failed.json"`, reload `{{base_url}}/#today` | The notice is gone; today's meetings are listed again (Globex design review, Acme demo, …) |

## Expected Final State
The agenda and config are as built; Your day lists today's meetings with no notice.

## Notes
- The notice depends on when the agenda was last written by a good sync, not on its meetings: an empty agenda from last night is caught too.
- A failed sync from an earlier day, or a good one today after a failure, says nothing (`calendar::tests::the_calendar_is_unread_today_only_after_a_failed_sync_with_no_good_one_today`).
