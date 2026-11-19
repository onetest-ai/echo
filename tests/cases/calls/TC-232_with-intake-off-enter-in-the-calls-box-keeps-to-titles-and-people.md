---
id: TC-232
title: With Intake off, Enter in the Calls box keeps to titles and people, with no chips, no results and no error
priority: high
type: functional
requirements: [F-CALLS-20, F-CALLS-24]
module: calls
run: agent
platform: any
tags: [calls, search, recall]
---

# TC-232: With Intake off, Enter in the Calls box keeps to titles and people

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --set standard` and served at `{{base_url}}`; Recall (Intake and
  search) is off, as the data leaves it: `target/debug/echoctl brain status`, run after `set -a; . <dir>/env; set +a`
  with `ECHOCTL_CONFIG_DIR` inside `<dir>`, shows `"enabled"` false or no settings

## Test Data

| Field | Value |
|-------|-------|
| Words | Acme weekly |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `Acme weekly` into "Search titles and people" | Only calls titled "Acme weekly sync" are listed, grouped by day |
| 2 | Press Enter in the box | The same calls stay listed; no row of filter icons or chips appears under the box, no "Said in" group, no "Titles and people" group, no error text; the placeholder is still "Search titles and people" |
| 3 | Press ⌘K (Ctrl+K on Windows), type `acme` | The palette lists actions and calls; it has no "Search what was said…" item and no "Search" group |
| 4 | Press Escape, clear the box | The palette closes; every call is listed again |

## Expected Final State
With Intake off the Calls box searches titles and people only, as before; nothing offers to search what was said.
