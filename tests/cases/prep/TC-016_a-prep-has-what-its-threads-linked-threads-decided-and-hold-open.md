---
id: TC-016
title: A prep of a thread's meeting lists each linked thread's latest decisions and open items; In short's model gets only those on its own AI profile
priority: medium
type: functional
requirements: [F-PREP-09]
module: prep
run: agent
platform: macos
data: standard
ai: virtual
tags: [prep, links, virtual-model, routing]
---

# TC-016: A prep has "From linked thread"; the model sees only threads on its profile

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, as in `prep/TC-015` (`serve.sh up <dir> <port> --set standard --ai virtual`), its prep script copied and the request log emptied: `cp tests/fixtures/model-scripts/prep-summary.json <dir>/model/model-script.json; : > <dir>/model/model-log.jsonl`
- Acme renewal follow-up = `99067d7f0917f050`, in the thread Acme renewal (default AI profile, `ollama`)

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `target/debug/echoctl project link acme-renewal --to acme-rollout --kind related`, `target/debug/echoctl project link umbrella-proposal --to acme-renewal --kind related`, `target/debug/echoctl project set umbrella-proposal --profile work` | Each exits 0 |
| 2 | `target/debug/echoctl prep --write 99067d7f0917f050` | No `error`; `summary` the script's; `linked` has `acme-rollout` (`profile` "ollama", `decisions` "Go live on a Tuesday" then "The pilot starts with the Berlin office", `open` five items, the late ones first: "Book the workshop room", "Map the user groups", …) and `umbrella-proposal` (`profile` "work") |
| 3 | `python3 -c "import json,os; u=[json.loads(l) for l in open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl')][0]['messages'][0]['content']; print('From the linked thread Acme rollout:' in u, '- decision Go live on a Tuesday' in u, 'Umbrella proposal' in u)"` | `True True False`: Acme rollout's records reached the model (same profile), Umbrella proposal's didn't (profile `work`) |
| 4 | Navigate to `{{base_url}}/#upcoming/99067d7f0917f050` | A section "From linked thread · Acme rollout" (the name opens `#project/acme-rollout/brief`) listing "Decision Go live on a Tuesday", "Decision The pilot starts with the Berlin office", "Task Book the workshop room · Sam Rivera · late", …; a section for Umbrella proposal only if it holds decisions or open items |

## Expected Final State
The prep holds both linked threads; only Acme rollout's records went to the virtual model.

## Notes
- A closed linked thread, and a linked thread that is one of the meeting's own threads, are left out (`prep::linked`).
