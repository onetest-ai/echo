---
id: TC-015
title: History of a synced item shows the quote, the mark, and the call's moment opens the call
priority: high
type: functional
requirements: [F-WORK-13]
module: work
run: agent
platform: any
tags: [work, history]
---

# TC-015: History of a synced item shows the quote, the mark, and the call's moment opens the call

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `echoctl project ledger globex-pilot sync`
- `echoctl project ledger globex-pilot apply --evidence <the build date − 1>_140000 --proposals /dev/stdin` (the evidence id is the stamp of "Globex pilot kickoff", always yesterday at 14:00: manifest.json → calls → Globex pilot kickoff; sync also prints it under `pending`) with the standard input `[{"op":"create","kind":"task","parent":"S001","type":"action","title":"Share the Globex pilot plan next week","internal_owner":"Sam Rivera","quote":"I'll draft the pilot plan and share it next week."}]`

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |
| Item | Share the Globex pilot plan next week |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work` and click the card "Share the Globex pilot plan next week" | Details opens; the path reads "Globex pilot › Pilot plan" |
| 2 | Read History | Under HISTORY, dated "yesterday": an entry "created" by "Me" with the mark "verified", the quote “I'll draft the pilot plan and share it next week.” and a source button `.wp-src` reading "▮ 00:13" with the title "Globex pilot kickoff" |
| 3 | Click that source button (`.wp-src[title="Globex pilot kickoff"]`) | The URL ends with `-globex-pilot-kickoff/minutes` or `/transcript`; the call "Globex pilot kickoff" is open |

## Expected Final State
The call Globex pilot kickoff is open; the new task keeps its quote in History.

## Teardown
- `echoctl project ledger globex-pilot edit '{"op":"delete","node":"<the id apply printed under created, T002 on the small data>"}'`

## Changes
- Adds a task with an agent entry to `globex-pilot`.
