---
id: TC-004
title: + Thread puts a call in a thread and a second pick takes it out
priority: high
type: functional
requirements: [F-CALLS-09]
module: calls
run: agent
platform: any
tags: [calls, threads]
---

# TC-004: + Thread puts a call in a thread and a second pick takes it out

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Call | Initech planning (in no thread) |
| Thread | Globex pilot |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Initech planning" | The header has no thread chip, only the button "+ Thread" |
| 2 | Click "+ Thread" | A menu lists the threads as `menuitemcheckbox` items "Acme rollout" and "Globex pilot" (both `aria-checked="false"`, no "✓"), then the item "New thread from this call…"; do not choose the last |
| 3 | Choose "Globex pilot" | A chip "Globex pilot" appears in the header |
| 4 | Run `echoctl project show globex-pilot` | Its calls include "Initech planning" with why `pinned` |
| 5 | Click "+ Thread" | The item now reads "✓ Globex pilot" (`aria-checked="true"`); "Acme rollout" has no tick |
| 6 | Choose "✓ Globex pilot" | The chip is gone |
| 7 | Run `echoctl project show globex-pilot` again | "Initech planning" is no longer in its calls; `exclusions` lists the call's stamp (the window's pick takes it out by hand) |

## Expected Final State
Initech planning is in no thread again.

## Teardown
- If the chip "Globex pilot" is still there: "+ Thread" › "✓ Globex pilot"
- `echoctl project reset globex-pilot <the call's folder>` (after `. <dir>/env`) clears the exclusion so the rules decide again; `exclusions` in `echoctl project show globex-pilot` is empty

## Notes
- Pins and unpins a call (no Claude Code).
