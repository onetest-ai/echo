---
id: TC-013
title: Prepare now with Claude Code refusing leaves a prep that names the reason and the facts it could gather
priority: medium
type: functional
requirements: [F-PREP-03, F-PREP-06, F-PREP-04]
module: prep
run: agent
platform: any
data: standard
tags: [prep, fake-claude, known-issue]
---

# TC-013: Prepare now with Claude Code refusing leaves a prep that names the reason and the facts it could gather

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A prep's id is the one in `<dir>/manifest.json` → `preps` (stable for a given meeting id); Coming up today rows are `[data-up="<prep id>"]`
- The data's `claude` is a fake that refuses every run

## Test Data

| Field | Value |
|---|---|
| Meeting | Initech team lunch (`ev-m08`, to come, in Globex pilot, one person Kim Park) |
| Prep file after Prepare now | `error`: "Claude · work: test data: Claude Code is a fake here"; people: Kim Park |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync", click "Coming up today" and the row "Initech team lunch" | The page says "No prep yet", "Echo writes it this morning for every meeting of a thread." and the button "Prepare now" |
| 2 | Click "Prepare now" and wait 10 seconds | The page shows the reason "Claude · work: test data: Claude Code is a fake here" (not the "No prep yet" box); the header lists "Kim Park" |
| 3 | Run `echoctl prep --show <the row's prep id>` | JSON with `error` "Claude · work: test data: Claude Code is a fake here" and one person, Kim Park |
| 4 | Read the row's `title` in Coming up today | "Prep ready" |

## Expected Final State
The prep exists and its page says why it has no summary.

## Notes
- Creates a prep file (a fact-only one); the run's data is disposable.
