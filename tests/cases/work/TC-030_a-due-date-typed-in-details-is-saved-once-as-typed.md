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
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-030: A Due date typed in Details is saved once, as typed

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir>` (plain `small`); window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `$E` is `$ECHOCTL_HOME/projects/acme-rollout/ledger/events.jsonl`; count its lines (`wc -l < $E`) before step 2 and again before step 3
- Type with the keyboard, the way a person does: click the date box's first part, then type. Not a programmatic fill (it sets the whole value at once and hides this case's point). In Playwright: `click` the box `{ position: { x: 8, y: <half its height> } }` (its first part), then `keyboard.press` each digit
- The box's parts come in the order of the computer's region, not the page's language: month / day / year in the US, day.month.year in most of Europe (Chromium's headless shell on a Mac set to a European region shows `dd.mm.yyyy` even with the page in en-US). Type the date in the order the box shows; the dates read "Fri, Dec 18" in en-US (`toLocaleDateString`)

## Test Data

| Field | Value |
|-------|-------|
| Task | T002 Answer the security questionnaire (Acme rollout), Due "no date" |
| Date typed | 18 December 2026, in the box's order (`12 18 2026` for month / day / year, `18 12 2026` for day.month.year) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T002 | Details: Due "no date · Set" |
| 2 | Click "Set" next to Due, press Enter without typing, then press Escape (or click outside Details) | Due reads "no date · Set"; `$E` has no new line since the count before this step |
| 3 | Click "Set" next to Due, click the box's first part (the month), type the date's parts in the box's order (Test Data) | While typing, `$E` gets no new line (count after the typing) |
| 4 | Press Enter | Due reads "Fri, Dec 18"; `$E` has exactly one new line since the count before step 3: `{"op":"edit","node":"T002","set":{"due":"2026-12-18"}, …}` |
| 5 | Read History | Exactly one entry "you set due", with "undo" (the values are step 4's line) |

## Expected Final State
T002 is due 2026-12-18, set once.

## Teardown
- Click "undo" on every "you set due" entry, newest first (one on a fixed build): Due reads "no date"

## Notes
- #303: the box's `change` fires for each part typed (a year half typed, `0002`, is a date too), so the box is saved once, on Enter or when left, and only when its date changed; a date before 2000 or after 2100 is refused by the window and by the ledger (`ledger::check_edit`).
- Leaving the box (a click elsewhere in Details) saves the same way as Enter: one line.
