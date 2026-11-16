---
id: TC-030
title: The CLI takes a marketplace by its name in any case or by its source; one not found is named with the marketplaces there are
priority: medium
type: functional
requirements: [F-PLUG-10]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, cli]
---

# TC-030: A marketplace named in the CLI

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built right before the run: `tests/fixtures/make_data.py <dir>`; CLI steps after `. <dir>/env`
- `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name Test-Plugins`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl plugin marketplace update test-plugins` | Exit 0 (the marketplace "Test-Plugins" read again) |
| 2 | `target/debug/echoctl plugin install widget-demo --from TEST-PLUGINS --yes` | "Installed Widget demo …" |
| 3 | `target/debug/echoctl plugin marketplace hide "$PWD/tests/fixtures/plugins"`; `plugin marketplace list` | "Test-Plugins" has `"hidden": true` (found by its folder) |
| 4 | `target/debug/echoctl plugin marketplace show test-plugins`; `target/debug/echoctl plugin marketplace update globex` | Shown again; the second fails: `no marketplace “globex” — the marketplaces: …` naming Test-Plugins among them |
| 5 | `target/debug/echoctl plugin marketplace remove ONETEST` | Refused: the built-in marketplace can be hidden, not removed |
| 6 | `target/debug/echoctl plugin remove widget-demo`; `target/debug/echoctl plugin marketplace remove test-plugins` | Both gone |
