---
id: TC-032
title: A task a later call says is covered (a covering mention the verifier supports) shows "Looks done" with Close and Not yet; a plain or unverified mention shows nothing; Close makes it done as yours, Not yet hides it until a later item suggests again; the sync never closes it
priority: high
type: functional
requirements: [F-WORK-18]
module: work
run: agent
platform: any
data: standard
tags: [work, ledger, looks-done]
---

# TC-032: A task that a later call only mentioned looks done

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`. `$A` = `target/debug/echoctl project ledger acme-rollout`
- `$A tree`'s `evidence` names the calls: **W** = the second of the three "Acme weekly sync" (a week before the build), **R** = "Acme security review" (the day before), **L** = the last "Acme weekly sync" (today)

**Changes:** Acme rollout's ledger (two tasks applied, a mention, the user's Close / Not yet)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$A apply --evidence <W> --proposals tests/fixtures/ledger/acme-guessed-due.json` | Exit 0; `created` T026 "Send Dana the rollout summary" and T027 "Send Dana the SSO checklist" |
| 2 | `$A apply --evidence <L> --proposals tests/fixtures/ledger/acme-plain-mention.json` (a plain mention), then `$A apply --evidence <R> --proposals tests/fixtures/ledger/acme-looks-done.json` (a covering one, `"covers": true`), then `$A tree` | T026 is still `open` with no `looks_done`: a plain mention never looks done, a covering one waits for the verifier |
| 2a | `tests/fixtures/ledger/verify.py acme-rollout supports --covers` (the verifier says the words cover it), then `$A tree` | Prints `"verdicts": 1`; T026 still `open`; its `looks_done` names R: `evidence` R, `kind` "meeting", `title` "Acme security review", `quote` "Please send the summary and the action items after the call", and a `mid` |
| 3 | Open `{{base_url}}/#project/acme-rollout/work`, open T026's details | A line "Looks done — the call Acme security review, <R's day>: “Please send the summary and the action items after the call”" with buttons "Close" and "Not yet" |
| 4 | Click "Not yet" | The line goes; `$A tree`: T026 `open`, no `looks_done` |
| 5 | In History, undo the "Not yet" entry (or `$A edit '{"op": "undo", "target": "<its mid>"}'`) | T026 shows "Looks done" again |
| 6 | Click "Close" | T026 `done` (its status pinned: History "set by you"), no `looks_done`; History has the call's entry for R with the quote |
| 7 | Undo the close in History | T026 `open` again with "Looks done" back |

## Expected Final State
T026 open with its "Looks done" suggestion; T027 open; nothing closed by the sync itself.

## Notes
- A mention from the item that created the task (W for T026) is never a "Looks done": only a later call, mail or chat, whose covering mention the verifier supported (`--covers` answers only those claims, not the data's others).
