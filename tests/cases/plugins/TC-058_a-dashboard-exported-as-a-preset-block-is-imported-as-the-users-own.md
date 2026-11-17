---
id: TC-058
title: "echoctl dashboard export prints a dashboard as a preset block without the page's client or thread, and import adds one as the user's own"
priority: medium
type: functional
requirements: [F-PLUG-41]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, dashboards, presets, cli]
---

# TC-058: Export and import a dashboard

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install board-demo --from test-plugins --yes`; `$E dashboard add-preset board-demo staffing`
- `F=$(mktemp)`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E dashboard export staffing --client > "$F"`; `cat "$F"` | `{"id": "staffing", "kind": "client", "name": "Staffing", "items": [{"widget": "plugin:board-demo:glance", "size": "L", "row": true}, {"widget": "plugin:board-demo:people", "size": "M", "row": true, "params": {"order": "role"}}]}` — no `client` in any `params` |
| 2 | `$E dashboard import "$F" --client` | Refused: "there is a dashboard \"Staffing\" already" |
| 3 | `sed 's/"Staffing"/"Staffing copy"/' "$F" \| $E dashboard import - --client`; `$E dashboard list --client` | `{"id": "staffing-copy", "name": "Staffing copy"}` (no `from`: the user's own); the list has both |
| 4 | Load `{{base_url}}/#client/acme/b-staffing-copy` | "Staffing copy" with the same two widgets as Staffing, for Acme; no line from a plugin |
| 5 | `$E dashboard import "$F" --thread` | Refused: "it is a client's dashboard, not a thread's" |
| 6 | Teardown below | — |

## Teardown
- `$E dashboard remove staffing --client`; `$E dashboard remove staffing-copy --client`; `$E plugin remove board-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -f "$F"`; `tests/fixtures/serve.sh down <dir>`
