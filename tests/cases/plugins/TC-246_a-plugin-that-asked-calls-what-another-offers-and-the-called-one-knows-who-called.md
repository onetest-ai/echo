---
id: TC-246
title: A plugin whose sheet asked for other plugins calls a command another offers, and the called one knows who called; without that line, or for a command not offered, it is refused
priority: high
type: functional
requirements: [F-PLUG-157]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, offers, permissions]
---

# TC-246: One plugin calls another

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- As TC-245: the same data, prelude and the marketplace `acme-offers` (run its precondition lines when it isn't added yet)

**Changes:** `<config dir>/plugins/` (the plugins, `offers/`), the plugins' stores

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install acme-asker` (no `--yes`: the sheet only) | The sheet lists `Calls the commands your other plugins offer and gets what they answer — what those plugins read or do for you, it can have them read or do for it` |
| 2 | `$E plugin install acme-asker --yes`; `$E plugin install acme-nosy --yes`; `$E plugin install acme-vaults --yes` | All three install |
| 3 | `$E plugin call acme-asker list` | `[{"description": "Search and notes across the Acme vaults", "id": "acme-vaults", "name": "Acme vaults"}]` |
| 4 | `$E plugin call acme-asker ask '{"id":"acme-vaults","name":"search","args":{"query":"Acme pilot"}}'` | `{"from": "acme-asker", "limit": 10, "query": "Acme pilot"}`: its default, and who called |
| 5 | `$E plugin call acme-vaults search '{"query":"Acme pilot"}'` | `"from": null`: called as its own command, from no plugin |
| 6 | `$E plugin call acme-asker ask '{"id":"acme-vaults","name":"search","args":{}}'`; then the same with `"name":"secret"`; then with `"id":"acme-quiet","name":"ping"` | Each fails: `acme-vaults's “search” needs “query”`; `acme-vaults offers no command “secret”`; `no plugin “acme-quiet” offers commands` |
| 7 | `$E plugin call acme-nosy ask '{"id":"acme-vaults","name":"search","args":{"query":"x"}}'` | Fails: `acme-nosy didn't ask for “plugins” when it was installed` |

## Teardown
- `$E plugin remove acme-asker`; `$E plugin remove acme-nosy` (Acme vaults stays for TC-247; removed there)
