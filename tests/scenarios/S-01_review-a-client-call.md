---
id: S-01
title: Review a client call: find it, read the minutes, check a point, note a follow-up
run: agent
data: small
cases:
  - tests/cases/window/TC-001_calls-list-by-day.md
  - tests/cases/window/TC-002_calls-search.md
  - tests/cases/window/TC-003_call-minutes-tab.md
  - tests/cases/window/TC-004_time-chip-jumps-to-transcript.md
  - tests/cases/window/TC-005_find-in-transcript.md
  - tests/cases/window/TC-007_my-notes-kept.md
  - tests/cases/today/TC-003_task-card-opens-details.md
auto: [tests/auto/specs/S-01.spec.mjs]
---

# S-01: Review a client call: find it, read the minutes, check a point, note a follow-up

**Story:** Sam opens Echo after the Acme weekly sync, finds the call, reads its minutes, checks one point against what was said, searches the transcript, notes a follow-up for the call, and opens the task it produced from Today.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–7 |
| The Calls list at `#calls` | stage 1, step 1 | 2, 3 |
| Acme weekly sync open (Minutes, then Transcript) | stage 3, step 1 | 4, 5, 6 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-001_calls-list-by-day.md` | — | — |
| 2 | `window/TC-002_calls-search.md` | — | On stage 1's view (`#calls`): skip the navigation of step 1; check its result (three calls listed) on the snapshot |
| 3 | `window/TC-003_call-minutes-tab.md` | — | On stage 2's view (`#calls`, search box empty after its step 4): skip the navigation of step 1, click "Acme weekly sync" |
| 4 | `window/TC-004_time-chip-jumps-to-transcript.md` | 3 | On stage 3's view (Acme weekly sync on Minutes): skip step 1 |
| 5 | `window/TC-005_find-in-transcript.md` | — | On stage 4's view (Acme weekly sync on Transcript): skip the navigation and the click on the call in step 1; click "Transcript" only if another tab is selected |
| 6 | `window/TC-007_my-notes-kept.md` | — | On stage 5's view (Acme weekly sync open): skip the navigation and the click on the call in step 1; click "My notes" if the right panel doesn't show it (stage 3 left What moved there) |
| 7 | `today/TC-003_task-card-opens-details.md` | — | — |

## Notes
- One visit to the call serves stages 3 to 6: the call is opened once (stage 3) and each later stage goes on from the view the one before left. Stage 6's reload (its step 3) is a check and stays.
- If a view a stage continues from isn't there (an earlier stage failed), run the skipped steps as written.
- Stage 6 adds a note to Acme weekly sync and removes it in its teardown.
