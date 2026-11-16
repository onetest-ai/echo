---
id: TC-001
title: The Threads list groups open threads under their client and folds a client
priority: high
type: functional
requirements: [F-THR-01, F-THR-02]
module: threads
run: agent
platform: any
tags: [threads, list]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-001: The Threads list groups open threads under their client and folds a client

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Nothing is folded: the Threads list shows all groups (browser storage key `echo.threads.folded` is empty)

## Test Data

| Field | Value |
|-------|-------|
| Open thread | Globex pilot |
| Folded client | Acme |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work` | Left column heading "Threads"; the list shows the groups "ACME", "GLOBEX" and "INITECH · YOUR COMPANY" in that order |
| 2 | Read the rows under ACME and GLOBEX | Under ACME: "Acme rollout", "1 call · Today", tag "Active". Under GLOBEX: "Globex pilot", "1 call · Yesterday", tag "Active" |
| 3 | Read the group INITECH · YOUR COMPANY | No thread row sits under it and it has no fold button (only the buttons for ACME and GLOBEX are named "Hide its threads") |
| 4 | Click the fold button of ACME (`[data-pj="fold"][data-id="acme"]`) | The row "Acme rollout" disappears from the list; the ACME header shows the count "1"; the button is now named "Show its threads" |
| 5 | Reload `{{base_url}}/#project/globex-pilot/work` (load `{{base_url}}/` first, then navigate) | ACME is still folded (the fold is remembered) |
| 6 | Click the ACME fold button again | The row "Acme rollout" is back; the button is named "Hide its threads" |
| 7 | Click the name "Acme" in the ACME header (`[data-pj="client"][data-id="acme"]`) | The URL ends with `#client/acme/overview` |

## Expected Final State
The list shows ACME, GLOBEX and INITECH · YOUR COMPANY; ACME is unfolded again and the Acme client page is open.

## Teardown
- Step 6 already unfolds ACME. If a step failed before it, click the ACME fold button until it is named "Hide its threads"

## Changes
- Changes only the folded state kept in the browser; no data is created.
