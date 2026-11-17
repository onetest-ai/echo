---
id: TC-113
title: A routine of a plugin that is off doesn't run and keeps its settings; removing the plugin switches it off; a run Echo started can't run a routine or switch one on
priority: high
type: functional
requirements: [F-PLUG-79]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, jobs]
---

# TC-113: A routine of a plugin that is off or gone doesn't run, and a run never starts or switches one

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The test marketplace: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install routine-demo --yes`; `$E jobs set routine-demo.mail-notes on=on every=30`

**Changes:** the installed plugins, `<config dir>/jobs.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin disable routine-demo`; `$E jobs routines`; `$E jobs run routine-demo.mail-notes` | The routine listed with `enabled` false, its `on` true and `every` 30 kept; the run refused: "Routine demo is off: its routines don't run" |
| 2 | `$E plugin enable routine-demo`; `ECHOCTL_HEADLESS=1 $E jobs run routine-demo.mail-notes`; `ECHOCTL_HEADLESS=1 ECHOCTL_RUN_SESSION=acts $E jobs set routine-demo.mail-notes on=off` | Both refused inside a run Echo started — "refused inside a run Echo started: a routine runs only on its schedule or when you run it" and "refused inside a run Echo started: how Echo's own jobs run is the user's to choose"; `$E jobs routines`: the routine `enabled` true, still on, no run |
| 3 | `$E plugin remove routine-demo`; `$E jobs routines` | No routine of `routine-demo`; the seven system routines |
| 4 | `$E plugin install routine-demo --yes`; `$E jobs routines` | The routine back, off (`on` false, `set` false, `enabled` false): its install never switches it on; `every` 30 kept |

## Expected Final State
Routine demo installed, its routine off, every 30 min.

## Teardown
- `$E plugin remove routine-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A routine's settings are the user's, in `<config dir>/jobs.json`; removing the plugin clears only its `on`.
