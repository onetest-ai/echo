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
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T002 | Owner a face "OH" and "Omar Haddad"; Handed over by "not known · Set"; Due "no date · Set" |
| 2 | Click the Owner value | The box takes the focus and Echo's menu opens under it (`.sg-menu`, no system list): "Me" (you), "Dana Lee", "Omar Haddad" — each a face and the name — then a line and "Nobody named" |
| 3 | Type `dan` key by key, press ↓ then Enter. Wait up to 3 s after an action for the window to redraw before reading. | While typed the menu shows only "Dana Lee" and "Nobody named"; then Owner reads "DL Dana Lee"; the avatar on the card reads "DL"; History has one new "you set internal owner" |
| 4 | Click "Set" next to Handed over by, type `Sam Rivera`, press Enter | The menu narrows to "Nobody named" as it is typed; Handed over by reads "Sam Rivera" with a face "SR" (a name not suggested is taken too) |
| 5 | Click "Set" next to Due; in Echo's calendar under it (`.dp-menu`: Today, Tomorrow, the Friday, Next Monday, In a week with their dates; the month; no "No date" — it has none) click › until December 2026, then 18 | Due reads "Fri, Dec 18" at once; the card shows a due chip "Dec 18". (Typed into the box, a date is saved on Enter or when left — TC-030) |
| 6 | Read History | Three entries "you set internal owner", "you set delegated by", "you set due", each with "undo" |

## Expected Final State
T002 has the new owner, hand-over and due date.

## Teardown
- Click "undo" on the three entries (newest first)
- Owner reads "Omar Haddad", Due "no date" again
- `target/debug/echoctl project person acme-rollout "Dana Lee" --remove` and `target/debug/echoctl project person acme-rollout "Omar Haddad" --remove`

## Changes
- Changes owner, delegated_by and due of T002.
