---
id: TC-031
title: A plugin may declare 24 widgets — installed, all in the catalog and in Add a widget under its name, the list scrolling; a 25th refused at install
priority: medium
type: functional
requirements: [F-TODAY-19]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, widgets, manifest]
---

# TC-031: A plugin with 24 widgets

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env` (`E=target/debug/echoctl`); the browser 1440 × 900
- In My plugins' folder (`$E plugin mine` names it) a folder `many` made by:
  `python3 -c 'import json,sys; d=sys.argv[1]; n=int(sys.argv[2]); open(d+"/echo-plugin.json","w").write(json.dumps({"id":"many","name":"Many widgets","version":"0.1.0","summary":"A test","main":"main.js","widgets":[{"id":f"w{i}","label":f"Widget {i}","about":f"number {i}","kind":"stat","sizes":["S","M"]} for i in range(n)]}))' <folder>/many 24`
  and `<folder>/many/main.js` holding `export function apply(ctx) { for (let i = 0; i < 24; i++) ctx.widgets.provide(`w${i}`, () => ({ value: i, label: 'items' })) }`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace update my-plugins`; `$E plugin install many --from my-plugins --yes` | "Installed Many widgets 0.1.0"; the sheet said it adds 24 widgets |
| 2 | `$E widget list` | 24 `plugin:many:w0` … `plugin:many:w23`, part "Many widgets" |
| 3 | Open `{{base_url}}/#today`; ✎ → Add a widget; the part chip "Many widgets" | 24 rows, Widget 0 … Widget 23, each with its about; the list scrolls inside the panel to the last one; the other parts' chips still there |
| 4 | Click All, then search "Widget 23" | One row, "Widget 23"; Add puts it on Today showing 23 |
| 5 | `$E today reset`; `$E plugin remove many`; make the manifest again with 25 (the same command, `25` at the end); `$E plugin marketplace update my-plugins`; `$E plugin install many --from my-plugins --yes` | Refused: "“many” can't be installed: it declares more than 24 widgets"; `$E plugin list` shows many not installed (`"installed": null`) |

## Expected Final State
No plugin "many" installed; Today as it comes.

## Teardown
- Delete `<folder>/many`

## Changes
- My plugins' folder in the test data folder.
