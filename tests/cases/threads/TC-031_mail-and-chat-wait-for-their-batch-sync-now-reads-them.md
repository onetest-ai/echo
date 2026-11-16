---
id: TC-031
title: Mail and chat wait for their batch on a scheduled sync — two hours or five items — and Sync now reads them at once
priority: high
type: functional
requirements: [F-THR-26, F-THR-20]
module: threads
run: agent
platform: macos
data: standard
ai: virtual
tags: [threads, sync, mail, batch, virtual-model]
---

# TC-031: Mail and chat wait for their batch on a scheduled sync, and Sync now reads them at once

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH`; the sync's script: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json`
- `$L` stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full); `$E` for `$ECHOCTL_HOME/projects/acme-rollout/ledger/evidence`
- **A made-up mail item**, written as the mail sync writes one (no mailbox is used): `mail(){ python3 -c "import json,sys,datetime as d; t=(d.datetime.now().astimezone()-d.timedelta(days=1)).isoformat(); json.dump({'id':sys.argv[1],'at':t,'date':t[:10],'title':'Acme rollout status'},open(sys.argv[2]+'/'+sys.argv[1]+'.json','w'))" "$1" "$E"; echo 'Test mail, nothing real.' > "$E/$1.md"; }`
- Besides `echoctl`, this case may run `python3 -c`, `cat` and `echo` in `<dir>`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$L pending`; `$L done --evidence <id>` for each item listed; then `$L run` (Sync now: the verifier and who's who catch up on what the data holds) | `$L pending` prints `[]`; the sync ends `state` "done" |
| 2 | `mail mail-acme-1`; run `$L run --scheduled` | `state` "done", `processed` 0, `message` "1 mail or chat item waits: read at HH:MM or once 5 wait" (HH:MM two hours from now); no run folder made (`run` absent); `$L pending` still lists `mail-acme-1` |
| 3 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/ledger/waiting.json` | `{"mail-acme-1": "<now>"}`: when it was first seen waiting, not the mail's own time a day ago |
| 4 | Run `$L run --scheduled` again | The same message with the same HH:MM: it still waits from step 2 |
| 5 | `mail mail-acme-2`, `mail mail-acme-3`, `mail mail-acme-4`; run `$L run --scheduled` | "4 mail or chat items wait: read at HH:MM or once 5 wait", the HH:MM of step 2 |
| 6 | `mail mail-acme-5`; run `$L run --scheduled` | Five wait: the sync goes to the model: `state` "done" and a `run` id (the virtual model's script reads its own evidence; what it makes of these items doesn't matter here) |
| 7 | `mail mail-acme-6`; run `$L run` (Sync now, no `--scheduled`) | It doesn't wait: `state` "done" with a `run` id, no "wait" in `message` |

## Expected Final State
A lone new mail waited on the scheduled sync, from when it was first seen; five went to the model; Sync now read a new one at once.

## Teardown
- `tests/fixtures/serve.sh down <dir>`

## Notes
- The scheduler itself runs only in the Echo app, which never runs on test data; `run --scheduled` is the same sync it starts.
