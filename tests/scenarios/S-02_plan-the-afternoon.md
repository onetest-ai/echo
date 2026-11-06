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
---

# S-02: Plan the afternoon: what's left today, the calendar, set a meeting to record and change your mind

**Story:** Sam checks what is left of the day on Today, looks at the calendar by day and week, opens the next meeting's card, sets the Globex design review to record when it starts, and then takes it back.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `today/TC-001_your-day-only-accepted-meetings.md` | — | — |
| 2 | `today/TC-002_your-day-line-now.md` | — | — |
| 3 | `window/TC-009_calendar-day-week.md` | — | — |
| 4 | `window/TC-010_calendar-card.md` | — | — |
| 5 | `calendar/TC-001_schedule-from-today.md` | — | — |

## Notes
- Stage 4 expects Globex design review not yet set to record: keep it before stage 5.
- Stage 5 ends with the meeting no longer set to record.
