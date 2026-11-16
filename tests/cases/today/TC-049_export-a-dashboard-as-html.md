---
id: TC-049
title: Export as HTML — Today, a client's Overview and a dashboard of its own saved to Downloads as one self-contained file that looks like Echo, with each widget's data as tables
priority: medium
type: functional
requirements: [F-TODAY-30]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, dashboards, clients, export]
---

# TC-049: Export a dashboard as HTML

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (its `env` sets `ECHOCTL_TEST_OPEN_LOG_ONLY` and `ECHOCTL_DOWNLOADS=<dir>/downloads`: nothing is saved into the user's own Downloads, nothing is opened); the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install table-demo --yes`; `target/debug/echoctl plugin install chart-demo --yes`
- `target/debug/echoctl today add plugin:chart-demo:hours --size M`; `target/debug/echoctl today --client acme add plugin:table-demo:roles`; `target/debug/echoctl dashboard add --client Staffing`; `target/debug/echoctl today --client acme --board staffing add plugin:table-demo:positions --size S`
- To look at a saved file, serve the downloads folder: `python3 -m http.server <port+1> --bind 127.0.0.1 --directory <dir>/downloads` (the browser can't open file://); stop it at the end

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today` | Today's head has, at its right, a download icon (aria-label "Export as HTML", tooltip "Export as HTML — Today as it shows now, with its data, saved to Downloads") just before ✎ "Edit Today" |
| 2 | Click ✎; look at the head; click Done | While editing there is no "Export as HTML" (Add a widget, Reset to the default, Done); after Done it is back |
| 3 | Click "Export as HTML" | A notice "Saved to Downloads: Echo - Today - <YYYY-MM-DD HHMM>.html"; `ls <dir>/downloads` lists that one file; `<dir>/open.log` ends with `reveal <dir>/downloads/Echo - Today - ….html` (shown in Finder / Explorer outside test data) |
| 4 | `grep -ci '<script' "<dir>/downloads/Echo - Today - "*.html`; `grep -c '<button\|<input' …` on it | 0 and 0: no script, no buttons or inputs in the file |
| 5 | Open the file through the downloads server | Its title "Today — Echo"; the head "Echo", "Today", "Snapshot of <weekday, date> at <time>"; then Today's widgets as on the page, in Echo's colours (Your day, Next meeting, Needs you now, The day's choices, My work · this week, Billable hours with its chart drawn); then "Data": a box a placed widget — "Your day · agenda" (Time, What, Kind, Threads, Answer, Minutes), "My work · this week" (Task, Whose, Thread, Due, "Send Acme the SSO checklist" among them), "Billable hours" (a row a week with its value) — no box for a widget not on Today; the browser made no request but the file's own; no console error |
| 6 | Emulate the dark colour scheme (`browser_emulate_media` dark), then back | The file turns dark as Echo does, then light again |
| 7 | Open `{{base_url}}/#client/acme/overview` (reload the page); click "Export as HTML" beside the dashboard's ✎ | Notice "Saved to Downloads: Echo - Acme Overview - ….html"; opened: head "Acme · Overview"; "Roles to fill" drawn as its table, its positions' links opening outside (`target="_blank"`, `https://staffing.acme.example/…`), no details chevrons; "Data" → "Roles to fill" with Position and Role and both rows |
| 8 | Open `{{base_url}}/#client/acme/b-staffing`; click "Export as HTML" | Notice "Saved to Downloads: Echo - Acme Staffing - ….html"; opened: head "Acme · Staffing", "Open positions" with its number, "Data" → "Open positions" with Value |
| 9 | On Today click "Export as HTML" twice within the same minute | Two files: "Echo - Today - <same minute>.html" and "… (2).html"; the first unchanged |
| 10 | Open `{{base_url}}/#project/acme-rollout/brief` (no widgets on its dashboard) | Only ✎ above the Brief, no "Export as HTML" |

## Expected Final State
Each export is one HTML file in `<dir>/downloads`, named "Echo - <page> - <date time>.html", self-contained (Echo's styles inline, no script, no outside request), showing the widgets as drawn and their data as tables; nothing in the user's own Downloads, nothing opened.
