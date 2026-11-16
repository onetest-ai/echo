---
id: TC-033
title: A mail with work, and an automatic reply naming who covers, go to the extract run on the thread's profile — never set aside
priority: high
type: functional
requirements: [F-THR-28, F-THR-20]
module: threads
run: agent
platform: macos
data: standard
ai: virtual
tags: [threads, sync, mail, noise, virtual-model]
---

# TC-033: A mail with work, and an automatic reply naming who covers, are read, not set aside

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- As `threads/TC-032` (standard data with the virtual model, its sync script, an empty request log, `$L`, `$E`, `mail`); on its own: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`
- Acme rollout has no AI profile of its own: it syncs on the default, `ollama` ("Virtual model", model `virtual`)
- Besides `echoctl`, this case may run `python3 -c`, `cat`, `grep` and `echo` in `<dir>`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$L pending`; `$L done --evidence <id>` for each item listed; then `$L run` | `$L pending` prints `[]`; the sync ends `state` "done" |
| 2 | `mail mail-acme-ask 'Acme rollout plan' 'Can you send the rollout plan by Friday?'`; `mail mail-acme-cover 'Automatic reply: Acme rollout plan' 'Away this week; for the rollout write to omar@acme.example.'`; run `$L run` | `state` "done" with a `run` id; `set_aside` `[]`; no "set aside" in `message` |
| 3 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/log.txt \| head -1` (`<run>` from step 2) | "HH:MM:SS  Echo syncs Acme rollout (acme-rollout) on Virtual model, model virtual": the thread's profile |
| 4 | `python3 -c "import json; L=[json.loads(l) for l in open('<dir>/model/model-log.jsonl')]; e=[x for x in L if 'sync — extract' in json.dumps(x.get('system'), ensure_ascii=False)]; m=json.dumps(e[-1]['messages'], ensure_ascii=False); print(e[-1]['model'], 'Can you send the rollout plan by Friday?' in m, 'omar@acme.example' in m, 'sync — filter' in json.dumps(L, ensure_ascii=False))"` | `virtual True True False`: both mails reached the extract run on the profile's model; no other request judged them |

## Expected Final State
Mail with work, and an automatic reply that names who to write to instead, were read by the extract run on the thread's own profile.

## Teardown
- `tests/fixtures/serve.sh down <dir>`
