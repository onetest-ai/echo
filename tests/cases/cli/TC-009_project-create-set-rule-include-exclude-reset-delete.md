---
id: TC-009
title: project create, rule, set, exclude, include, reset and delete change a thread as the window does
priority: high
type: functional
requirements: [F-CLI-05]
module: cli
run: agent
platform: any
data: standard
tags: [cli, threads, rules]
---

# TC-009: project create, rule, set, exclude, include, reset and delete change a thread as the window does

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `DISC=$(ls "$ECHOCTL_HOME/calls" | grep -- '-duff-discovery$')`, `FU=$(ls "$ECHOCTL_HOME/calls" | grep -- '-duff-follow-up$')`, `COFFEE=$(ls "$ECHOCTL_HOME/calls" | grep -- '-coffee-chat$')` (each call's folder name starts with its date, which follows the day the data is built: look it up, never type a date)
- Reload the page fully (`{{base_url}}/`) after each CLI change before reading the window (app_profile → Fragile areas)

## Test Data

| Field | Value |
|---|---|
| New thread | "Duff pilot" (id `duff-pilot`), client Duff (no threads before), stage lead |
| Duff's calls | Duff follow-up (`$FU`), Duff discovery (`$DISC`) |
| A call pinned by hand | Coffee chat (`$COFFEE`) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl project create "Duff pilot" --client duff --stage lead` | Exit code 0; JSON with `id` "duff-pilot", `client` "duff", `stage` "lead", `group` "pursuit", `call_count` 0, `rules` [] |
| 2 | Run `target/debug/echoctl project rule duff-pilot --people` | Exit code 0; `rules` is `[{"kind":"people"}]`; `calls` lists Duff follow-up and Duff discovery, both `why` "people" |
| 3 | Reload, navigate to `{{base_url}}/#project/duff-pilot/calls` | Under DUFF in the thread list "Duff pilot" with "2 calls" and "Lead"; the Calls tab lists Duff follow-up and Duff discovery, each "the client's people" with "Take out" |
| 4 | Run `target/debug/echoctl project set duff-pilot --stage proposal --reason "they asked for a quote" --name "Duff pilot bid"` | `name` "Duff pilot bid", `id` still "duff-pilot", `stage` "proposal"; the last of `stages` has `stage` "proposal" and `reason` "they asked for a quote" |
| 5 | Run `target/debug/echoctl project exclude duff-pilot $DISC` | `exclusions` is `["<the stamp of $DISC: its first 17 characters, e.g. 2026-10-28_100000>"]`; `calls` holds only Duff follow-up |
| 6 | Run `target/debug/echoctl project include duff-pilot $COFFEE` | `pins` is `["<the stamp of $COFFEE: its first 17 characters>"]`; `calls` lists Duff follow-up (`people`) and Coffee chat (`pinned`) |
| 7 | Reload, navigate to `{{base_url}}/#project/duff-pilot/calls` | The thread reads "Duff pilot bid" with "Proposal"; the Calls tab lists Duff follow-up ("Take out") and Coffee chat ("added by hand", "Unpin"), then "1 call taken out by hand."; Duff discovery is not listed |
| 8 | Run `target/debug/echoctl project reset duff-pilot $DISC`, then `target/debug/echoctl project reset duff-pilot $COFFEE` | After the second: `exclusions` [] and `pins` []; `calls` lists Duff follow-up and Duff discovery again (`people`) |
| 9 | Run `target/debug/echoctl project rule duff-pilot --remove 1` | `rules` [] and `calls` [] |
| 10 | Run `target/debug/echoctl project delete duff-pilot` | Prints "duff-pilot moved to the trash" |
| 11 | Run `target/debug/echoctl project show duff-pilot` | Exit code 1; "Error: no project \"duff-pilot\"" |
| 12 | Reload, navigate to `{{base_url}}/#projects` | No "Duff pilot bid" in the thread list; DUFF shows no thread |

## Expected Final State
The thread was made, changed and deleted from the CLI; the window showed each state after a reload. Duff has no thread again.

## Teardown
- None: step 10 deleted the thread.

## Notes
- The id is kept on a rename (`set --name`): links and pins keep working.
