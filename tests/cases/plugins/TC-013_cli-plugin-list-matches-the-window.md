---
id: TC-013
title: echoctl plugin list gives installed, offered and marketplaces
priority: medium
type: functional
requirements: [F-PLUG-10]
module: plugins
run: agent
platform: any
tags: [plugins, cli]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-013: echoctl plugin list gives installed, offered and marketplaces

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash (`. <dir>/env` first) run `echoctl plugin list` | JSON with the keys `busy`, `discover`, `errors`, `installed`, `markets` |
| 2 | Read `installed` | `[]` |
| 3 | Read `markets` | One entry: `"name": "onetest"`, `"source": "onetest-ai/echo-plugins"`, `"built_in": true`, `"hidden": false` |

## Expected Final State
The CLI shows no plugin and the built-in marketplace, as the window does.

## Notes
- `discover` and the marketplace's `count` come from the network; not asserted.
