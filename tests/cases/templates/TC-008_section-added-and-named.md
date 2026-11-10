---
id: TC-008
title: A section is added with + Add a section and saved once it has a heading
priority: medium
type: functional
requirements: [F-TPL-03]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-008: A section is added with + Add a section and saved once it has a heading

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
| 2 | Click the button "Add a section" at the foot of the sections | A fifth section appears with the heading "New section" selected |
| 3 | Type `Risks` as the heading and press Tab | The fifth heading reads "Risks" |
| 4 | Run `grep -n '^## ' "$ECHOCTL_HOME/.echo/templates"/status-sync.md` in Bash (`. <dir>/env` first) | Five headings; the last is `## Risks` |
| 5 | Reload the page and open "Status sync" | The row reads "... · 5 sections" |

## Expected Final State
Status sync has a fifth section called Risks.

## Teardown
- Click "Reset to built-in" in the editor of "Status sync".
