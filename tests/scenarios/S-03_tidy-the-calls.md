---
id: S-03
title: Tidy the calls: a call without minutes, delete one, take it back
run: agent
data: small
cases:
  - tests/cases/window/TC-006_call-without-minutes.md
  - tests/cases/window/TC-008_delete-call.md
---

# S-03: Tidy the calls: a call without minutes, delete one, take it back

**Story:** Sam opens a call whose minutes were never written, sees what Echo offers, decides to delete a call, keeps it at first, deletes it, and restores it from the trash with the CLI.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-006_call-without-minutes.md` | — | — |
| 2 | `window/TC-008_delete-call.md` | — | — |

## Notes
- Stage 2 deletes and restores Initech planning: run stage 1 first, it reads the same call.
