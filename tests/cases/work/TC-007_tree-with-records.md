---
id: TC-007
title: The tree shows workstream, stories with criteria, tasks, and questions, decisions and risks with their statuses
priority: high
type: functional
requirements: [F-WORK-07]
module: work
run: agent
platform: any
tags: [work, tree]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-007: The tree shows workstream, stories with criteria, tasks, and questions, decisions and risks with their statuses

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create three records under S001 of Acme rollout: `echoctl project ledger acme-rollout edit '{"op":"create","kind":"question","parent":"S001","title":"Who signs off the pilot?"}'` (note the ids each create prints), the same with `"kind":"decision","title":"Pilot starts in Berlin"` and `"kind":"risk","title":"SSO vendor delay"`
- In S-06 the decision and the risk are already there: the decision from stage 9 (work/TC-002) and the risk from stage 10 (work/TC-013), with the ids they printed; create only the question. On its own: create all three

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click "Tree" | A workstream header "ACME ROLLOUT · workstream" |
| 2 | Read the row S001 | `S001`, "SSO for the Berlin pilot", "criteria 0 / 1", status "● to do" |
| 3 | Read the rows T001 and T002 | T001: "Sam Rivera" and a date, "● to do"; T002: "Omar Haddad", "● to do"; both indented under S001 |
| 4 | Read the row "Who signs off the pilot?" | Tag "question", "Who signs off the pilot?", status "● open" (indented under S001) |
| 5 | Read the decision and the risk rows | "Pilot starts in Berlin": tag "decision", no status; "SSO vendor delay": tag "risk", status "● no response"; both at the story's own level (not indented like the tasks) |

## Expected Final State
The tree lists S001 with T001, T002, the question and the story-level decision and risk.

## Teardown
- Delete the three records with `echoctl project ledger acme-rollout edit '{"op":"delete","node":"<id>"}'` (the ids the creates printed)
- Click "Board"

## Changes
- Creates then deletes a question, a decision and a risk.
