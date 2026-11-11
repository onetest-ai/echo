---
id: TC-031
title: A call in two threads tells each thread's sync which other thread it is in, with the thread's brief and terms to judge by, and the verifier's packets say so too
priority: high
type: functional
requirements: [F-WORK-17]
module: work
run: agent
platform: macos
data: standard
tags: [work, ledger, sync, threads]
---

# TC-031: A call shared by two threads

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- In the standard set, **Globex pilot scoping** is in Globex pilot (by its people) and pinned to Acme rollout; both threads are on the default AI profile. `$A` = `target/debug/echoctl project ledger acme-rollout`, `$G` = `target/debug/echoctl project ledger globex-pilot`

**Changes:** the two threads' ledgers (evidence read in; a question and its answer applied in step 4)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$G sync`; `$A sync`; then `$A pending` | In `$A pending`, the entry titled "Globex pilot scoping" has `also_in` `{"threads": ["Globex pilot"], "others": 0}`; an Acme-only call (e.g. "Acme security review") has no `also_in` |
| 2 | `echo "Acme rollout: help desk training and the data migration." > <dir>/brief.md`; `target/debug/echoctl project set acme-rollout --brief <dir>/brief.md`; then `$A context` | `brief` is that text; `terms` lists Acme rollout's glossary terms (`[]` in the standard set) |
| 3 | `grep -n "several threads" plugin/skills/project-sync/EXTRACT.md`; `grep -n "One thread at a time" plugin/skills/project-sync/VERIFY.md` | Each finds its rule: the extractor takes only this thread's part of an item with `also_in`; the verifier judges a quote about another thread's work `wrong` |
| 4 | Write `<dir>/q.json` = `[{"op": "create", "ref": "q1", "kind": "question", "parent": "E01", "title": "Where the pilot starts", "quote": "Our security team wants to review the questionnaire answers before anything goes live"}, {"op": "answer", "node": "q1", "text": "Me: the Berlin office first, wider after two weeks.", "quote": "Then we keep the pilot to the Berlin office first and widen it after two weeks"}]`; `$A apply --evidence <Globex pilot scoping's id> --proposals <dir>/q.json`; `$A verify-prep --out <dir>/packets.json`; read `<dir>/packets.json` | `rejected` is empty; the packet from Globex pilot scoping (the new question's answer) has `thread` "Acme rollout" and `also_in` `{"threads": ["Globex pilot"], "others": 0}`; a packet from an Acme-only call has neither |

## Expected Final State
Acme rollout's sync is told, for the call it shares, which other thread it is in and what Acme rollout is about; nothing else changed but the one proposal.

## Teardown
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-WORK-17), not from the code. The model's judgment itself isn't run here (no real model in regression): the case checks what the sync is given. A thread on another AI profile is counted, never named — the Rust test `ledger::an_item_in_several_threads_is_read_for_this_thread_only`.
