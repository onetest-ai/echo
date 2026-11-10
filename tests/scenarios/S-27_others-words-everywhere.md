---
id: S-27
title: Others' words everywhere: nothing runs, nothing breaks
run: agent
data: hostile
cases:
  - tests/cases/hostile/TC-002_meetings-calendar-and-coming-up.md
  - tests/cases/hostile/TC-011_inbox-found-for-you-and-needs-you.md
  - tests/cases/today/TC-011_needs-you-now-actions-are-icons-named-in-their-labels.md
  - tests/cases/hostile/TC-010_threads-and-clients.md
  - tests/cases/hostile/TC-009_people-and-organisations.md
  - tests/cases/hostile/TC-008_work-items.md
  - tests/cases/hostile/TC-012_plugins-and-marketplaces.md
  - tests/cases/hostile/TC-013_skills-templates-and-meeting-types.md
  - tests/cases/hostile/TC-014_vocabulary.md
  - tests/cases/hostile/TC-001_call-titles-and-folders.md
  - tests/cases/hostile/TC-005_minutes-in-the-frame.md
  - tests/cases/hostile/TC-007_my-notes.md
  - tests/cases/hostile/TC-003_transcript-and-speakers.md
  - tests/cases/hostile/TC-004_meeting-chat.md
  - tests/cases/hostile/TC-015_live-notes-and-run-outcomes.md
  - tests/cases/hostile/TC-016_ai-profiles.md
  - tests/cases/hostile/TC-006_minutes-page-on-its-own.md
auto: [tests/auto/specs/S-27.spec.mjs]
---

# S-27: Others' words everywhere: nothing runs, nothing breaks

**Data:** `hostile` (`tests/fixtures/serve.sh up <dir> <port> --set hostile`). Never the Echo app: it would start minutes runs.

**Story:** A person opens every place where other people's words reach the window: call titles, meetings, the transcript, the chat, the minutes and their own page, My notes, the work, people, threads and clients, the Inbox and Today, plugins, templates and skills, the vocabulary, live notes, and AI profiles. Everything carries `<img src=x onerror=alert(1)>`, `<script>`, `javascript:` links, a 10 000-character word, right-to-left and combining characters, emoji, Cyrillic and Markdown that looks like links or headings. Nothing may run, nothing may become an element, and the layout holds.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`hostile`) | the lead (`serve.sh up … --set hostile`), right before the run | all |
| The dialog and page-error guard (`page.__dlg`, `page.__perr`) | Prelude, steps P2–P3 (armed once, checked to survive a reload) | 1–16 (each stage skips its step 1 and reads its own record with check D, which empties it) |
| One page load of the window (`{{base_url}}/`, then hash navigations only) and its console | Prelude, step P4 (the console's baseline) | 1–8 (group A, the window's own views: console read once, at stage 8) and 9–15 (group B, the hostile call and its minutes frame: console read once, at stage 15) |
| Your day on screen | stage 1 (step 16) | 2 |
| The hostile call open (`[data-call*="HOSTILE-61"]`), right panel closed | stage 10 (step 2) | 10–14 (each skips its own step 2) |
| The standalone minutes page (full loads: `/minutes/<folder>` and back) | stage 16 (steps 3, 7) | 16 (group C: its own console read, step 8) |

## Prelude

Done once, before stage 1, timed as the stage `prelude`.

| # | Action | Expected Result |
|---|---|---|
| P1 | Navigate to `{{base_url}}/` (the cookie) in the open browser and wait for the text "Your day" | Your day loads |
| P2 | Run `browser_run_code_unsafe` with the **Guard** code of any of these cases (Checks) | It returns `armed` |
| P3 | Run `browser_run_code_unsafe` with `async (page) => { await page.reload(); await page.getByText('Your day').first().waitFor(); await page.evaluate(() => setTimeout(() => alert('probe'), 0)); await page.waitForTimeout(1000); const r = [...page.__dlg]; page.__dlg.length = 0; page.__perr.length = 0; return r }` | `["alert: probe"]`: the guard armed in P2 still records, in a later call and after a full reload. If it returns anything else or throws (`page.__dlg` undefined), the guard does not survive: then **don't skip step 1 in any stage** (each stage arms it again) and say so in the report's notes |
| P4 | Run `browser_console_messages` (level `error`) | At most the `favicon.ico` 404. These messages are the baseline: a group's console check counts only messages not listed at an earlier check |

## Stages

Every stage skips its step 1 (the guard is the prelude's) unless P3 said otherwise; every other step runs as written. In a stage whose last step is "check D, then `browser_console_messages`", only check D runs there, except at the end of a group (stages 8, 15, 16), where the console is read for the whole group. No navigation inside groups A and B is a full page load (all are `{{base_url}}/#…`), so the console holds every message of the group. A bad message (anything but the `favicon.ico` 404; any "Refused to …" or Content-Security-Policy report) fails the stage that reads it; its `failure_reason` quotes the message with its URL and names the group's stage whose view first loads that URL (the hostile call's minutes frame, `/minutes/<the HOSTILE-61 folder>`, is first loaded by stage 9 step 9).

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `hostile/TC-002_meetings-calendar-and-coming-up.md` | — | Skip step 1. Step 18: check D only. Skip its teardown: keep Your day (step 16) for stage 2 |
| 2 | `hostile/TC-011_inbox-found-for-you-and-needs-you.md` | — | On stage 1's view (Your day): skip steps 1–2, check step 2's expected result on one snapshot. Step 16: check D only. Teardown: close the right panel; skip the navigation |
| 3 | `hostile/TC-010_threads-and-clients.md` | — | Skip step 1. Step 19: check D only. Skip its teardown (the next stage navigates) |
| 4 | `hostile/TC-009_people-and-organisations.md` | — | Skip step 1. Step 20: check D only. Skip its teardown |
| 5 | `hostile/TC-008_work-items.md` | — | Skip step 1. Step 19: check D only. Teardown: close the Details panel only; skip the navigation |
| 6 | `hostile/TC-012_plugins-and-marketplaces.md` | — | Skip step 1. Step 18: check D only. Skip its teardown |
| 7 | `hostile/TC-013_skills-templates-and-meeting-types.md` | — | Skip step 1. Step 18: check D only. Skip its teardown (step 16 already left the template editor) |
| 8 | `hostile/TC-014_vocabulary.md` | — | Skip step 1. Step 11 as written: check D, then the console for **group A** (stages 1–8). Its teardown as written |
| 9 | `hostile/TC-001_call-titles-and-folders.md` | — | Skip step 1. Step 19: check D only. Skip its teardown |
| 10 | `hostile/TC-005_minutes-in-the-frame.md` | — | Skip step 1; step 2 opens the hostile call for stages 10–14 (right panel closed, sidebar folded: its Preconditions). Step 11: check D only. Skip its teardown: keep the call open on its Minutes tab |
| 11 | `hostile/TC-007_my-notes.md` | — | On stage 10's view (the hostile call, Minutes tab, right panel closed): skip steps 1–2, check step 2's expected result on one snapshot (if the call isn't open, do step 2). Step 13: check D only. Teardown: make sure My notes is closed; skip the navigation |
| 12 | `hostile/TC-003_transcript-and-speakers.md` | — | On stage 11's view (the hostile call, My notes closed): skip steps 1–2 as in stage 11. Step 12: check D only. Teardown: clear the Find box; skip the navigation |
| 13 | `hostile/TC-004_meeting-chat.md` | — | On stage 12's view (the hostile call, Transcript tab): skip steps 1–2 as in stage 11. Step 10: check D only. Skip its teardown |
| 14 | `hostile/TC-015_live-notes-and-run-outcomes.md` | — | On stage 13's view (the hostile call, Chat tab): skip steps 1–2 as in stage 11. Step 12: check D only. Skip its teardown (the next stage navigates) |
| 15 | `hostile/TC-016_ai-profiles.md` | — | Skip step 1. Step 12 as written: check D, then the console for **group B** (stages 9–15). Its teardown as written |
| 16 | `hostile/TC-006_minutes-page-on-its-own.md` | — | Skip step 1 (step 2's `fetch` runs from the window page stage 15 left). Step 8 as written: check D, then the console for **group C** (this stage: the standalone page and the reload back). Skip its teardown (step 7 is on `#today`) |
| 17 | `today/TC-011_needs-you-now-actions-are-icons-named-in-their-labels.md` | — | After stage 16: its own step 1 navigates to `#today` (Needs you now's rows from the hostile data); the guard from the prelude stays armed |

## Notes
- The guard is armed once (prelude) and every stage ends with check D, which reads and empties what the guard recorded: a dialog or page error fails the stage it happened in. A stage never depends on another: all of them only read. If P3 shows the guard doesn't survive, every stage arms it again (its step 1).
- The console is read three times, once per group of pages (group A: the window's own views, stages 1–8; group B: the hostile call with its minutes frame, stages 9–15; group C: the standalone minutes page, stage 16), not after every stage: one message would otherwise fail every later stage again. The `favicon.ico` 404 is the only message allowed. A "Refused to execute inline event handler" or other Content-Security-Policy report is a FAIL, not a pass (markup reached the DOM): (the hostile minutes' mermaid label logged one until #112).
- Stages 10 and 11 read the minutes frame through `frameLocator` in `browser_run_code_unsafe`; keep the right panel closed and the sidebar folded for them.
- The data's times are relative to the build: build it right before the run (stage 1 needs the hostile meeting still to come, one earlier today).
- Do not press Trust, Ignore, Import, Create, Install, Edit, Record, Write minutes or Send to Claude anywhere in this scenario.
