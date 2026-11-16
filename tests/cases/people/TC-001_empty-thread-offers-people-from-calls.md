---
id: TC-001
title: With nobody in the thread, People says so, offers the people seen in its calls; Map, Org chart and List are remembered
priority: medium
type: functional
requirements: [F-PPL-01, F-PPL-03]
module: people
run: agent
platform: any
tags: [people, views]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-001: With nobody in the thread, People says so, offers the people seen in its calls; Map, Org chart and List are remembered

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- In S-07 this precondition makes the scenario's shared thread: the stages after it use `people-lab`, so there the `project delete` of this case's Teardown is skipped

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` | The view switch (`role=group[name="View"]`) has the buttons "Map" (pressed), "Org chart", "List"; the legend champion, supporter, neutral, sceptic, blocker; a button "+ Person" |
| 2 | Read the body | "Nobody in this thread yet" and "2 people were in its calls — add them from List, then drag them where they stand." |
| 3 | Click "List" in the view switch (`role=group[name="View"]`) | "List" is pressed; the text "Nobody in this thread yet." and a block "SEEN IN CALLS" with "2 aren't in this thread yet" and the rows "Dana Lee" and "Omar Haddad" |
| 4 | Reload `{{base_url}}/` and navigate to the same URL | "List" is still pressed (remembered) |
| 5 | Click "Org chart" | "Org chart" is pressed; the body is the org chart (no table) |
| 6 | Click "Map" | "Map" is pressed again |

## Expected Final State
The Map is the active view.

## Teardown
- `echoctl project delete people-lab`
- Click "Map" if another view is active

## Changes
- Creates and deletes thread `people-lab`.
