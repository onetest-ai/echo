---
id: TC-010
title: project ledger tree reads the work; edit creates, edits, ticks a criterion, links, accepts, rejects, merges, undoes and deletes as the user
priority: high
type: functional
requirements: [F-CLI-06]
module: cli
run: agent
platform: any
data: standard
tags: [cli, work, ledger]
---

# TC-010: project ledger tree reads the work; edit creates, edits, ticks a criterion, links, accepts, rejects, merges, undoes and deletes as the user

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `$E` below stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full: the shell may be zsh, which doesn't split a variable into words). Read a node from the tree with `$E tree` → `nodes.<id>` (`state`, `title`, `parent`, `due`, `internal_owner`, `depends_on`, `blocks`, `merged_into`, `ac`, `suggestions`)
- Reload the page fully (`{{base_url}}/`) after a CLI change before reading the window

## Test Data

| Item (acme-rollout) | Before |
|---|---|
| S002 Data migration | open story; criteria "Data moved to the new tenant" (not met), "Old tenant switched off" (not met) |
| T004 Write the help desk guide | a pending `suggest_merge` to T003 ("the same work") |
| T006 Plan the rollback rehearsal | under S001; a pending `suggest_move` to S002 |
| T008 Run the data migration dry run | open, under S002 |
| New task | T026 (the next task id: T001–T025 exist) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$E tree` | JSON with `evidence`, `nodes`, `notes`, `pending`, `processed`, `roots`; `roots` is `["E01","E02"]`; `nodes.T004.suggestions[0]` has `op` "suggest_merge", `to` "T003", `status` "pending"; `nodes.T006.suggestions[0]` has `op` "suggest_move", `to` "S002", `status` "pending"; note the two suggestions' `mid` |
| 2 | Run `$E edit '{"op":"create","kind":"task","parent":"S002","title":"Book the dry-run window","internal_owner":"Me"}'` | JSON with `type` "create", `id` "T026", `by` "human"; in `$E tree`, T026 is `open` under `S002`, `internal_owner` "Me" |
| 3 | Run `$E edit '{"op":"edit","node":"T026","set":{"title":"Book the dry-run window with Rita","due":"2026-11-20"}}'` | JSON with `type` "human", `op` "edit", `node` "T026"; the tree's T026 has that title and `due` "2026-11-20" |
| 4 | Run `$E edit '{"op":"ac_set","node":"S002","ac":1,"met":true}'` | The tree's S002 `ac[0]` "Data moved to the new tenant" has `met: true` (and `human: true`); `ac[1]` stays `met: false` |
| 5 | Run `$E edit '{"op":"link","node":"T026","depends_on":"T008"}'` | T026 `depends_on` `["T008"]`; T008 `blocks` `["T026"]` |
| 6 | Run `$E edit '{"op":"accept","target":"<T006's mid>"}'` and `$E edit '{"op":"reject","target":"<T004's mid>"}'` | T006 `parent` is now "S002", its suggestion `status` "accept"; T004 stays under S001, its suggestion `status` "reject" |
| 7 | Reload, navigate to `{{base_url}}/#project/acme-rollout/work`, click "Tree" | "Data migration" (S002) reads "criteria 1 / 2"; under it T006 "Plan the rollback rehearsal" (with "pinned: parent"), T008 and T026 "Book the dry-run window with Rita" ("Me · Nov 20") are listed; "Plan the rollback rehearsal" is no longer under "SSO for the Berlin pilot" |
| 8 | Run `$E edit '{"op":"merge","node":"T026","into":"T008"}'` and note its `mid` | T026 `state` "merged", `merged_into` "T008" |
| 9 | Run `$E edit '{"op":"undo","target":"<the merge's mid>"}'` | T026 `state` "open" again, `merged_into` null |
| 10 | Run `$E edit '{"op":"delete","node":"T026"}'` | T026 `state` "deleted"; reload `{{base_url}}/#project/acme-rollout/work` → Tree: "Book the dry-run window with Rita" is not shown |
| 11 | Run `$E edit '{"op":"frobnicate"}'` and `$E edit '{"op":"edit","node":"T999","set":{"title":"x"}}'` | Exit code 1 each: "Error: bad op \"frobnicate\"" and "Error: unknown node \"T999\""; nothing is written |
| 12 | Run `$E edit '{"op":"undo","target":"0000000000"}'` (a mid no event has) | Exit code 1 with an error naming the unknown target; nothing is written |

## Expected Final State
S002 has its first criterion met; T006 sits under Data migration; T004's merge was rejected; T026 was made, linked, merged, unmerged and deleted. Every change shows in the tree and the window as the user's.

## Notes
- Step 12 is expected to FAIL today: `accept`, `reject` and `undo` take any `target` without checking it (exit 0, an event is appended that changes nothing). Reported as a suspected bug; no stage depends on it.
- The window's own edits go through the same `edit` ops (`work/` cases); this case checks the CLI path a Claude Code session uses.
