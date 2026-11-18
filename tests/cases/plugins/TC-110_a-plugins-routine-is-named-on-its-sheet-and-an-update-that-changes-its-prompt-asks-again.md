---
id: TC-110
title: A plugin's routine is named on its install sheet with what its run may do and its prompt's hash; an update that changes the prompt asks again and, taken, keeps it on; a routine breaking a rule is refused with why
priority: high
type: functional
requirements: [F-PLUG-75]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, sheet, update]
---

# TC-110: A plugin's routine is named on its sheet, and an update that changes it asks again

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served (any set; the case uses only the CLI): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A copy of the test marketplace (its routine-demo gets updated): `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `$E plugin marketplace add "$C" --name test-plugins`

**Changes:** the installed plugins, `$C/plugins/routine-demo/`

## Test Data

| Plugin | Routine demo (`routine-demo`) 0.1.0: `uses.claude`, one routine `mail-notes` "Mail → notes", every 60 min, its prompt `routines/mail-notes.md`; 0.2.0 (`$C/next/routine-demo`): the same routine, its prompt also reading meeting chats |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install routine-demo --from test-plugins` | The sheet has "Runs Claude Code on your AI profiles, acting on its own as you set routines in Settings (Auto, or skipping every permission check)" and "Has a routine “Mail → notes”, off unless you switch it on here or later in Settings → Plugins: then every 60 min Echo runs Claude Code on your default AI profile with the prompt it ships, acting on its own as you set routines — it may change your Echo data and send as you, as your Claude · Echo folder session may (its prompt: <8 hex characters>)"; `Error: run again with --yes to agree`. Then the same with `--yes`: `Installed Routine demo 0.1.0` |
| 2 | `$E jobs set routine-demo.mail-notes on=on`; `cp -R "$C/next/routine-demo/." "$C/plugins/routine-demo/"`; `$E plugin marketplace update test-plugins`; `$E plugin update routine-demo` | Asks again: the routine's line with another hash after "its prompt:", marked `(new)`; `Error: run again with --yes to agree`. Then with `--yes`: `Updated Routine demo to 0.2.0`; `$E jobs routines`: the routine still `enabled` true, its `on` true — the user's yes to the update is their yes to the changed routine |
| 3 | A broken copy: `B=$(mktemp -d)/b`; `cp -R "$C/plugins/routine-demo" "$B"`; `python3 -c "import json,sys;p=sys.argv[1]+'/echo-plugin.json';m=json.load(open(p));m['routines'][0]['every']=5;json.dump(m,open(p,'w'))" "$B"`; `$E plugin check "$B"` | Exit 1, naming `echo-plugin.json`: `routine “mail-notes”: every is 15 to 1440 minutes` |
| 4 | `python3 -c "import json,sys;p=sys.argv[1]+'/echo-plugin.json';m=json.load(open(p));m['routines'][0]['every']=60;m['uses']={};json.dump(m,open(p,'w'))" "$B"`; `$E plugin check "$B"` | Exit 1: `routine “mail-notes” runs Claude Code: the manifest needs “claude” in uses` |
| 5 | `rm "$B/routines/mail-notes.md"`; `python3 -c "import json,sys;p=sys.argv[1]+'/echo-plugin.json';m=json.load(open(p));m['uses']={'claude':{}};json.dump(m,open(p,'w'))" "$B"`; `$E plugin check "$B"` | Exit 1: `routine “mail-notes”: routines/mail-notes.md isn't in the plugin` |

## Expected Final State
Routine demo 0.2.0 installed, its routine on (kept by the update); the marketplace copy updated.

## Teardown
- `$E plugin remove routine-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C" "$B"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cp`, `mktemp`, `rm` and `python3 -c` (editing the copy's manifest).
- That a routine an update adds starts off (also one whose id an earlier version had, switched on): the same Rust test, below.
- The rules of a routine's manifest and the hash over its definition and prompt: the Rust test `plugins::routines::a_routine_is_checked_named_on_the_sheet_and_its_prompt_change_asks_again`.
