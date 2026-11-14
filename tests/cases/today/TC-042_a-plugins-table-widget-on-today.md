---
id: TC-042
title: A plugin's table widget on Today — column names in small caps that stay put while its rows scroll inside the card, rows under their group's header with a count, pills, dates (the year when not this one), numbers at the end, a column hidden at M, a row's details on ▸, its link (https opens on the first click), its text escaped; data it can't draw says why; stats items' "Details ›"
priority: high
type: functional
requirements: [F-TODAY-27]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, untrusted-text]
---

# TC-042: A plugin's table widget on Today

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (its `env` sets `ECHOCTL_TEST_OPEN_LOG_ONLY`: an outside link goes to `<dir>/open.log`, never to the browser); the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- The test plugin `table-demo` from `tests/fixtures/plugins` (marketplace "test-plugins"): `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install table-demo --yes`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl widget list` | Five `plugin:table-demo:…` widgets (`roles` too, TC-044): `staffing` (kind "table", sizes M and L, heights 1 and 2), `counts` (kind "stats"), `wrong` (kind "table"), `positions` (kind "stat", for TC-044) |
| 2 | `target/debug/echoctl today add plugin:table-demo:staffing --size L --above core:calendar.agenda`; `today add plugin:table-demo:counts --size M` and `today add plugin:table-demo:wrong --size M --after plugin:table-demo:counts`; open `{{base_url}}/#today` (reload once if a widget hasn't answered yet) | "Needs attention" at the top: its header "NEEDS ATTENTION 13 · 2 critical, 2 high"; column names in small caps — Urgency, Position, Role, Start, Status, Candidates, Why; groups in this order, each with its count: "NWB-PAI · NORTHWIND BANK 2", "CTL-GENAI · CONTOSO LABS 2", "LATER THIS QUARTER 8", then "OTHER 1" (the row without a group). Urgency as outlined pills — Critical red, High amber, Normal green, On hold grey; Start as "Nov 15", "Nov 24" and "Jan 5, 2027" (as Echo writes dates: the year only when it isn't this one — the fixture's dates are fixed, so run in 2026); Candidates right-aligned; the row of 1101000005 shows its role `<img src=x onerror=alert(1)> kept as text` as text (no image, no dialog) |
| 2a | Read the header's end (`[data-pw="plugin:table-demo:staffing"] .pw-upd`): its text and its tooltip | A refresh icon and a time three hours before now (the plugin's `asOf`: when its own read was), not the time it answered; the tooltip "Updated <that time>" |
| 3 | Scroll inside the table (the wheel over its rows) to its end | The rows scroll inside the card; the column names stay at the top of the card; the page itself does not move; the last row is 1101000007, Delivery Manager |
| 4 | Click ▸ beside 1101000001 | Under the row, indented: "Dana Example — Proposed · due Nov 14", "Lee Sample — Booked · due Nov 15", "Ira Placeholder — Preselected"; the ▸ is ▾ (its label "Hide details"). Click the ▾: the lines go |
| 5 | Hover the link starting "1101000002 ·" (underlined, the row's link) | Its tooltip "Open in the browser (staffing.acme.example)" |
| 6 | Click it once; then `cat <dir>/open.log` | It opens at once, nothing asked (no "in the browser?" line under the rows); `open.log` ends with `https://staffing.acme.example/position/1101000002` |
| 7 | Click 1101000003 (an `echo://` link) | It opens in Echo, no question first: the window shows the thread Acme rollout's Brief (`#project/acme-rollout/brief`); nothing more in `open.log` |
| 8 | `target/debug/echoctl today size plugin:table-demo:staffing M`; reload | The columns Status and Why are gone; the rest as at L. 1101000002's position (a link, "1101000002 · Key Engineer for the DevOps platform…") ends with … before the Role column, a gap between them — never over Role; its whole text on hover |
| 9 | Look at "Staffing" and "Wrong table" | Staffing: 2 critical (red) with "Details ›", 2 high (amber) with "Details ›", 1 on hold without; clicking the second "Details ›" opens it at once (a new last line in `open.log`, under `https://staffing.acme.example/`). Wrong table: "Couldn't draw this — row 1's cell “start” isn't a date (YYYY-MM-DD)" |
| 10 | `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove table-demo` | Today as it comes; `widget list` has no `plugin:table-demo:` widget |

## Teardown
- `target/debug/echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`; close the browser tab

## Notes
- Each shape (columns, groups and their order, hide at M, pills, dates, numbers, links, details, escaping, every refusal, stats links) is the node test `today-plugin-widgets.test.mjs`; a table refused at size S is the Rust test `plugins::a_table_widget_takes_sizes_m_and_l`.
- Where Today places an L widget (a full row, or beside Your day) is Today's layout (`today/TC-021`), not the table's. A plugin widget's declared `height: "fill"` isn't honoured by Today yet (#523): the table caps its own scroll area instead.
