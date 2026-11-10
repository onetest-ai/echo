---
id: TC-028
title: Sync now on a thread of the virtual model runs extract, verify and people under the guard, reads its new call into the work with its words, verifies it in a fresh run, and keeps the fields the user set
priority: critical
type: functional
requirements: [F-THR-20, F-WORK-16, F-CC-04]
module: work
run: agent
platform: macos
data: standard
ai: virtual
tags: [work, ledger, sync, virtual-model, claude-code, guard]
---

# TC-028: Sync now on a thread of the virtual model reads its new call into the work, verifies it, and keeps the fields the user set

**Module:** Work | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual` (`tests/fixtures/README.md`, "The virtual model"); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH`. The default AI profile is `ollama` ("Virtual model", model `virtual`); Acme rollout has no profile of its own, so it syncs on it. The real Claude Code, its tools, Echo's guard and the ledger run; only the model is `tests/fixtures/model_server.py`. No model server or Claude account is used
- The sync's script: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json` and an empty request log (`: > <dir>/model/model-log.jsonl`). One sync = three Claude Code runs in a row, each answered by its own rule (by the prompt's "Mode: extract.", "Mode: verify.", "Mode: people."):
  - **extract** (rule 0): `$L pending`, `$L context`, then `echoctl project ledger globex-pilot context` (another thread's: the guard must refuse it), Read the item's `.md`, Write `<run folder>/<evidence id>.json` with four proposals, `$L apply`, `$L done`. The proposals, each with its words from Acme security review: create a risk "Second-phase budget not approved yet" (under E01), create a task "Nominate two help desk observers" (under S001, a dependency the client owes, Omar Haddad); T008 `due` "November 27" ("Tom plans the dry run for the last week of the month…"); T003 `reassign` to "Priya", handed over by Me ("I will ask Priya to prepare a short training deck…")
  - **verify** (rule 1): `$L verify-prep --out <run folder>/packets.json`, Read it, Write `verdicts.json` judging every packet `supports` (the mids read from the packets), `$L verify-apply`
  - **people** (rule 2): `$L people-context --items '<first evidence id>'`, Write `<id>_people.json`: Priya Nair's role "help desk training" and a fact about her, and a role "dry run owner" for Tom Becker, whose role the user set; `$L people-apply`
- `$L` stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full). A node from the tree: `$L tree` → `nodes.<id>` (`due`, `internal_owner`, `delegated_by`, `pinned`, `history`)
- The user's own edits pin a field (`pinned`); the extractor is told never to change a pinned field, and a proposal that still tries is recorded in the item's history, not applied (the window marks it "kept yours", as in `calls/TC-020`). The script tries it on both fields on purpose
- Besides `echoctl`, this case may run `ls`, `cat`, `grep` and `python3 -c` (reading the request log and the tree) in `<dir>`

## Test Data

| Item / evidence | Before the sync | After |
|---|---|---|
| T008 Run the data migration dry run | the user sets `due` 2026-11-20 (step 1) | `due` 2026-11-20, `pinned` holds "due"; the agent's "due November 27" in its history, not applied ("kept yours") |
| T003 Prepare the training deck | the user sets `internal_owner` "Alex Chen" (step 1) | still "Alex Chen", `pinned` holds "internal_owner", `delegated_by` as before; the agent's reassign to Priya in its history, not applied |
| Evidence pending | Globex pilot scoping (`<the build date − 10>_110000`), Acme security review (`<the build date − 1>_110000`), Memo (`<the build date>_…`); the evidence ids follow the build day | the first and last marked done by the user (step 2), so only Acme security review is read |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$L edit '{"op":"edit","node":"T008","set":{"due":"2026-11-20"}}'` and `$L edit '{"op":"edit","node":"T003","set":{"internal_owner":"Alex Chen"}}'` | Each prints JSON with `type` "human", `by` "human", `op` "edit"; in `$L tree`, T008 `due` "2026-11-20" with "due" in `pinned`, T003 `internal_owner` "Alex Chen" with "internal_owner" in `pinned` |
| 2 | Run `$L pending`; then `$L done --evidence <the build date − 10>_110000` (Globex pilot scoping's id from pending) and `$L done --evidence <the Memo's id from pending>`; run `$L pending` again | First: three items (Globex pilot scoping, Acme security review, Memo); each `done` prints `{"done": "<id>"}`; then only `<the build date − 1>_110000` "Acme security review" is left (`<evid>` below). Save `$L tree` to `<dir>/tree-before.json`; note T003's `delegated_by` and the highest R and T ids |
| 3 | Navigate to `{{base_url}}/#project/acme-rollout/work`, wait for "Sync now", note the time and click the "Sync now" link (`[data-pj="sync"]` in the work bar) | The sync starts (the bar may show "Reading 1 call…" for a moment: the whole sync takes a few seconds) |
| 4 | Run `$L status` until `state` is no longer "running" (every 5 seconds, up to a minute) | `state` "done", `processed` 1, `left` 0, `run` a run id (`<run>`); `message` the three reports: "Extract (virtual model): 1 item read; …", "Verify (virtual model): 3 supports.", "People (virtual model): proposed Priya Nair's role, …" |
| 5 | Run `$L pending` and `ls $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/` | `[]`; the run folder holds `log.txt`, `<evid>.json`, `packets.json`, `verdicts.json`, `<evid>_people.json` |
| 6 | Run `$L tree` and compare with `<dir>/tree-before.json` | Two new nodes, the next ids: a risk "Second-phase budget not approved yet" under E01 and a task "Nominate two help desk observers" under S001 (`client_owed` true, `client_owner` "Omar Haddad"), each created by the agent with its `quote` and `evidence` `<evid>`; T005's open claim from before ("waiting on Omar Haddad: the access list") now has `verdict` "supports" |
| 7 | In `$L tree` read T008 and T003 | T008 `due` "2026-11-20", "due" still in `pinned`; its last history entry is the agent's `status` `due` "2026-11-27" with `verdict` "supports" and `applied` false. T003 `internal_owner` "Alex Chen", "internal_owner" still in `pinned`, `delegated_by` as noted in step 2; its agent entry `reassign` (Priya, `verdict` "supports") has `applied` false |
| 8 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/log.txt` | "Echo syncs Acme rollout (acme-rollout) on Virtual model, model virtual"; three "Claude Code started · model virtual"; every `→ Bash` is `echoctl project ledger acme-rollout …` but one: `echoctl project ledger globex-pilot context`, followed by "✗ … Echo's project sync: only `echoctl project ledger acme-rollout <pending\|context\|apply\|done\|verify-prep\|verify-apply\|people-context\|people-apply>` runs here" and, after the extract's result, "Not allowed: Bash {…globex-pilot context…}"; no other "✗"; every `→ Write` is inside the run folder, every `→ Read` inside the thread's `ledger/`; three "Finished: ok" |
| 9 | Run `python3 -c "import json,os; L=[json.loads(l) for l in open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl')]; T=lambda c: c if isinstance(c,str) else ' '.join(b.get('text','') for b in c); R=lambda e: [b.get('is_error') or False for b in (e['messages'][-1]['content'] if isinstance(e['messages'][-1]['content'],list) else []) if b.get('type')=='tool_result']; print([len([e for e in L if e.get('rule')==r]) for r in (0,1,2)], sorted({e['model'] for e in L if e.get('rule') is not None}), [sorted(set(e['tools'])-{'DeferredToolPlaceholder'}) for e in L if e.get('rule') is not None and e['step']==0]); f={r:T(e['messages'][0]['content']) for e in L if e.get('step')==0 for r in [e.get('rule')]}; print(['/echo:project-sync' in f[0], 'Mode: extract. Evidence ids, strictly in this order:' in f[0], 'nobody is watching it' in f[0], '/ledger/runs/' in f[0], 'Mode: verify.' in f[1], 'Mode: people. Evidence ids:' in f[2]]); v=[e for e in L if e.get('rule')==1]; print(any('Nominate two help desk observers' in json.dumps(e['messages']) for e in v)); print([x for e in L if e.get('rule')==0 for x in R(e)])"` | `[8, 5, 4] ['virtual'] [['Bash', 'Edit', 'Glob', 'Grep', 'Read', 'Skill', 'Write'], …]` (three runs, each offered the sync's tools only: no web, no agents, no MCP); `[True, True, True, True, True, True]` (each run's prompt names the skill, its mode, the evidence and its run folder, and that nobody is watching); `False` (the verifier never saw the extraction's proposals: a fresh process); `[False, False, True, False, False, False, False]` (the model was told only the other thread's `context` was refused) |
| 10 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/people-suggestions.json` and find Priya Nair in `$ECHOCTL_HOME/projects/people.json` | A pending suggestion: Priya Nair's `role` "help desk training", with its quote; Priya Nair has the fact "Prepares the help desk training deck and its FAQ page for the Acme rollout." (evidence `<evid>`); no suggestion for Tom Becker: the role the user set (`people-apply` answered `"added": 1`, `"facts": 1` and rejected it, "the user set this role: leave it", in the run's last tool result in the request log) |
| 11 | Reload `{{base_url}}/#project/acme-rollout/work` | The bar reads "Synced <time> · N to review"; "Sync now" enabled. Click the card T008: Details "Due" "Fri, Nov 20", History "due November 27" marked "kept yours" above "you set due" with "undo". Click T003: "Owner" "Alex Chen", "Handed over by" as before the sync, History the agent's "reassign" marked "kept yours" above "you set internal owner" |

## Expected Final State
Acme security review is read into Acme rollout's work through the real Claude Code and its guard (two new items with their words, every claim verified), nothing pending; T008's date and T003's owner are still the user's.

## Teardown
- None (the work keeps what the sync made; later stages must not expect Acme rollout's original tree)

## Notes
- Known issue #292: steps 7 and 11 fail for T003 until it is fixed. A reassign whose owner the user set leaves the owner, but changes Handed over by and is recorded `applied: true` ("reassign · Me · verified" instead of "kept yours"). The case asserts the right behaviour.
- The script skips the skill's dependency pass (a model may find no dependency); its proposals are fixed, so every run checks the same guard refusal, the same pinned fields and the same verdicts.
- The real-model counterpart (2026-11-07): 4 min 36 s on the network test server, 45 min on this Mac's Ollama; the model skipped the Priya reassignment on its own, so "kept yours" was never exercised there. Outside the ledger only Echo's CLI changes files, as designed: the people pass's `people-apply` writes `projects/people.json` and `projects/acme-rollout/people-suggestions.json`; `ledger/mails.json` records "couldn't read your organisation's domains: mail waits" (no Microsoft 365 on test data).
- The automatic timing (a thread syncs by itself 15 minutes after its call) is the app's scheduler: not covered here.
