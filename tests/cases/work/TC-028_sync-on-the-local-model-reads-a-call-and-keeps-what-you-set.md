---
id: TC-028
title: Sync now on a thread of the local model reads its new call into the work, verifies it, and keeps the fields the user set
priority: critical
type: functional
requirements: [F-THR-20, F-WORK-16, F-CC-04]
module: work
run: agent
platform: macos
data: standard
ai: local
tags: [work, ledger, sync, local-model, claude-code, long]
---

# TC-028: Sync now on a thread of the local model reads its new call into the work, verifies it, and keeps the fields the user set

**Module:** Work | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). `<profile>` and `<model>` below are the default AI profile's `name` and `notes_model` in `target/debug/echoctl ai status` (for example "Qwen · this computer" and `qwen3.8:27b-mlx`); the fake `claude` passes only runs routed to that profile, so no Claude account is used
- **One sync = three model runs in a row** (extract, verify, people) — about 5 minutes on the network test server, about 45 minutes on a Mac running the model itself. `project ledger <id> run` has no `--limit`: step 2 leaves one evidence item so the run reads only that one (the people pass still reads every call it hasn't read yet). Assert states, node ids and kinds, pinned fields and files; never the model's wording or which items it proposes. No retry unless it failed for an infrastructure reason (then BLOCKED)
- `$L` stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full). A node from the tree: `$L tree` → `nodes.<id>` (`due`, `internal_owner`, `pinned`, `history`)
- The user's own edits pin a field (`pinned`); the extractor is told never to change a pinned field, and a proposal that still tries is recorded in the item's history, not applied (the window marks it "kept yours", as in `calls/TC-020`)
- **In S-30** `<profile>` and `<model>` are read once, in the scenario's prelude (`echoctl ai status` and one `ls` of the calls), and every stage uses them. On its own, read them as above

## Test Data

| Item / evidence | Before the sync | After |
|---|---|---|
| T008 Run the data migration dry run | the user sets `due` 2026-11-20 (step 1) | `due` 2026-11-20, `pinned` holds "due" |
| T003 Prepare the training deck | the user sets `internal_owner` "Alex Chen" (step 1) | still "Alex Chen", `pinned` holds "internal_owner" — although the call says "I will ask Priya to prepare a short training deck" |
| Evidence pending | Globex pilot scoping (`2026-10-28_110000`), Acme security review (`2026-11-06_110000`), Memo (`2026-11-07_…`) | the first and last marked done by the user (step 2), so only Acme security review is read |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$L edit '{"op":"edit","node":"T008","set":{"due":"2026-11-20"}}'` and `$L edit '{"op":"edit","node":"T003","set":{"internal_owner":"Alex Chen"}}'` | Each prints JSON with `type` "human", `by` "human", `op` "edit"; in `$L tree`, T008 `due` "2026-11-20" with "due" in `pinned`, T003 `internal_owner` "Alex Chen" with "internal_owner" in `pinned` |
| 2 | Run `$L pending`; then `$L done --evidence 2026-10-28_110000` and `$L done --evidence <the Memo's id from pending>`; run `$L pending` again | First: three items (Globex pilot scoping, Acme security review, Memo); each `done` prints `{"done": "<id>"}`; then only `2026-11-06_110000` "Acme security review" is left. Save `$L tree` to `<dir>/tree-before.json` and note the node ids |
| 3 | Navigate to `{{base_url}}/#project/acme-rollout/work`, wait for "Sync now", note the time and click the "Sync now" link (`[data-pj="sync"]` in the work bar) | Within 15 seconds (reload to see it) the bar reads "Reading 1 call…" and "Sync now" is disabled |
| 4 | Every 2 minutes run `$L status` until `state` is no longer "running" (up to 60 minutes on a slow model); note how long it took | `state` "done", `processed` 1, `left` 0, `run` a run id; `message` holds the extract, verify and people reports |
| 5 | Run `$L pending` and `ls $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/` | `[]`; the run folder holds `log.txt` and the run's own JSON files (proposals for `2026-11-06_110000`, `packets.json`, `verdicts.json`, people files) |
| 6 | Run `$L tree` and compare with `<dir>/tree-before.json` | At least one new node (ids after the old ones) or a new agent history entry citing `2026-11-06_110000`; every agent entry of this sync has a `quote` and `evidence` "2026-11-06_110000" (or one of the people pass's) and a `verdict` (supports / weak / wrong) or is a create |
| 7 | In `$L tree` read T008 and T003 | T008 `due` "2026-11-20", T003 `internal_owner` "Alex Chen", both still pinned; any agent entry of this sync in their `history` that touches due or owner has `applied` false |
| 8 | Run `grep -c '' $ECHOCTL_HOME/projects/acme-rollout/ledger/runs/<run>/log.txt` and `grep '✗' …/log.txt` | The log names "Echo syncs Acme rollout (acme-rollout) on <profile>, model <model>"; every `→ Bash` line is `echoctl project ledger acme-rollout …`; every `→ Write` is inside the run folder; every `→ Read` inside the thread's `ledger/` or the plugin's `project-sync` skill; `✗` lines, if any, are refused tools (list them in the report) |
| 9 | Reload `{{base_url}}/#project/acme-rollout/work` | The bar reads "Synced <time> · N to review"; "Sync now" enabled. Click the card T008: Details "Due" "Fri, Nov 20", History "you set due" with "undo". Click T003: "Owner" "Alex Chen", History "you set internal owner" |

## Expected Final State
Acme security review is read into Acme rollout's work by the local model (new items with their words, verified or weak), nothing pending; T008's date and T003's owner are still the user's.

## Teardown
- None (the work keeps what the sync made; later stages must not expect Acme rollout's original tree)

## Notes
- Verified 2026-11-07 twice with the same sync started by `$L run` (the code behind Sync now): on the network test server (`qwen3.8:27b`) 4 min 36 s — new D003, R003, S005, T026–T028, two `mention` entries on T003/T008 with `applied` false, no `✗`; on this Mac's Ollama (`qwen3.8:27b-mlx`) 45 min 19 s (extract 27 min, of which ~10 min a retry loop, see the report; verify 4 min; people 15 min). On this Mac: R003 risk "Second-phase budget not yet approved", T026, T027 (client-owed dependencies, verified), a link T005 → T027 (supports); 3 to review. T008 and T003 untouched; the extractor skipped "the Priya reassignment of T003 (owner is pinned to Alex Chen)" on its own, so no "kept yours" entry was made in this run — only the files and fields show the rule held. No `✗` in the log.
- Outside the ledger only Echo's CLI changed files, as designed: the people pass's `people-apply` added facts and an empty "about" to `projects/people.json` and proposals to `projects/acme-rollout/people-suggestions.json`; `ledger/mails.json` records "couldn't read your organisation's domains: mail waits" (no Microsoft 365 on test data).
- The automatic timing (a thread syncs by itself 15 minutes after its call) is the app's scheduler: not covered here.
