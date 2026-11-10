---
id: TC-001
title: Tasks sorts the open tasks into piles with counts and folds the others
priority: critical
type: functional
requirements: [F-TASK-01, F-TASK-02]
module: tasks
run: agent
platform: any
tags: [tasks, piles]
auto: [tests/auto/specs/S-08.spec.mjs]
---

# TC-001: Tasks sorts the open tasks into piles with counts and folds the others

**Module:** Tasks | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- No change has been made to the tasks on the data (run this before any case that edits them)

## Test Data

| Field | Value |
|-------|-------|
| Task on Sam | Send Acme the SSO checklist (Acme rollout), Draft the Globex pilot plan (Globex pilot) |
| Task on others | Answer the security questionnaire (Omar Haddad) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | The heading "Tasks" and the text "across 2 active projects"; the filters "All threads", "Anyone", "Due" |
| 2 | Read the four pile headings | "I owe 2", "Owed to me 0", "Waiting for others 0", "Nobody named 0"; the last three each say "Nothing here" |
| 3 | Read the cards in "I owe" | Two cards: "Send Acme the SSO checklist" and "Draft the Globex pilot plan" |
| 4 | Look below the piles | A folded button "Others · 1" with the text "other people's work in your threads: not handed over by you, not stopping you"; no card "Answer the security questionnaire" is shown |
| 5 | Click "Others · 1" | The card "Answer the security questionnaire" appears with "Omar Haddad" and the thread "Acme rollout" |
| 6 | Click "Others · 1" again | The card goes; the pile is folded again |
| 7 | Run `echoctl project tasks` | JSON: `piles.mine` holds 2 tasks ("Send Acme the SSO checklist", "Draft the Globex pilot plan"), `piles.others` holds 1 ("Answer the security questionnaire"), `piles.owed`, `piles.waiting` and `piles.nobody` are empty |

## Expected Final State
Tasks shows I owe 2 and the folded Others · 1, matching `echoctl project tasks`; the other three piles are empty.

## Notes
- Creates or changes nothing.
