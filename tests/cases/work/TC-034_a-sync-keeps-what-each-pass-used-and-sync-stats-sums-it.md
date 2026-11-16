---
id: TC-034
title: A thread's sync keeps what each pass used, and sync-stats sums it per day and pass
priority: medium
type: functional
requirements: [F-THR-25]
module: work
run: agent
platform: macos
data: standard
ai: virtual
tags: [work, ledger, sync, virtual-model, usage]
---

# TC-034: A thread's sync keeps what each pass used, and sync-stats sums it per day and pass

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH`. The sync's script: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json` (one sync = an extract, a verify and a people run, as in `work/TC-028`). The model server reports each answer's tokens, so every run's result event carries a usage
- `$L` stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full)
- Besides `echoctl`, this case may run `cat`, `grep` and `python3 -c` in `<dir>`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl project sync-stats --days 1` | `{"days": [], "total": {}}`: no sync has run on this data |
| 2 | Run `$L pending`; `$L done --evidence <id>` for every item but Acme security review (`<the build date − 1>_110000`); then `$L run` | The sync ends: `state` "done", `processed` 1; `usage` holds `extract`, `verify` and `people`, each with `input`, `output`, `cache_read`, `cache_write` and `turns` numbers, `input` and `output` above 0, and `ok` true |
| 3 | Run `grep -E '^[0-9:]+  (extract|verify|people): [0-9]+ tokens' $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/log.txt` (`<run>` from step 2) | Three lines, one per pass, each `<pass>: N tokens (input …, output …, cache read …, cache write …), M turns`, N the sum of the four |
| 4 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/ledger/sync-usage.jsonl` | Three JSON lines with `run` `<run>`, `pass` extract, verify, people, `ok` true, and the same token counts as step 2 |
| 5 | Run `target/debug/echoctl project sync-stats --days 1` | `days` one entry, today: `runs` 3, `tokens` the sum of the three passes; `passes.extract.runs` 1, `passes.verify.runs` 1, `passes.people.runs` 1, each `failed` 0; `total.tokens` the same as the day's |
| 6 | Run `target/debug/echoctl project sync-stats --days 0` | The same as step 5 (at least today) |

## Expected Final State
Acme rollout's sync has its three passes' usage in its run log, its state and `ledger/sync-usage.jsonl`; sync-stats reports them for today.

## Teardown
- `tests/fixtures/serve.sh down <dir>`
