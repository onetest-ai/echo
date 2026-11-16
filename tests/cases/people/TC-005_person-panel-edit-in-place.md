---
id: TC-005
title: A person's panel shows who they are and their place; title, stance and role are edited in place
priority: medium
type: functional
requirements: [F-PPL-05]
module: people
run: agent
platform: any
tags: [people, panel]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-005: A person's panel shows who they are and their place; title, stance and role are edited in place

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- In S-07: `people-lab` is the scenario's shared thread, made by stage 10's precondition (people/TC-001) and kept; skip making it. On its own: make it as above
- Put Dana Lee in the thread ("Add" in the List). In S-07 she is in it since stage 11 (people/TC-003), placed on the Map and made a champion by the stages after it; the stance set here, supporter, differs from that
- The people view is Map (click "Map" in the view switch if another is pressed)

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |
| Person | Dana Lee |
| Title | Head of IT |
| Stance | supporter |
| Role | Sponsor |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` and click the chip "Dana Lee" | The right panel opens on "Person": "Dana Lee", "Acme", the blocks "WHO THEY ARE" ("Describe them…"), "IN PEOPLE LAB" (Role, Stance, Influence, Interest, Workstream, Governance, Works for here), "ABOUT THEM" (Title, Organisation "Acme (by mail domain)", Group, Reports to, Matters, Phone), "WHAT ECHO SAW" ("1 of 1" calls) |
| 2 | Click "Set" next to Title, type `Head of IT`, press Enter. Wait up to 3 s after an action for the window to redraw before reading. | Title reads "Head of IT" and the panel's subtitle starts with "Head of IT" |
| 3 | Choose Stance `supporter` in the select "Stance". Wait up to 3 s after an action for the window to redraw before reading. | The select shows "supporter" |
| 4 | Click "Set" next to Role, type `Sponsor`, press Enter. Wait up to 3 s after an action for the window to redraw before reading. | Role reads "Sponsor" |
| 5 | Run `echoctl project people people-lab` | Dana Lee has `"title": "Head of IT"` and in `places`: `"stance": "supporter"`, `"role": "Sponsor"` |

## Expected Final State
Dana Lee's panel shows the title, the stance and the role.

## Teardown
- `echoctl project delete people-lab`

## Changes
- Edits Dana Lee in `people-lab`.
