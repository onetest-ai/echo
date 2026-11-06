---
id: TC-001
title: echoctl project tasks sorts open tasks into the user's and others' piles
priority: medium
type: functional
module: cli
run: agent
platform: any
tags: [cli, tasks]
---

# TC-001: echoctl project tasks sorts open tasks into the user's and others' piles

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; the shell has run `. <dir>/env`

## Test Data

| Field | Value |
|-------|-------|
| Sam's tasks | Send Acme the SSO checklist, Draft the Globex pilot plan |
| Others' task | Answer the security questionnaire (Omar Haddad) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl project tasks` | Exit code 0; JSON with a `piles` object |
| 2 | Read `piles.mine[].title` | Exactly the two tasks of Sam's, "Send Acme the SSO checklist" first or second |
| 3 | Read `piles.others[].title` | ["Answer the security questionnaire"] |
| 4 | Read `piles.mine` for "Send Acme the SSO checklist" | Its `project` is "acme-rollout" and its `due` is today's date |

## Expected Final State
The command prints the open tasks split by owner: Sam's two in `mine`, Omar's one in `others`, each with its thread.
