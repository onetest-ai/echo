---
id: TC-028
title: Sync now on a thread of the virtual model reads its new call into the work and who's who in one answer given the call's text, fixes what Echo rejected in one more, verifies it in a fresh run — none with tools — and keeps the fields the user set
priority: critical
type: functional
requirements: [F-THR-20, F-THR-27, F-WORK-16, F-CC-04]
module: work
run: agent
platform: macos
data: standard
ai: virtual
tags: [work, ledger, sync, virtual-model, claude-code]
auto: [tests/auto/specs/S-33.spec.mjs]
---

# TC-028: Sync now on a thread of the virtual model reads its new call into the work and who's who in one answer, fixes what Echo rejected in one more, verifies it in a fresh run, and keeps the fields the user set

**Module:** Work | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual` (`tests/fixtures/README.md`, "The virtual model"); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH`. The default AI profile is `ollama` ("Virtual model", model `virtual`); Acme rollout has no profile of its own, so it syncs on it. The real Claude Code and the ledger run; only the model is `tests/fixtures/model_server.py`. No model server or Claude account is used
- The sync's script: `cp tests/fixtures/model-scripts/thread-sync.json <dir>/model/model-script.json` and an empty request log (`: > <dir>/model/model-log.jsonl`). One sync = three Claude Code runs in a row, none with tools, each answered by its own rule (by its system prompt's "# Echo: a thread's sync — extract" or "— verify", and the fix run's message "## What Echo rejected"):
  - **extract** (rule 1, one request): its answer is one JSON object, `{"items": [{"id": "<evidence id>", "work": […], "note": "…", "people": […]}], "report": "…"}`; Echo applies it. The work proposals, each with its words from Acme security review: create a risk "Second-phase budget not approved yet" (under E01), create a task "Nominate two help desk observers" (under S001, a dependency the client owes, Omar Haddad); T008 `due` "November 27" ("Tom plans the dry run for the last week of the month…"); T003 `reassign` to "Priya", handed over by Me ("I will ask Priya to prepare a short training deck…"). The people proposals: Priya Nair's role "help desk training" and a fact about her, and a role "dry run owner" for Tom Becker, whose role the user set (rejected by Echo)
  - **fix** (rule 0, one request): given the first message, the answer, the rejection and the ids its refs got; it answers `{"items": [], "report": "Tom Becker's role is the one the user set: left as it is."}`
  - **verify** (rule 2, one request): its answer is the JSON array of verdicts, every packet `supports` (the mids read from the packets in its message); Echo applies them
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
| 4 | Run `$L status` until `state` is no longer "running" (every 5 seconds, up to a minute) | `state` "done", `processed` 1, `left` 0, `run` a run id (`<run>`); `message` exactly Echo's extract line, the two scripted reports and Echo's verify line: "Extract: 1 item read, 4 changes to the work, 2 about people. 1 rejection left unfixed.\nExtract (virtual model): 1 item read; created a risk and a client-owed task, proposed T008's date and T003's owner; proposed Priya Nair's role, added a fact about her; Tom Becker's role is the user's.\nTom Becker's role is the one the user set: left as it is.\nVerify: 3 supports."; `usage` holds `extract` and `verify` only (`extract` `turns` 2: both its runs) |
| 5 | Run `$L pending` and `ls $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/` | `[]`; the run folder holds `log.txt`, `<evid>.json`, `<evid>_people.json` (the answer's proposals, as Echo applied them), `packets.json`, `verdicts.json` and `people-applied.txt` (`<evid>`: the item whose people Echo applied, so it is marked read for people) |
| 6 | Run `$L tree` and compare with `<dir>/tree-before.json` | Two new nodes, the next ids: a risk "Second-phase budget not approved yet" under E01 and a task "Nominate two help desk observers" under S001 (`client_owed` true, `client_owner` "Omar Haddad"), each created by the agent with its `quote` and `evidence` `<evid>`; T005's open claim from before ("waiting on Omar Haddad: the access list") now has `verdict` "supports" |
| 7 | In `$L tree` read T008 and T003 | T008 `due` "2026-11-20", "due" still in `pinned`; its last history entry is the agent's `status` `due` "<the build year>-11-27" (`ledger::resolve_due` reads "November 27" in the evidence date's year) with `verdict` "supports" and `applied` false. T003 `internal_owner` "Alex Chen", "internal_owner" still in `pinned`, `delegated_by` as noted in step 2; its agent entry `reassign` (Priya, `verdict` "supports") has `applied` false |
| 8 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/log.txt` | "Echo syncs Acme rollout (acme-rollout) on Virtual model, model virtual"; three "Claude Code started · model virtual" and three "Finished: ok"; no `→ Bash`, `→ Write` or `→ Read` line, no "✗" and no "Not allowed"; two `extract: … tokens` lines and a `verify: … tokens` line, no `people:` |
| 9 | Run `python3 -c "import json,os; L=[json.loads(l) for l in open(os.environ['ECHOCTL_HOME']+'/../model/model-log.jsonl')]; T=lambda c: c if isinstance(c,str) else ' '.join(b.get('text','') for b in c); S=lambda e: json.dumps(e['system'],ensure_ascii=False); print([len([e for e in L if e.get('rule')==r]) for r in (1,0,2)], sorted({e['model'] for e in L if e.get('rule') is not None}), sorted({t for e in L if e.get('rule') is not None for t in e['tools']}-{'DeferredToolPlaceholder'})); F={e.get('rule'):e for e in L}; f={r:T(e['messages'][0]['content']) for r,e in F.items() if r is not None}; print(['Pass 1 — the tree moves' in S(F[1]), 'You have no tools' in S(F[1]), 'Acme' in S(F[1]), 'strictly in this order:' in f[1], 'the budget for the second phase is not approved yet' in f[1], '/ledger/runs/' in f[1], f[0].startswith(f[1]), 'the user set this role: leave it' in f[0], 'One thread at a time' in S(F[2])]); print('Nominate two help desk observers' in json.dumps(F[2]['messages']))"` | `[1, 1, 1] ['virtual'] []` (the extract, its fix run and the verifier, one request each, no tools at all — no Bash, Write, Read, skill, web, agents or MCP); `[True, True, False, True, True, False, True, True, True]` (the extract's system prompt holds EXTRACT and PEOPLE, says it has no tools and nothing of the thread's; its message names the item and holds the call's words, no run folder; the fix run's message is the extract's first, then the answer and Tom Becker's rejection; the verifier's holds VERIFY); `False` (the verifier never saw the extraction's proposals: a fresh process) |
| 10 | Run `cat $ECHOCTL_HOME/projects/acme-rollout/people-suggestions.json` and find Priya Nair in `$ECHOCTL_HOME/projects/people.json` | A pending suggestion: Priya Nair's `role` "help desk training", with its quote; Priya Nair has the fact "Prepares the help desk training deck and its FAQ page for the Acme rollout." (evidence `<evid>`); no suggestion for Tom Becker: the role the user set (rejected, "the user set this role: leave it", given to the fix run) |
| 11 | Reload `{{base_url}}/#project/acme-rollout/work` | The bar reads "Synced <time> · 3 to review"; "Sync now" enabled. Click the card T008: Details "Due" "Fri, Nov 20", History "due November 27" marked "kept yours" above "you set due" with "undo". Click T003: "Owner" "Alex Chen", "Handed over by" as before the sync, History the agent's "reassign" marked "kept yours" above "you set internal owner" |

## Expected Final State
Acme security review is read into Acme rollout's work and who's who through the real Claude Code in one answer given its text, Echo applying it (two new items with their words, every claim verified by a fresh run; no run with tools), nothing pending; T008's date and T003's owner are still the user's.

## Teardown
- None (the work keeps what the sync made; later stages must not expect Acme rollout's original tree)

## Notes
- The script's proposals are fixed (no dependency: a model may find none), so every run checks the same answer, the same rejection, the same pinned fields and the same verdicts.
- The real-model counterpart (2026-11-07): 4 min 36 s on the network test server, 45 min on this Mac's Ollama; the model skipped the Priya reassignment on its own, so "kept yours" was never exercised there. Outside the ledger only Echo changes files, as designed: applying the people proposals writes `projects/people.json` and `projects/acme-rollout/people-suggestions.json`; `ledger/mails.json` records "couldn't read your organisation's domains: mail waits" (no Microsoft 365 on test data).
- The automatic timing (a thread syncs by itself 15 minutes after its call) is the app's scheduler: not covered here.
