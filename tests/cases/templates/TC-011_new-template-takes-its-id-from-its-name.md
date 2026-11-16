---
id: TC-011
title: New template takes its id from its name and is Yours
priority: high
type: functional
requirements: [F-TPL-04]
module: templates
run: agent
platform: any
tags: [templates]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-011: New template takes its id from its name and is Yours

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/client-kickoff.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click "New template" | An editor titled "New template" opens with a box "Name" and the sections Summary, Decisions, Action items, Open questions |
| 2 | Type `Client kickoff` in "Name", type `kickoff` in "Add a word" and press Enter | A chip "kickoff" shows |
| 3 | Click "Create" | A fifth row "Client kickoff" shows with the tag "Yours" and "kickoff · 4 sections"; the tab reads "Minutes 5"; the editor shows a button "Delete" |
| 4 | Run `grep -n '^name\|^match' "$ECHOCTL_HOME/.echo/templates"/client-kickoff.md` in Bash (`. <dir>/env` first) | Prints `name: client-kickoff` and `match: [kickoff]` |

## Expected Final State
A template of the user's own, in a file named from its name.

## Teardown
- Click "Delete" in the editor of "Client kickoff".
