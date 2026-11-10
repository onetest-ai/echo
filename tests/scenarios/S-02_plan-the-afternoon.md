---
id: S-02
title: Plan the afternoon: what's left today, the calendar, set a meeting to record and change your mind
run: agent
data: small
cases:
  - tests/cases/today/TC-001_your-day-only-accepted-meetings.md
  - tests/cases/today/TC-002_your-day-line-now.md
  - tests/cases/window/TC-009_calendar-day-week.md
  - tests/cases/window/TC-010_calendar-card.md
  - tests/cases/calendar/TC-001_schedule-from-today.md
auto: [tests/auto/specs/S-02.spec.mjs]
---

# S-02: Plan the afternoon: what's left today, the calendar, set a meeting to record and change your mind

**Story:** Sam checks what is left of the day on Today, looks at the calendar by day and week, opens the next meeting's card, sets the Globex design review to record when it starts, and then takes it back.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build (less than 30 minutes old for stages 1 and 2) | the lead (`serve.sh up`) | 1–5 |
| Today's Your day at `#today` | stage 1, step 1 | 2 |
| The calendar's Week, on today's week | stage 3, steps 4–5 | 4 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `today/TC-001_your-day-only-accepted-meetings.md` | — | — |
| 2 | `today/TC-002_your-day-line-now.md` | — | On stage 1's view (`#today`): skip step 1 |
| 3 | `window/TC-009_calendar-day-week.md` | — | — |
| 4 | `window/TC-010_calendar-card.md` | — | On stage 3's view (Week, today's week after its step 5): skip the navigation of step 1; check its result (today's column has Globex design review) on the snapshot |
| 5 | `calendar/TC-001_schedule-from-today.md` | — | — |

## Notes
- Stages 1 and 2 read the same Your day: one visit. Stage 4 opens its card on the Week stage 3 ends on.
- Stage 5 goes back to `#today` on purpose: it sets the meeting to record and takes it back, and stage 4 must read the card before that (Globex design review not yet set to record). Moving stage 5 up would save one navigation but make stage 4 depend on stage 5's undo.
- If a view a stage continues from isn't there (an earlier stage failed), run the skipped steps as written.
- Stage 5 ends with the meeting no longer set to record.
