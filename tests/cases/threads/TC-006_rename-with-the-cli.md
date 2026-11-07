---
id: TC-006
title: A thread renamed with `echoctl project set --name` shows its new name in the window
priority: medium
type: functional
requirements: [F-THR-07]
module: threads
run: agent
platform: any
tags: [threads, cli]
---

# TC-006: A thread renamed with `echoctl project set --name` shows its new name in the window

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot (`globex-pilot`) |
| New name | Globex pilot phase 2 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/brief` | The heading "Globex pilot" |
| 2 | Run `echoctl project set globex-pilot --name "Globex pilot phase 2"` | JSON with `"name": "Globex pilot phase 2"` and `"id": "globex-pilot"` |
| 3 | Reload `{{base_url}}/` then navigate to `{{base_url}}/#project/globex-pilot/brief` | The heading "Globex pilot phase 2"; the URL still has `globex-pilot`; the left list under GLOBEX shows "Globex pilot phase 2" and no row "Globex pilot" |

## Expected Final State
The window shows Globex pilot phase 2 with the unchanged id globex-pilot.

## Teardown
- `echoctl project set globex-pilot --name "Globex pilot"`

## Changes
- Changes the name of `globex-pilot`.
