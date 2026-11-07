---
id: TC-008
title: project list, show, of, people and found give the threads, their calls with why, a call's threads and suggestions, people and candidates as the window shows them
priority: high
type: functional
requirements: [F-CLI-04]
module: cli
run: agent
platform: any
data: standard
tags: [cli, threads, found]
---

# TC-008: project list, show, of, people and found give the threads, their calls with why, a call's threads and suggestions, people and candidates as the window shows them

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Today's Acme weekly sync and Initech planning carry the run's time in their folder names: `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)` and `I=$(ls "$ECHOCTL_HOME/calls" | grep -- '-initech-planning$')`

## Test Data

| Thing | Value |
|---|---|
| Threads | 10: acme-hackathon, acme-renewal, acme-rollout, globex-pilot, initech-hiring, internal-tooling, legacy-migration, tyrell-bid, umbrella-lead, umbrella-proposal |
| Acme rollout's calls | 6: three "pinned" (Memo, Acme security review, Globex pilot scoping), three "series" (Acme weekly sync today, Oct 31, Oct 24) |
| Initech planning | in no thread; suggested: Acme renewal, Acme rollout, Globex pilot, each "1 of its 1 people are in its calls" |
| Candidates (found) | 6 |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl project list` | Exit code 0; `projects` holds the 10 thread ids of the Test Data; `acme-rollout` has `name` "Acme rollout", `client` "acme", `stage` "active", `group` "delivery", `call_count` 6; `umbrella-lead` has `call_count` 0 and `stage` "lead"; `stages` lists the 10 stage ids from `lead` to `cancelled` |
| 2 | Navigate to `{{base_url}}/#projects` and wait for "Found for you" | The thread list shows "Acme rollout" with "6 calls · Today" and "Active", "Umbrella lead" with "0 calls" and "Lead"; "Found for you" carries the count 6 |
| 3 | Run `target/debug/echoctl project show acme-rollout` and read `calls[].title` / `calls[].why` | 6 calls, newest first: Memo `pinned`, Acme weekly sync `series`, Acme security review `pinned`, Acme weekly sync `series`, Globex pilot scoping `pinned`, Acme weekly sync `series`; `rules` is `[{"kind":"series","subject":"Acme weekly sync"},{"contains":"rollout","kind":"subject"}]` |
| 4 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | The same 6 calls in the same order; each `pinned` one reads "added by hand" with "Unpin", each `series` one "its meeting series" with "Take out"; the tab label reads "Calls6" |
| 5 | Run `target/debug/echoctl project of "$A"` | `projects` is `[{"id":"acme-rollout","name":"Acme rollout","why":"series"}]`; `suggested` has one entry, `id` "acme-renewal", `why` "6 of its 6 people are in its calls" |
| 6 | Run `target/debug/echoctl project of "$I"` | `projects` is `[]`; `suggested` lists `acme-renewal`, `acme-rollout`, `globex-pilot`, each with `why` "1 of its 1 people are in its calls" |
| 7 | Click the row Initech planning in Calls (`[data-call$="-initech-planning"]`) | Under the title the bar "Part of Acme renewal? 1 of its 1 people are in its calls" with "Add" and "Not this" (the window shows the first suggestion) |
| 8 | Run `target/debug/echoctl project people acme-rollout` | `calls` is 6; `people` has 13 entries, 9 of them with `in_thread: true` (Dana Lee, Omar Haddad, Rita Gomez, Ivan Petrov, Yuki Tanaka, Alex Chen, Lena Vogel, Ola Svensson, Tom Becker) |
| 9 | Navigate to `{{base_url}}/#project/acme-rollout/people` | The People tab label reads "People9" |
| 10 | Run `target/debug/echoctl project found` | A JSON array of 6 candidates, with `key`s `domain:acme.example`, `domain:northwind.example`, `series:umbrella lead notes`, `domain:initrode.example`, `domain:duff.example`, `series:design guild`; `domain:initrode.example` has `new_client` `{"domain":"initrode.example","name":"Initrode"}` and `why` "2 calls mostly with people from initrode.example" |
| 11 | Click "Found for you" in the thread list (`[data-pj="found"]`) | 6 candidates, each with "Accept" and "Reject"; their name boxes hold, in order, "Acme calls", "Northwind Analysts calls", "Umbrella lead notes 1", "Initrode calls", "Duff calls", "Design guild" (the `name`s of step 10); the first reads "3 calls mostly with people from acme.example" |

## Expected Final State
The CLI and the window agree on the threads, the calls and why each belongs, a call's suggestions, the thread's people and the candidates. Nothing changed.

## Notes
- `project of` for a call in a thread by its series says `series`; by a pin `pinned`; by its people `people`.
