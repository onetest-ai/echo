---
id: TC-057
title: "A dashboard from a plugin says when the plugin's update changed it — Update replaces the user's changes, Keep mine keeps them — and when its plugin is off or removed it stays the user's"
priority: high
type: functional
requirements: [F-PLUG-40]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, dashboards, presets, update]
---

# TC-057: A plugin's dashboard after its plugin changed or went

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- A copy of the test marketplace (its board-demo gets updated): `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `$E plugin marketplace add "$C" --name test-plugins`; `$E plugin install board-demo --from test-plugins --yes`; `$E dashboard add-preset board-demo staffing`
- The user's own change to it: `$E today --client acme --board staffing remove plugin:board-demo:people`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Load `{{base_url}}/#client/acme/b-staffing` | "Staffing" with only "Team glance · Acme"; no line from its plugin |
| 2 | Board Demo 0.2.0 (its Staffing: People L, then Team glance M): `cp "$C/next/board-demo/echo-plugin.json" "$C/plugins/board-demo/echo-plugin.json"`; `$E plugin marketplace update test-plugins`; `$E plugin update board-demo --yes`; reload the page | "Updated Board Demo to 0.2.0"; under the name a group "From its plugin": "Board Demo updated this dashboard — Update replaces your changes", "Keep mine", "Update" |
| 3 | Click "Keep mine" | The line goes; the dashboard still only "Team glance · Acme" |
| 4 | `$E dashboard list --client` | Staffing's `from.version` is "0.2.0" |
| 5 | `$E dashboard add-preset board-demo staffing` | Refused: added already (Keep mine didn't drop it) |
| 6 | `$E plugin disable board-demo`; reload | "Its plugin, Board Demo, is off: its widgets show nothing" with "Remove…" |
| 7 | `$E plugin remove board-demo`; reload | "Its plugin was removed: its widgets show nothing" with "Remove…"; the tab "Staffing" still there |
| 8 | Click "Remove…" | "Remove Staffing from every client? Its widgets go with it." with "Cancel" and "Remove"; click "Cancel": it stays |
| 9 | Teardown below | — |

## Teardown
- `$E dashboard remove staffing --client`; `$E plugin marketplace remove test-plugins`; `rm -rf "$(dirname "$C")"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Update taking the new layout (People L above Team glance M) and an update when the user changed nothing: `boards::tests::a_plugins_dashboard_is_offered_added_once_by_the_user_and_says_when_its_plugin_changed_or_went`; checked by hand in the served window (the line goes, the new layout shows).
