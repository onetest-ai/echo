---
id: TC-008
title: Asks lists, by person, what they owe late
priority: medium
type: functional
requirements: [F-TASK-08]
module: tasks
run: agent
platform: any
tags: [tasks, asks]
---

# TC-008: Asks lists, by person, what they owe late

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Omar's task is handed over by Sam and late: `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T002","set":{"delegated_by":"Me","due":"<the date two days ago, YYYY-MM-DD>"}}'`. Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Person | Omar Haddad |
| Task | Answer the security questionnaire |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "Owed to me 1" |
| 2 | Click the activity button "Inbox" to open the right panel, then its tab "Asks" | The panel shows "Asks · by person" |
| 3 | Read the panel | A person block "Omar Haddad" with the text "no meeting this week" and one button "Answer the security questionnaire — was due …" (the date two days ago); the empty text "Nothing to chase" is gone |
| 4 | Select "Globex pilot · 1" in the "Thread" filter | The panel no longer lists "Omar Haddad" (his task is in Acme rollout) and shows "Nothing to chase" |
| 5 | Select "All threads" and click the Asks button "Answer the security questionnaire — was due …" | The tab "Details" opens with the title "Answer the security questionnaire" and the path "Acme rollout › SSO for the Berlin pilot" |

## Expected Final State
Asks names Omar Haddad with his late task, follows the Thread filter, and opens the task's Details.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T002","set":{"delegated_by":null,"due":null}}'`
- Close the panel (`[aria-label="Close the panel"]`)

## Notes
- Changes Acme rollout T002 (undone in Teardown). Not on the data: a next meeting this week for the person ("no meeting this week" is what shows).
