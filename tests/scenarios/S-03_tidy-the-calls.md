---
id: S-03
title: Tidy the calls: a call without minutes, delete one, take it back
run: agent
data: small
cases:
  - tests/cases/window/TC-006_call-without-minutes.md
  - tests/cases/window/TC-008_delete-call.md
auto: [tests/auto/specs/S-03.spec.mjs]
---

# S-03: Tidy the calls: a call without minutes, delete one, take it back

**Story:** Sam opens a call whose minutes were never written, sees what Echo offers, decides to delete a call, keeps it at first, deletes it, and restores it from the trash with the CLI.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–2 |
| Initech planning open | stage 1, step 1 | 2 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `window/TC-006_call-without-minutes.md` | — | — |
| 2 | `window/TC-008_delete-call.md` | — | On stage 1's view (Initech planning open, on Transcript): skip step 1 |

## Notes
- Both stages use Initech planning: it is opened once (stage 1). Stage 2 deletes and restores it, so it runs second.
- If stage 1 failed before the call was open, run stage 2's step 1 as written.
