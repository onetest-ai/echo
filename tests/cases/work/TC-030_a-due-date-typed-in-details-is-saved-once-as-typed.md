---
id: TC-030
title: A Due date typed in Details is saved once, as typed; Enter on an empty box saves nothing
priority: high
type: functional
requirements: [F-WORK-11]
module: work
run: agent
platform: any
tags: [work, details, due, keyboard]
---

# TC-030: A Due date typed in Details is saved once, as typed

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir>` (plain `small`); window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `$E` is `$ECHOCTL_HOME/projects/acme-rollout/ledger/events.jsonl`; count its lines before steps 2 and 5 (`wc -l < $E`)
- Type with the keyboard, the way a person does: click a part of the date box, then type. Not a programmatic fill (it sets the whole value at once and hides this case's point)

## Test Data

| Field | Value |
|-------|-------|
| Task | T002 Answer the security questionnaire (Acme rollout), Due "no date" |
| Date typed | 12 / 18 / 2026 (the box's order in the en-US locale: month, day, year; in another locale, its own order) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T002 | Details: Due "no date · Set" |
| 2 | Click "Set" next to Due, then press Enter without typing | The box closes or stays empty; Due still reads "no date"; `$E` has no new line |
| 3 | Click "Set" next to Due, click the box's first part (the month), type `12`, `18`, `2026` | While typing, `$E` gets no new line |
| 4 | Press Enter | Due reads "Fri, Dec 18"; `$E` has exactly one new line, `{"op":"edit","node":"T002","set":{"due":"2026-12-18"}, …}` |
| 5 | Read History | One new entry "you set due" with "undo"; no entry with any other date |

## Expected Final State
T002 is due 2026-12-18, set once.

## Teardown
- Click "undo" on the "you set due" entry: Due reads "no date"

## Notes
- Known issue #303: each keystroke is saved now (the box's `change` fires per part of the date), dates like `0002-12-12` included, and Enter on an untouched empty box saves today. Steps 2–5 fail until it is fixed.
