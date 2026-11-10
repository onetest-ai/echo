---
id: TC-007
title: A section moved down changes the order in the template file
priority: medium
type: functional
requirements: [F-TPL-03]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-007: A section moved down changes the order in the template file

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/status-sync.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the row "Status sync" | Headings: Summary, Progress, Blockers & risks, Action items; the button "Move Summary up" is disabled |
| 2 | Click the button "Move Summary down" | Headings: Progress, Summary, Blockers & risks, Action items; the row has the tag "Edited" |
| 3 | Run `grep -n '^## ' "$ECHOCTL_HOME/.echo/templates"/status-sync.md` in Bash (`. <dir>/env` first) | The first two headings printed are `## Progress` then `## Summary` |

## Expected Final State
Progress comes before Summary in the page and in the file.

## Teardown
- Click "Reset to built-in" in the editor of "Status sync".
