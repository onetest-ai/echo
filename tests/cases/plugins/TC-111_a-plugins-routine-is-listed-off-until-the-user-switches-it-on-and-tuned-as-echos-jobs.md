---
id: TC-111
title: A plugin's routine is listed with Echo's jobs in the routines' one shape, off until the user switches it on (Settings → Plugins or the CLI), tuned as Echo's jobs are; a plugin with Echo's jobs tunes it but never switches it
priority: high
type: functional
requirements: [F-PLUG-76]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, jobs, settings]
---

# TC-111: A plugin's routine is off until the user switches it on, and tuned as Echo's jobs

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The test marketplace: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install routine-demo --yes`; `$E plugin install jobs-demo --yes`
- The served window's cookie for the IPC steps: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** the installed plugins, `<config dir>/jobs.json`

## Test Data

| Plugin | Routine demo (`routine-demo`): its routine `mail-notes` "Mail → notes", every 60 min. Jobs demo (`jobs-demo`): `uses.echo.jobs` |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E jobs routines` | The seven system routines, then `routine-demo.mail-notes`: `source` "plugin:routine-demo", `name` "Mail → notes", its `what`, `enabled` false, `to` [], `schedule` {"text": "Every 60 min", "kind": "timer"}, `next_run` null, `last` null, `model` {"value": "", "default": "the profile's minutes model"}; its `settings`: `on` (kind "switch") false, `every` 60 (range 15–1440), `night` true, `model` "", each `set` false |
| 2 | Navigate to `{{base_url}}/#settings/plugins`; click the row "Routine demo" | Its detail has "Its routines": "Mail → notes", its what, "Every 60 min · paused at night · off", and a switch "Mail → notes on", not checked |
| 3 | Click the switch "Mail → notes on"; wait 2 s | The switch is checked; the line "Every 60 min · paused at night" (no "off"). `$E jobs routines`: the routine `enabled` true, its `on` true and `set` true, `next_run` a time (now, or the night's end when it is night) |
| 4 | `$E jobs set routine-demo.mail-notes every=30 night=off model=sonnet` | The routine as now: `schedule.text` "Every 30 min", `every` 30, `night` false, `model.value` "sonnet", each `set` true |
| 5 | `$E jobs set routine-demo.mail-notes every=5`; `$E jobs set routine-demo.mail-notes on=maybe` | Each refused: "routine-demo.mail-notes every: 5 is outside 15–1440"; "routine-demo.mail-notes on: on or off, not “maybe”"; the routine still every 30, on |
| 6 | Jobs demo's window half tuning it, as its page sends it on a click: `curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data '{"cmd":"plugin_jobs_set","id":"jobs-demo","job":"routine-demo.mail-notes","settings":["every=45"],"req":"r1"}' {{base_url}}/ipc` | The `reply` (`window.echo.jobsSet(…)`, its JSON quoted) answers `req` r1 with the routine, its schedule "Every 45 min" |
| 7 | The same with `"settings":["on=off"],"req":"r2"` | The `reply` answers `req` r2 with the error "a plugin can't switch a routine on or off: you do, in Settings → Plugins or with echoctl jobs set"; `$E jobs routines`: the routine still on, every 45 |
| 8 | Reload `{{base_url}}/#settings/plugins`; click the row "Routine demo"; click the switch "Mail → notes on"; wait 2 s | The switch is not checked; the line "Every 45 min · off". `$E jobs set routine-demo.mail-notes on=on`, then `on=`: on, then off again (`on` `set` false) |

## Expected Final State
Routine demo's routine off, every 45 min, not paused at night, on sonnet; Echo's own jobs unchanged.

## Teardown
- `$E plugin remove routine-demo --delete-data`; `$E plugin remove jobs-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -f "$D/jar"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `curl` (the served window's own IPC, as Jobs demo's window half sends it on a click) and `rm`.
- The routine's settings, its `on` refused from a plugin and its place in the one shape: the Rust test `jobs::a_plugins_routine_reads_in_the_one_shape_off_until_the_user_switches_it_on_and_a_plugin_never_does`.
