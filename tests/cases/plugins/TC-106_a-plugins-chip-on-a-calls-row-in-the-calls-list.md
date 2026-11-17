---
id: TC-106
title: A plugin's chip shows on the rows of the calls it is for in the calls list, opens the call like the row, and goes when the plugin is switched off
priority: medium
type: functional
requirements: [F-PLUG-74]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, slots, calls-list]
---

# TC-106: A plugin's chip on a call's row in the calls list

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- Data demo installed: in S-36, stage 1. On its own: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes` (its window half puts a chip "Has chat" on each call that has a meeting chat)

**Changes:** Data demo switched off and on again

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#calls`; type `kickoff` in "Search calls" | The row "Globex pilot kickoff" shows, in its second line after its time and length, a chip "Has chat" whose tooltip is "Data demo: this call has a meeting chat" |
| 2 | Clear the search, type `Initech planning` | Its rows show no "Has chat" chip (no chat) |
| 3 | Search `kickoff` again; click the chip "Has chat" | The call Globex pilot kickoff opens, as a click on its row does |
| 4 | Navigate to `{{base_url}}/#settings/plugins`, switch "Data demo on" off; navigate to `{{base_url}}/#calls`, search `kickoff` | The row has no "Has chat" chip |
| 5 | Switch "Data demo on" on again (Settings → Plugins); back to `{{base_url}}/#calls`, search `kickoff` | The chip "Has chat" is back |

## Expected Final State
Data demo is installed and on.

## Teardown
- In S-36: none. On its own: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A chip is text only (Echo escapes it), cut at 24 characters; one that throws is left out and the list draws; a second chip with the same id is left out — `node crates/echoctl/ui/call-row-slot.test.mjs`.
