---
id: TC-012
title: After a failed calendar sync, Your day leaves out the older agenda's meetings and says the calendar couldn't be read today
priority: medium
type: regression
requirements: [F-TODAY-09]
module: today
run: agent
platform: any
data: standard
tags: [today, calendar]
---

# TC-012: After a failed calendar sync, Your day leaves out the older agenda's meetings and says the calendar couldn't be read today

**Module:** Today | **Priority:** Medium | **Type:** Regression

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every shell step runs after `. <dir>/env`
- A failed sync (offline, an expired sign-in, the calendar command failing) keeps the last `agenda.json`; the case makes yesterday's by moving every meeting of `$ECHOCTL_HOME/.echo/agenda.json` back one day

## Test Data

| Field | Value |
|---|---|
| Agenda | `$ECHOCTL_HOME/.echo/agenda.json`, kept as `<dir>/agenda.bak.json` and put back at the end |
| Notice | "The calendar couldn’t be read today: its meetings aren’t listed." |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `cp "$ECHOCTL_HOME/.echo/agenda.json" <dir>/agenda.bak.json && python3 -c 'import json,os,datetime as D;p=os.environ["ECHOCTL_HOME"]+"/.echo/agenda.json";a=json.load(open(p));[m.__setitem__(k,(D.datetime.fromisoformat(m[k])-D.timedelta(days=1)).isoformat()) for m in a for k in ("start","end") if m.get(k)];json.dump(a,open(p,"w"))'` | Exit 0 |
| 2 | Reload `{{base_url}}/#today` and wait for "Your day" | Under "Your day" the notice "The calendar couldn’t be read today: its meetings aren’t listed." |
| 3 | Read Your day's rows | Only today's recorded calls (each with "Open the call", Minutes or a minutes state); no "Industry webinar", no "Record when it starts" / "Don't record", no "Import Teams transcript" |
| 4 | Read the line under the greeting | It has no "meetings" count |
| 5 | Run `cp <dir>/agenda.bak.json "$ECHOCTL_HOME/.echo/agenda.json"`, reload `{{base_url}}/#today` | The notice is gone; today's meetings are listed again (Globex design review, Acme demo, …) |

## Expected Final State
The agenda is today's again; Your day lists its meetings with no notice.

## Notes
- `/api/upcoming` (Coming up) already left the older meetings out; Today now agrees with it.
