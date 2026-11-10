---
id: TC-012
title: Details edits Owner, Handed over by and Due of a task; a person's box suggests the thread's people
priority: high
type: functional
requirements: [F-WORK-11]
module: work
run: agent
platform: any
tags: [work, details]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-012: Details edits Owner, Handed over by and Due of a task; a person's box suggests the thread's people

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout has two people in its People tab: `target/debug/echoctl project person acme-rollout "Dana Lee"` and `target/debug/echoctl project person acme-rollout "Omar Haddad"` (the plain test data gives the thread none)

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
| 3 | Click "Set" next to Handed over by; read the box's suggestions (`input[data-ledger-set="delegated_by"]`, its `list` → `#wp-names option`) | The box suggests "Me" then the thread's people, in their order: "Me", "Dana Lee", "Omar Haddad" (Owner's box suggests the same) |
| 4 | Type `Sam Rivera`, press Enter | Handed over by reads "Sam Rivera" (a name not suggested is taken too) |
| 5 | Click "Set" next to Due, enter the date 2026-12-18 in the field named "Due" | Due reads "Fri, Dec 18"; the card shows a due chip "Dec 18" |
| 6 | Read History | Three entries "you set internal owner", "you set delegated by", "you set due", each with "undo" |

## Expected Final State
T002 has the new owner, hand-over and due date.

## Teardown
- Click "undo" on the three entries (newest first)
- Owner reads "Omar Haddad", Due "no date" again
- `target/debug/echoctl project person acme-rollout "Dana Lee" --remove` and `target/debug/echoctl project person acme-rollout "Omar Haddad" --remove`

## Changes
- Changes owner, delegated_by and due of T002.
