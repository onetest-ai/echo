---
id: S-09
title: A call's housekeeping: list, minutes, kind, thread, tabs and the actions menu
run: agent
data: small
cases:
  - tests/cases/calls/TC-007_pick-a-call.md
  - tests/cases/calls/TC-009_row-state-icons.md
  - tests/cases/calls/TC-010_minutes-sections-of-a-short-call.md
  - tests/cases/calls/TC-001_call-actions-menu-with-minutes.md
  - tests/cases/calls/TC-011_what-moved-has-not-read-yet.md
  - tests/cases/calls/TC-002_call-actions-menu-without-minutes.md
  - tests/cases/calls/TC-004_add-and-remove-a-thread.md
  - tests/cases/calls/TC-003_kind-chip-guess-then-yours.md
  - tests/cases/calls/TC-005_thread-chip-opens-the-thread.md
  - tests/cases/calls/TC-006_tab-stays-from-call-to-call.md
  - tests/cases/calls/TC-008_call-that-cannot-be-opened.md
---

# S-09: A call's housekeeping: list, minutes, kind, thread, tabs and the actions menu

**Story:** Sam goes through his calls like a tidy-up: the list's icons, a short call's minutes, the kind of a meeting, what moved in its thread, a thread chip, putting a call in a thread and out again, the tabs, the actions menu of a call with and without minutes, and a call that cannot be opened.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–11 |
| The Calls list at `#calls` | stage 1, step 1 | 2–8 |
| Acme weekly sync open on Minutes | stage 4, step 1 | 5 |
| Initech planning open | stage 5, step 4 | 6, 7 |
| Acme weekly sync open (Client again) | stage 8's Teardown | 9 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-007_pick-a-call.md` | — | On the prelude's view: skip the load of `{{base_url}}/` in step 1, navigate to `#calls` |
| 2 | `calls/TC-009_row-state-icons.md` | — | On stage 1's view (`#calls`, no call open): skip the navigation of step 1; check its result on the snapshot |
| 3 | `calls/TC-010_minutes-sections-of-a-short-call.md` | — | On stage 2's view (`#calls`): skip the navigation of step 1, click "Globex pilot kickoff" |
| 4 | `calls/TC-001_call-actions-menu-with-minutes.md` | — | On stage 3's view (the Calls list beside Globex pilot kickoff): skip the navigation of step 1, click "Acme weekly sync" |
| 5 | `calls/TC-011_what-moved-has-not-read-yet.md` | — | On stage 4's view (Acme weekly sync open on Minutes, menu closed): skip the navigation and the click on the call in step 1; open What moved as it says |
| 6 | `calls/TC-002_call-actions-menu-without-minutes.md` | — | On stage 5's view (Initech planning open, its step 4): skip step 1; check its result on the snapshot |
| 7 | `calls/TC-004_add-and-remove-a-thread.md` | — | On stage 6's view (Initech planning open, menu closed): skip step 1; check its result (no thread chip, only "+ Thread") on the snapshot |
| 8 | `calls/TC-003_kind-chip-guess-then-yours.md` | — | On stage 7's view (the Calls list, Initech planning open): skip the navigation of step 1, click "Acme weekly sync" |
| 9 | `calls/TC-005_thread-chip-opens-the-thread.md` | — | On stage 8's view (Acme weekly sync open after its Teardown): skip step 1; check its result on the snapshot |
| 10 | `calls/TC-006_tab-stays-from-call-to-call.md` | — | — |
| 11 | `calls/TC-008_call-that-cannot-be-opened.md` | — | — |

## Notes
- The calls are opened in an order that lets each stage go on from the call the stage before left open: the list (1–2), Globex pilot kickoff (3), Acme weekly sync (4–5), Initech planning (5–7), Acme weekly sync (8–9). Each stage skips only the navigation or the click that reaches it; if that call isn't open (an earlier stage failed), run the skipped steps as written.
- Stage 10 navigates again on purpose: it needs Acme weekly sync freshly opened on Minutes after stage 9 left for the thread. Stage 11 loads `{{base_url}}/` itself (its check needs a fresh load).
- Stage 8 changes the kind of Acme weekly sync to Partner and back to Client; the stages before it don't read the kind, the stages after it don't depend on the guess mark. Stage 7 puts Initech planning in Globex pilot and takes it out; stages 8 to 11 do not depend on that.
- Not written for `small`: the countdown ("Minutes in N min", Not yet, Write them automatically again) needs a call that ended less than 15 minutes ago with automatic minutes switched on by the app (`auto.due` is `off` on this data); it waits for the `standard` data set, as do Chat, Screens, a memo's thread chips and the minutes run states.
- No stage presses a menu item, Write minutes, Sync now or Ask Claude.
