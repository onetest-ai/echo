---
id: TC-056
title: "A plugin's dashboards are offered in the dashboards' + under From plugins and added only by the user, once; a thread's for pursuit threads shows only on them"
priority: high
type: functional
requirements: [F-PLUG-39]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, dashboards, presets]
---

# TC-056: A plugin's dashboard, offered and added

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; `{{base_url}}/` loaded once after the install below; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- The test marketplace `tests/fixtures/plugins` ("test-plugins") with "board-demo" (Board Demo: a client's "Staffing" — Team glance L, People M by role — and a thread's "Pursuit" for pursuit threads)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install board-demo --from test-plugins` | The sheet has "Adds 4 widgets you can put on Today" and, apart from the lines, "Offers dashboards: Staffing (clients), Pursuit (threads)"; it asks for `--yes` |
| 2 | `$E plugin install board-demo --from test-plugins --yes`; `$E dashboard list --client`; `$E dashboard presets --client` | Installed; no dashboards (none added by itself); one preset: "Staffing", `"added": false`, "Board Demo" |
| 3 | Load `{{base_url}}/#client/acme/overview`; click "+" after the tabs (`[data-board-add="client"]`) | The name field "Dashboard name" with a menu under it, "From plugins": "Staffing — Board Demo" |
| 4 | Press Tab (the row "Staffing — Board Demo" takes the focus, the menu stays), then Enter | A tab "Staffing" after Overview, open: "Staffing", "On every client, each showing its own"; a wide "Team glance · Acme" "3 people at acme", under it "People · Acme" with "Dana Lee" (lead) and "Omar Haddad" (engineer) |
| 5 | Click "+" again | Only the name field: nothing from plugins left to offer |
| 6 | `$E dashboard add-preset board-demo staffing` | Refused: "Board Demo's “Staffing” is added already" |
| 7 | Load `{{base_url}}/#project/acme-rollout/brief` (Active); click "+" after the tabs; click "Pursuit — Board Demo" | The page stays on Acme rollout's Brief; its tabs: Brief, Work, … — no Pursuit (it is for pursuit threads) |
| 8 | Load `{{base_url}}/#project/acme-renewal/brief` (Negotiation) | Its tabs: Brief, Pursuit, Work, …; "Pursuit" opens "Pursuit steps · Acme renewal" |
| 9 | Teardown below | — |

## Teardown
- `$E dashboard remove staffing --client`; `$E dashboard remove pursuit --thread`; `$E plugin remove board-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The manifest's checks of a preset: `plugins::presets::tests`; the "+" rows and the `for` rule: `board-presets.test.mjs`.
