---
id: TC-009
title: A section is removed with its trash button
priority: medium
type: functional
requirements: [F-TPL-03]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-009: A section is removed with its trash button

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/status-sync.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the row "Status sync" | Four sections |
| 2 | Click the button "Remove the section Blockers & risks" | Three sections: Summary, Progress, Action items; the row reads "... · 3 sections" |
| 3 | Run `grep -c '^## ' "$ECHOCTL_HOME/.echo/templates"/status-sync.md` in Bash (`. <dir>/env` first) | Prints `3` |

## Expected Final State
The template has three sections.

## Teardown
- Click "Reset to built-in" in the editor of "Status sync".
