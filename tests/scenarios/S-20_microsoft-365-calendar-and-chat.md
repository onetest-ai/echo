---
id: S-20
title: Microsoft 365: sign in, the calendar, scheduled and joined recordings, chats, sign out
run: person
env: this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists, a Microsoft 365 test tenant the user signed in to once
cases:
  - tests/cases/calendar/TC-002_connect-m365.md
  - tests/cases/privacy/TC-006_token-private.md
  - tests/cases/calendar/TC-007_no-notetakers.md
  - tests/cases/calendar/TC-008_declined-cancelled.md
  - tests/cases/calendar/TC-005_scheduled-meeting-records.md
  - tests/cases/calendar/TC-006_join-and-record.md
  - tests/cases/calendar/TC-009_chat-scheduled.md
  - tests/cases/calendar/TC-010_chat-adhoc.md
  - tests/cases/calendar/TC-003_other-app-id.md
  - tests/cases/calendar/TC-004_sign-out.md
  - tests/cases/calendar/TC-011_without-m365.md
---

# S-20: Microsoft 365: sign in, the calendar, scheduled and joined recordings, chats, sign out

**Environment:** this Mac: the dev build run as an app beside the user's own Echo, on its own test data folder and config (never `~/echo`); native UI (tray menu, window, dialogs) driven through System Events; for a recorded call, a second Teams participant or Echo's test call source once it exists, a Microsoft 365 test tenant the user signed in to once

**Story:** Sam connects Microsoft 365, sees the week, sets a meeting to record and joins another, gets meeting and ad-hoc chats with the calls, checks labels for opted-out, declined and cancelled meetings, switches the app ID, signs out, and records without Microsoft 365.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calendar/TC-002_connect-m365.md` | — | — |
| 2 | `privacy/TC-006_token-private.md` | 1 | — |
| 3 | `calendar/TC-007_no-notetakers.md` | 1 | — |
| 4 | `calendar/TC-008_declined-cancelled.md` | 1 | — |
| 5 | `calendar/TC-005_scheduled-meeting-records.md` | 1 | — |
| 6 | `calendar/TC-006_join-and-record.md` | 1 | — |
| 7 | `calendar/TC-009_chat-scheduled.md` | 5 | — |
| 8 | `calendar/TC-010_chat-adhoc.md` | — | — |
| 9 | `calendar/TC-003_other-app-id.md` | — | — |
| 10 | `calendar/TC-004_sign-out.md` | — | — |
| 11 | `calendar/TC-011_without-m365.md` | 10 | — |

## Notes
- Sign-in is the user's, once; the agent never types a password.
