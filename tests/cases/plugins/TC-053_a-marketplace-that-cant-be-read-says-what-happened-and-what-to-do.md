---
id: TC-053
title: A marketplace git can't reach says what happened, what to do and git's line, the whole text visible
priority: medium
type: functional
requirements: [F-PLUG-07]
module: plugins
run: agent
platform: any
tags: [plugins, marketplaces, errors]
---

# TC-053: A marketplace that can't be read says what happened and what to do

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`), `{{base_url}}/` loaded first; the browser 1440 × 900
- CLI steps run in Bash after `. <dir>/env`
- No network is used: the address ends in `.invalid`, which never resolves

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins`, click the tab "Marketplaces", type `https://me:tok@nohost.invalid/group/r.git` in "Marketplace", `nohost` in "Name" and click "Add" | A row "nohost" shows the address `https://nohost.invalid/group/r.git` and under it an error in three lines, none cut off: "Echo couldn't get the marketplace from nohost.invalid: can't reach nohost.invalid." then on its own line "Check the address and the network, then try again." then on its own line "git said: …" (git's own line, naming the host). Neither `me` nor `tok` appears anywhere on the page |
| 2 | Click "⋯" on the row, "Remove…" and confirm; run `echoctl plugin marketplace add 'https://me:tok@nohost.invalid/group/r.git' --name nohost`, then `echoctl plugin marketplace list` | "Added nohost, but it can't be read yet: " and the same three lines (the sentence, the instruction, "git said: …"); in the list nohost's `source` is `https://nohost.invalid/group/r.git` and `error` holds the three lines; no `tok` in either output |

## Expected Final State
The marketplace "nohost" is added and unreadable.

## Teardown
- `echoctl plugin marketplace remove nohost` 
