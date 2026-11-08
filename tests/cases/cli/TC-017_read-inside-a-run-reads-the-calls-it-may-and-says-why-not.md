---
id: TC-017
title: Inside a run Echo started, echoctl read gives Yolo the calls of its task's thread, a minutes run its own call, and a refusal says why instead of "no call"
priority: medium
type: functional
requirements: [F-CC-11]
module: cli
run: agent
platform: any
data: standard
tags: [cli, links, read, runs, yolo, guard]
---

# TC-017: Inside a run Echo started, echoctl read gives Yolo the calls of its task's thread, a minutes run its own call, and a refusal says why instead of "no call"

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `G=$(ls "$ECHOCTL_HOME/calls" | grep -- '-globex-pilot-scoping$' | tail -1)` (a call of Globex pilot) and `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | head -1)` (an Acme call, not in Globex pilot)
- A run is imitated with the variables Echo sets for it: `ECHOCTL_HEADLESS=1` in every run; `ECHOCTL_RUN_TASK=globex-pilot/T005` for Yolo on "Book the training slots"; `ECHOCTL_RUN_CALL=<the call's folder>` for a minutes run. No model runs

## Test Data

| Call | Threads | AI profile |
|---|---|---|
| `$G` Globex pilot scoping | Globex pilot (and Acme rollout) | the default (neither the call nor Globex pilot pins one) |
| `$A` the older Acme weekly sync | Acme rollout | the default |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl read "echo://call/$G" \| head -1` (the user) | "# Call: Globex pilot scoping — <its date> 11:00" |
| 2 | Run `ECHOCTL_HEADLESS=1 ECHOCTL_RUN_TASK=globex-pilot/T005 target/debug/echoctl read "echo://call/$G" \| head -1` (Yolo, a call of its thread) | Exit code 0; the same first line |
| 3 | Run `ECHOCTL_HEADLESS=1 ECHOCTL_RUN_TASK=globex-pilot/T005 target/debug/echoctl read "echo://call/$A"` (Yolo, a call of another thread) | Exit code 1; `Error: Yolo reads only the calls of its task's thread (Globex pilot), not "<$A>"` |
| 4 | Run `ECHOCTL_HEADLESS=1 ECHOCTL_RUN_CALL="$ECHOCTL_HOME/calls/$A" target/debug/echoctl read "echo://call/$A" \| head -1` and the same with `"echo://call/$G"` (a minutes run of `$A`) | Its own call: exit 0, "# Call: Acme weekly sync — …"; the other: exit 1, `Error: a run Echo started reads only its own call, not "<$G>"` |
| 5 | Run `ECHOCTL_HEADLESS=1 target/debug/echoctl read "echo://call/$G"` (a run with no call and no task) and `ECHOCTL_HEADLESS=1 ECHOCTL_RUN_TASK=globex-pilot/T005 target/debug/echoctl read echo://call/2026-01-01_000000-no-such-call` | Exit 1 each: `Error: a run Echo started reads only its own call, not "<$G>"`; then `Error: no call "2026-01-01_000000-no-such-call"` (only a call that isn't there says "no call") |

## Expected Final State
Read only: nothing in the data folder changed.

## Notes
- A Yolo run reads only a call pinned to the profile it runs on (its thread's, else the default): a call pinned to another profile is refused with "… is pinned to another AI profile than this run's: its text isn't sent here". The standard set pins no call to another profile, so that refusal is covered by the Rust test `links::tests::a_run_reads_the_calls_it_may_and_a_refusal_says_why`, not here.
- A thread's sync run can't run `read` at all: its guard allows only `echoctl project ledger <id> <step>` (`guard::PROJECT_STEPS`), and it reads its calls from the ledger's evidence.
