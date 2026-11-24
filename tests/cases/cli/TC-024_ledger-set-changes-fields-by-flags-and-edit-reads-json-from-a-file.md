---
id: TC-024
title: project ledger set changes an item's fields by flags, with text that has quotes in it; edit-file reads the JSON from a file
priority: high
type: functional
requirements: [F-CLI-06]
module: cli
run: agent
platform: any
data: standard
tags: [cli, work, ledger]
---

# TC-024: project ledger set changes an item's fields by flags; edit-file reads the JSON from a file

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `$E` below stands for `target/debug/echoctl project ledger acme-rollout` (write it out in full). Read a node with `$E tree` → `nodes.<id>`

**Changes:** the acme-rollout ledger (T008's fields; put back in step 7)

## Test Data

| Item (acme-rollout) | Before |
|---|---|
| T008 Run the data migration dry run | under S002, client owner Rita Gomez, no internal owner, due date or description; note its `state` in `$E tree` before step 1 (it is put back in steps 7–8) |
| A description with quotes | `Closed: Rita's dry run "passed" (see the runbook)` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `$E set T008 --state in_progress --internal-owner "Rita Ortiz" --due 2026-11-30` | Exit code 0; one line, its fields in name order: `T008: due 2026-11-30 · internal_owner Rita Ortiz · state in_progress (mid <the edit's mid>)`; the tree's T008 has those three values |
| 2 | Run `$E set T008 --description "Closed: Rita's dry run \"passed\" (see the runbook)"` | Exit code 0; one line naming `description`; the tree's T008 `description` is exactly the test data's text, both quote kinds kept |
| 3 | Write the test data's text to a file `<dir>/desc.txt`; run `$E set T008 --description-file <dir>/desc.txt --internal-owner none` | Exit code 0; T008's `description` is the file's text (its one trailing newline dropped; `none` written in a file would stay text), `internal_owner` null |
| 4 | Run `$E set T008 --state finished`, then `$E set T008`, then `$E set T999 --state done`, then `$E set T008 --owner Me` | Exit code non-zero each, nothing written (the events file's line count unchanged): the state names the allowed ones; "nothing to set"; "unknown node"; the last is refused by the command's own flags: "unexpected argument '--owner'" (`$E set --help` lists them) |
| 5 | Write `{"op":"edit","node":"T008","set":{"title":"Run the data migration dry run (Rita's)"}}` to `<dir>/edit.json`; run `$E edit-file <dir>/edit.json` | Exit code 0; the same JSON answer as `edit '<json>'` gives; T008's title has the apostrophe |
| 6 | Run `$E edit-file <dir>/missing.json` | Exit code non-zero: the file can't be read, it names the file |
| 7 | Put T008 back: `$E set T008 --state <noted state> --due none --description none --title "Run the data migration dry run"` | Exit code 0; one line with those four fields; T008 has the noted state, no due date, no description, its old title |
| 8 | Run these inside a Yolo run of task `acme-rollout/T008` (`ECHOCTL_HEADLESS=1 ECHOCTL_RUN_TASK=acme-rollout/T008` in front of the command): first `$E set T008 --state done`, then `$E set T008 --internal-owner Me`, then `$E edit-file <dir>/edit.json`, then `$E set T008 --description-file <dir>/desc.txt` | The first is allowed (Yolo closing its own task); the second is refused with Yolo's message, which names `echoctl project work acme-rollout '{"op":"edit","node":"T008","set":{"state":"done"}}'`; the third and fourth are refused: inside a run the text is passed itself, never a file. Then, outside the run, `$E set T008 --state <noted state>` |

## Expected Final State
T008 as before the run (steps 7 and 8).

## Notes
- `set` is the same edit as `edit '{"op":"edit","node":…,"set":{…}}'`: it pins the fields as the user's and can be undone with the `mid` its line names (`edit '{"op":"undo","target":"<mid>"}'`).
- `none` clears an owner, a due date or a description.
