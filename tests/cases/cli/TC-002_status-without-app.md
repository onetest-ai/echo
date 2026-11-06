---
id: TC-002
title: echoctl status reports no watcher and today's agenda when the app isn't running
priority: medium
type: functional
requirements: [F-CLI-03]
module: cli
run: agent
platform: any
tags: [cli, status]
external_id: TC-CLI-004
---

# TC-002: echoctl status reports no watcher and today's agenda when the app isn't running

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; the shell has run `. <dir>/env`; the app is not running on it

## Test Data

| Field | Value |
|-------|-------|
| Agenda | Acme weekly sync, Initech planning, Globex design review, Industry webinar, Acme demo |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl status` | Exit code 0; JSON with `watcher`, `armed` and `agenda` |
| 2 | Read `watcher` and `armed` | Both are null (no app runs on this data folder, nothing armed) |
| 3 | Read `agenda[].subject` | The five meetings of the test data, starting with "Acme weekly sync" |

## Expected Final State
`status` answers without the app: no watcher, nothing armed, today's agenda from the data folder.
