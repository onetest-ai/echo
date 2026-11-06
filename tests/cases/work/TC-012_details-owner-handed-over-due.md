---
id: TC-012
title: Details edits Owner, Handed over by and Due of a task
priority: high
type: functional
requirements: [F-WORK-11]
module: work
run: agent
platform: any
tags: [work, details]
---

# TC-012: Details edits Owner, Handed over by and Due of a task

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Task | T002 Answer the security questionnaire |
| Owner | Dana Lee |
| Handed over by | Sam Rivera |
| Due | 2026-12-18 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T002 | Owner "Omar Haddad"; Handed over by "not known · Set"; Due "no date · Set" |
| 2 | Click the Owner value, type `Dana Lee`, press Enter. Wait up to 3 s after an action for the window to redraw before reading. | Owner reads "Dana Lee"; the avatar on the card reads "DL" |
| 3 | Click "Set" next to Handed over by, type `Sam Rivera`, press Enter | Handed over by reads "Sam Rivera" |
| 4 | Click "Set" next to Due, enter the date 2026-12-18 in the field named "Due" | Due reads "Fri, Dec 18"; the card shows a due chip "Dec 18" |
| 5 | Read History | Three entries "you set internal owner", "you set delegated by", "you set due", each with "undo" |

## Expected Final State
T002 has the new owner, hand-over and due date.

## Teardown
- Click "undo" on the three entries (newest first)
- Owner reads "Omar Haddad", Due "no date" again

## Changes
- Changes owner, delegated_by and due of T002.
