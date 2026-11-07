---
id: S-09
title: A call's housekeeping: list, minutes, kind, thread, tabs and the actions menu
run: agent
data: small
cases:
  - tests/cases/calls/TC-007_pick-a-call.md
  - tests/cases/calls/TC-009_row-state-icons.md
  - tests/cases/calls/TC-010_minutes-sections-of-a-short-call.md
  - tests/cases/calls/TC-003_kind-chip-guess-then-yours.md
  - tests/cases/calls/TC-011_what-moved-has-not-read-yet.md
  - tests/cases/calls/TC-005_thread-chip-opens-the-thread.md
  - tests/cases/calls/TC-004_add-and-remove-a-thread.md
  - tests/cases/calls/TC-006_tab-stays-from-call-to-call.md
  - tests/cases/calls/TC-001_call-actions-menu-with-minutes.md
  - tests/cases/calls/TC-002_call-actions-menu-without-minutes.md
  - tests/cases/calls/TC-008_call-that-cannot-be-opened.md
---

# S-09: A call's housekeeping: list, minutes, kind, thread, tabs and the actions menu

**Story:** Sam goes through his calls like a tidy-up: the list's icons, a short call's minutes, the kind of a meeting, what moved in its thread, a thread chip, putting a call in a thread and out again, the tabs, the actions menu of a call with and without minutes, and a call that cannot be opened.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-007_pick-a-call.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `calls/TC-009_row-state-icons.md` | — | — |
| 3 | `calls/TC-010_minutes-sections-of-a-short-call.md` | — | — |
| 4 | `calls/TC-003_kind-chip-guess-then-yours.md` | — | — |
| 5 | `calls/TC-011_what-moved-has-not-read-yet.md` | — | — |
| 6 | `calls/TC-005_thread-chip-opens-the-thread.md` | — | — |
| 7 | `calls/TC-004_add-and-remove-a-thread.md` | — | — |
| 8 | `calls/TC-006_tab-stays-from-call-to-call.md` | — | — |
| 9 | `calls/TC-001_call-actions-menu-with-minutes.md` | — | — |
| 10 | `calls/TC-002_call-actions-menu-without-minutes.md` | — | — |
| 11 | `calls/TC-008_call-that-cannot-be-opened.md` | — | — |

## Notes
- Stage 4 changes the kind of Acme weekly sync to Partner and back to Client; later stages do not read the kind. Stage 7 puts Initech planning in Globex pilot and takes it out; stages 8 to 10 do not depend on that.
- Not written for `small`: the countdown ("Minutes in N min", Not yet, Write them automatically again) needs a call that ended less than 15 minutes ago with automatic minutes switched on by the app (`auto.due` is `off` on this data); it waits for the `standard` data set, as do Chat, Screens, a memo's thread chips and the minutes run states.
- No stage presses a menu item, Write minutes, Sync now or Ask Claude.
