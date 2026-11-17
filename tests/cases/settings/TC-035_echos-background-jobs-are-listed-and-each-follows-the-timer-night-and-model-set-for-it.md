---
id: TC-035
title: Echo's background jobs are listed with their defaults, and each follows the timer, night pause and model set for it
priority: high
type: functional
requirements: [F-SET-23]
module: settings
run: agent
platform: macos
data: standard
tags: [jobs, settings, models]
---

# TC-035: Echo's background jobs are listed and each follows what is set for it

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `python3 tests/fixtures/make_data.py <dir> --set standard --ai virtual` (an empty `<dir>`); every `echoctl` command is `$E …` (`E=target/debug/echoctl`) after `. <dir>/env`; `$L` is `$E project ledger acme-rollout`
- The thread sync's script, told `virtual-sonnet` is a model it has: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json`; `python3 -c "import json;p='<dir>/model/model-script.json';d=json.load(open(p));d.setdefault('model',{})['names']=['virtual','virtual-sonnet'];json.dump(d,open(p,'w'))"`
- **A made-up mail item**, as threads/TC-031 writes one: its `mail()` function, with `E=$ECHOCTL_HOME/projects/acme-rollout/ledger/evidence` for its folder
- Run before 22:30 (the thread sync pauses in the night by default)

**Changes:** `<config dir>/jobs.json`, `<config dir>/jobs-state.json`, the thread acme-rollout's ledger

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `$E jobs` | `night` `{"from": "22:30", "to": "07:00"}`; seven jobs in this order, each with its fields' `value` its `default` and `set` false: `needs` (every 5, night true, model ""), `thread_sync` (batch_wait 120, batch_items 5, night true, model ""), `fetch` (every 30, night true), `minutes` (delay 15, model ""), `prep` (every 5, night true, model ""), `brain_reader` (every 5, per_pass 30, night true, model ""), `live_notes` (every_s 90, model ""); `runs_today` 0 |
| 2 | Run `$E jobs set needs every=10 night=off model=haiku` | The list: `needs` every 10, night false, model "haiku", each `set` true |
| 3 | Run, one at a time: `$E jobs set needs every=0`; `$E jobs set needs "model=two words"`; `$E jobs set needs batch_wait=5`; `$E jobs set nope every=5`; `$E jobs set night from=25:00`; `$E jobs set needs every=20 every=0`; `ECHOCTL_HEADLESS=1 $E jobs set needs every=20` | Each refused, saying why ("outside 1–240", "isn't a model name", "needs has every, night, model", "no job “nope”", "isn't a time", "outside", "refused inside a run Echo started"); `$E jobs`: `needs` still every 10 |
| 4 | Run `$E jobs set needs every= night= model=` | `needs` back to every 5, night true, model "", `set` false |
| 5 | Run `$E jobs set night from=23:00 to=06:00`, then `$E jobs set night from= to=` | First `night` `{"from": "23:00", "to": "06:00"}`, then 22:30 and 07:00 again |
| 6 | Run `$L pending` and `$L done --evidence <id>` for each item listed; `$L run`. Run `$E jobs set thread_sync batch_items=2 batch_wait=45 model=virtual-sonnet`; `mail mail-a1`; `$L run --scheduled` | "1 mail or chat item waits: read at HH:MM or once 2 wait", HH:MM 45 minutes from now; no `run` |
| 7 | `mail mail-a2`; `: > <dir>/model/model-log.jsonl`; `$L run --scheduled` | Two wait: the sync goes to the model (a `run` id); every request in `<dir>/model/model-log.jsonl` asks for `virtual-sonnet`; `$E jobs`: `thread_sync` `runs_today` 1 and a `last_run` |

## Expected Final State
The thread sync job batches 2 items at most 45 minutes on `virtual-sonnet`; the others on their defaults.

## Teardown
- `rm -rf <dir>`

## Notes
- That Needs you now, minutes, prep (with meeting types and task modes), the Recall reader and live notes follow their timer, night pause and model is the Rust tests on each (`needs::…the_checks_run_on_the_profiles_live_model`, `notes::…a_debug_build_takes_the_test_delay…`, `projectsync::…mail_and_chat_are_due…`, `jobs::tests`); their schedulers run only in the Echo app, which never runs on test data.
