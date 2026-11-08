---
id: S-27
title: Others' words everywhere: nothing runs, nothing breaks
run: agent
data: hostile
cases:
  - tests/cases/hostile/TC-001_call-titles-and-folders.md
  - tests/cases/hostile/TC-002_meetings-calendar-and-coming-up.md
  - tests/cases/hostile/TC-003_transcript-and-speakers.md
  - tests/cases/hostile/TC-004_meeting-chat.md
  - tests/cases/hostile/TC-005_minutes-in-the-frame.md
  - tests/cases/hostile/TC-006_minutes-page-on-its-own.md
  - tests/cases/hostile/TC-007_my-notes.md
  - tests/cases/hostile/TC-008_work-items.md
  - tests/cases/hostile/TC-009_people-and-organisations.md
  - tests/cases/hostile/TC-010_threads-and-clients.md
  - tests/cases/hostile/TC-011_inbox-found-for-you-and-needs-you.md
  - tests/cases/today/TC-007_needs-you-now-actions-are-icons-named-in-their-labels.md
  - tests/cases/hostile/TC-012_plugins-and-marketplaces.md
  - tests/cases/hostile/TC-013_skills-templates-and-meeting-types.md
  - tests/cases/hostile/TC-014_vocabulary.md
  - tests/cases/hostile/TC-015_live-notes-and-run-outcomes.md
  - tests/cases/hostile/TC-016_ai-profiles.md
---

# S-27: Others' words everywhere: nothing runs, nothing breaks

**Data:** `hostile` (`tests/fixtures/serve.sh up <dir> <port> --set hostile`). Never the Echo app: it would start minutes runs.

**Story:** A person opens every place where other people's words reach the window: call titles, meetings, the transcript, the chat, the minutes and their own page, My notes, the work, people, threads and clients, the Inbox and Today, plugins, templates and skills, the vocabulary, live notes, and AI profiles. Everything carries `<img src=x onerror=alert(1)>`, `<script>`, `javascript:` links, a 10 000-character word, right-to-left and combining characters, emoji, Cyrillic and Markdown that looks like links or headings. Nothing may run, nothing may become an element, and the layout holds.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `hostile/TC-001_call-titles-and-folders.md` | — | Load `{{base_url}}/` (the cookie) in the open browser |
| 2 | `hostile/TC-002_meetings-calendar-and-coming-up.md` | — | — |
| 3 | `hostile/TC-003_transcript-and-speakers.md` | — | — |
| 4 | `hostile/TC-004_meeting-chat.md` | — | — |
| 5 | `hostile/TC-005_minutes-in-the-frame.md` | — | — |
| 6 | `hostile/TC-006_minutes-page-on-its-own.md` | — | — |
| 7 | `hostile/TC-007_my-notes.md` | — | — |
| 8 | `hostile/TC-008_work-items.md` | — | — |
| 9 | `hostile/TC-009_people-and-organisations.md` | — | — |
| 10 | `hostile/TC-010_threads-and-clients.md` | — | — |
| 11 | `hostile/TC-011_inbox-found-for-you-and-needs-you.md` | — | — |
| 12 | `hostile/TC-012_plugins-and-marketplaces.md` | — | — |
| 13 | `hostile/TC-013_skills-templates-and-meeting-types.md` | — | — |
| 14 | `hostile/TC-014_vocabulary.md` | — | — |
| 15 | `hostile/TC-015_live-notes-and-run-outcomes.md` | — | — |
| 16 | `hostile/TC-016_ai-profiles.md` | — | — |
| 17 | `today/TC-007_needs-you-now-actions-are-icons-named-in-their-labels.md` | — | — |

## Notes
- Every stage arms the dialog recorder itself (its step 1) and ends with check D; a dialog in any stage fails that stage. A stage never depends on another: all of them only read.
- The console is checked at the end of every stage: the `favicon.ico` 404 is the only message allowed. A "Refused to execute inline event handler" or other Content-Security-Policy report is a FAIL, not a pass (markup reached the DOM).
- Stages 5 and 7 read the minutes frame through `frameLocator` in `browser_run_code_unsafe`; keep the right panel closed and the sidebar folded for them.
- The data's times are relative to the build: build it right before the run (stage 2 needs the hostile meeting still to come, one earlier today).
- Do not press Trust, Ignore, Import, Create, Install, Edit, Record, Write minutes or Send to Claude anywhere in this scenario.
