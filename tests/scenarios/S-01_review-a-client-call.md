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
---

# S-01: Review a client call: find it, read the minutes, check a point, note a follow-up

**Story:** Sam opens Echo after the Acme weekly sync, finds the call, reads its minutes, checks one point against what was said, searches the transcript, notes a follow-up for the call, and opens the task it produced from Today.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-001_calls-list-by-day.md` | — | — |
| 2 | `window/TC-002_calls-search.md` | — | — |
| 3 | `window/TC-003_call-minutes-tab.md` | — | Clear the search box |
| 4 | `window/TC-004_time-chip-jumps-to-transcript.md` | 3 | — |
| 5 | `window/TC-005_find-in-transcript.md` | — | — |
| 6 | `window/TC-007_my-notes-kept.md` | — | — |
| 7 | `today/TC-003_task-card-opens-details.md` | — | — |

## Notes
- Stage 4 fails until #101 is fixed (the chip highlights the turn before the moment); stage 5 doesn't depend on it.
- Stage 6 adds a note to Acme weekly sync and removes it in its teardown.
