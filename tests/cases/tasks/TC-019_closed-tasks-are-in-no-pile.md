---
id: TC-019
title: A done, cancelled, merged or deleted task is in no pile and in no count
priority: critical
type: functional
requirements: [F-TASK-01, F-TASK-02]
module: tasks
run: agent
platform: any
tags: [tasks, piles, state, pairwise]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-019: A done, cancelled, merged or deleted task is in no pile and in no count

**Module:** Tasks | **Priority:** Critical | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

33 closed tasks (done, cancelled, merged, deleted), with or without an owner:

| Thread | Id | Title | State | Owner |
|---|---|---|---|---|
| Acme rollout | T001 | Send the SSO checklist | done | Me |
| Acme rollout | T009 | Prepare the renewal quote | done | Sam Rivera |
| Acme rollout | T013 | Order the licences | deleted | Dana Lee |
| Acme rollout | T017 | Publish the release notes | cancelled | Alex Chen |
| Acme rollout | T019 | Clean up the test accounts | cancelled | Sam Rivera |
| Acme rollout | T020 | Agree the acceptance criteria | merged | nobody named |
| Acme rollout | T022 | Confirm the go-live date (2) | done | Sam Rivera |
| Acme rollout | T025 | Agree the acceptance criteria (2) | deleted | Sam Rivera |
| Acme renewal | T001 | Prepare the renewal quote | done | Sam Rivera |
| Acme renewal | T002 | Legal review of the contract terms | done | Rita Gomez |
| Acme renewal | T004 | Share the architecture diagram | merged | Sam Rivera |
| Acme renewal | T007 | Write the cut-over runbook | cancelled | Sam Rivera |
| Acme renewal | T009 | Fix the SSO redirect | deleted | nobody named |
| Acme renewal | T013 | Review the invoice | deleted | Sam Rivera |
| Acme renewal | T015 | Confirm the rollback window | deleted | Alex Chen |
| Acme renewal | T016 | Rehearse the demo | done | Sam Rivera |
| Acme renewal | T020 | Rehearse the demo (2) | cancelled | Sam Rivera |
| Globex pilot | T003 | Chase the signed SOW | deleted | Sam Rivera |
| Globex pilot | T008 | Collect feedback from Berlin | done | Kim Park |
| Globex pilot | T011 | Chase the signed SOW (2) | cancelled | Sam Rivera |
| Globex pilot | T013 | Review the access matrix (2) | merged | Sam Rivera |
| Umbrella proposal | T001 | Review the test plan | done | Alex Chen |
| Umbrella proposal | T003 | Draft the status report | merged | Alex Chen |
| Umbrella proposal | T006 | Ask for the budget code | cancelled | nobody named |
| Umbrella proposal | T007 | Archive the old tenant | merged | Sam Rivera |
| Umbrella proposal | T015 | Check the licence count (2) | done | Sam Rivera |
| Umbrella proposal | T016 | Sign off the data mapping (2) | deleted | Sam Rivera |
| Initech hiring | T001 | Update the risk log | cancelled | Lena Vogel |
| Initech hiring | T002 | Set up the test tenant | done | Sam Rivera |
| Initech hiring | T004 | Agree the support hours | merged | Lena Vogel |
| Initech hiring | T007 | Plan the hypercare week | done | Sam Rivera |
| Initech hiring | T011 | Update the contact list | done | nobody named |
| Initech hiring | T014 | Update the contact list (2) | merged | Sam Rivera |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe"; click "+ 8" in "I owe", "+ 5" in "Nobody named" and "Others · 24" | Every pile and Others is unfolded; the page holds 59 elements `[data-tk="open"]` in all (14 + 5 + 5 + 11 + 24) |
| 2 | For each row of the table look for `[data-tk="open"][data-project=<thread id>][data-node=<Id>]` (thread ids are the names in lower case with a hyphen: acme-rollout, acme-renewal, globex-pilot, umbrella-proposal, initech-hiring) | None of the rows has a card in any pile: done, cancelled, merged and deleted tasks are shown nowhere |
| 3 | Open the "Thread" filter and read the options of the five threads | Among them "Acme renewal · 11", "Acme rollout · 17", "Globex pilot · 10", "Initech hiring · 9", "Umbrella proposal · 10": only open tasks are counted (the list also offers "Umbrella lead · 0" and "Internal tooling · 0", threads with no open task: `TC-024`) |
| 4 | Look in "Waiting for others" for a card of Kim Park or Lena Vogel | None: Globex pilot T003, T011 and Initech hiring T001 wait on them but are closed, so they leave no card |
| 5 | Run `echoctl project tasks`, collect the thread and id of every entry of `piles` and look for the rows of the table | None of the rows is in any pile |

## Expected Final State
No card of a closed task is on Tasks; the piles hold 59 cards.

## Teardown
- None: the case only reads

## Notes
- Covers node.state (done, cancelled, merged, deleted) against owner, handed over by, wait, due and mode: the closed rows of the data are listed.
- Tasks shows only open, in progress and blocked tasks (`tasks.rs` `open`).
