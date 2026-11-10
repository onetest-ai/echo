---
id: TC-022
title: The minutes frame shows only the sections the call's minutes hold, for a call, a Teams import and a memo
priority: medium
type: functional
requirements: [F-MIN-01]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, kind]
---

# TC-022: The minutes frame shows only the sections the call's minutes hold, for a call, a Teams import and a memo

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Sections in the frame (headings) |
|---|---|
| Acme weekly sync (Today) | Summary, Decisions, Action items, Open questions |
| Globex steering (imported) | Summary, Decisions |
| Memo | Summary |
| Globex pilot kickoff | Summary, Action items |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then the tab "Minutes" | `iframe#minutes-frame` is shown; its content (accessibility snapshot) has the headings Summary, Decisions, Action items, Open questions; no bar above it |
| 2 | Click the row Globex steering | The frame has Summary and Decisions; no Action items, no Open questions |
| 3 | Click the row Memo | The frame has Summary only |
| 4 | Click the row Globex pilot kickoff | The frame has Summary and Action items |

## Expected Final State
Globex pilot kickoff's minutes are shown.

## Notes
- Creates or changes nothing. The frame's text is not readable by `evaluate` or `browser_wait_for`: read it in the snapshot (see app_profile.md, Fragile areas).
- Globex pilot kickoff also has a bar above the minutes: `TC-023`.
