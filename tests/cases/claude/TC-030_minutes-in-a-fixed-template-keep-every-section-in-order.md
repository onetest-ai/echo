---
id: TC-030
title: Minutes in a template that keeps every section have exactly its sections in order, past the heading cap, with the user's heading inside one and the end time in the header (a real run on the local model)
priority: high
type: functional
requirements: [F-MIN-23]
module: claude
run: agent
platform: macos
data: standard
ai: local
tags: [claude, minutes, templates, local-model]
---

# TC-030: Minutes in a template that keeps every section have exactly its sections in order, past the heading cap, with the user's heading inside one and the end time in the header

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model"); the fake `claude` passes only runs routed to the default AI profile, so no Claude account is used
- **One model run** (started in step 5), about 1–2 minutes on the network test server, 8–10 on a Mac running the model itself. Assert files, states and headings, never the model's wording; no retry unless the run failed for an infrastructure reason (then BLOCKED)
- `<call>` below is today's Acme weekly sync: the last folder of `ls $ECHOCTL_HOME/calls | grep -- '-acme-weekly-sync$'`. It has chat (3 messages) and two shared screens; its made-up transcript (`ACME_SYNC_TODAY` in `tests/fixtures/make_data.py`) discusses the Berlin rollout and has no hiring decision, no board meeting and no office move
- **In S-30** `<call>` is read once, in the scenario's prelude (step P2); on its own, read it as above

**Changes:** `<data>/.echo/templates/fixed-check.md`, the call's `my-notes.md`, `notes.md`, `notes-run.json`, `meta.json` (`template`)

## Test Data

| Section of "Fixed check", in order | Its words | Expected in the minutes |
|---|---|---|
| Summary | One line on what the meeting was for | there |
| Decisions | Each decision in one line | there |
| Action items | `- [ ] owner — task` | there |
| Open questions | Unresolved points | there |
| Hiring | Hiring decisions that were made | there, body `_None recorded._` |
| Board meeting | The date of the next board meeting, nothing else; if none was set, write: Not scheduled | there, body `Not scheduled` |
| Berlin | Only when the Berlin rollout was discussed: where it stands | there (the condition holds) |
| Office move | Only when an office move was decided: what | absent (the condition doesn't hold) |
| Appendix: On screen | Only when screens were shared: one bullet per screen, `[mm:ss]`, its file name and what it showed | there (the call has frames) |

If `heading_budget` (step 1) is more than 4, add unconditional sections (`Risks`, `Next steps`, `Owners`, …, each "What was said about it") after Open questions until there are `heading_budget` + 2 unconditional sections.

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl context <call>` | JSON; note `heading_budget` (a number, N); `template.name` is `status-sync` (the title's "weekly" / "sync" pick Status sync); `template.fixed` is `false`; `ended_at` is set |
| 2 | Navigate to `{{base_url}}/#settings/templates`, click "New template", name it `Fixed check`; in the header card replace the text with `# {title}` and a second line `**Ended:** {end}`; replace the sections with the Test Data's sections (headings and words, in that order) | The form holds the header and the sections |
| 3 | Tick "Keep every section" and click "Create" | A row "Fixed check" with the tag "Yours" reads "… sections, all kept" |
| 4 | Open `<call>` in Calls, its My notes, "Edit as text" (`textarea#my-notes`); at the end add the lines `# Fixed topic` and `- Ask Dana about the checklist`; wait for "Saved"; run the command of step 1 | `user_headings` contains `Fixed topic`; `template.name` is still `status-sync` |
| 5 | In the call's Minutes tab open "Call actions", under "Write again with" click "Fixed check" | The menu closes; within 15 seconds the tab shows "Updating the minutes…" |
| 6 | Wait until `cat $ECHOCTL_HOME/calls/<call>/notes-run.json` shows a `state` other than "queued"/"running" (every 30 seconds, up to 20 minutes) | `state` "done" |
| 7 | Run the command of step 1 | `template.name` `fixed-check`, `template.fixed` `true`, `heading_budget` `null`, `ended_at` as in step 1 |
| 8 | Run `grep -n '^## ' $ECHOCTL_HOME/calls/<call>/notes.md` | In this order: Summary, Decisions, Action items, Open questions (and the added unconditional ones), Hiring, Board meeting, Berlin, Appendix: On screen; then at most one more heading, one containing "In the chat" (the template has none of its own, so it can only come after its last section); no `## Office move`, no `## Fixed topic`, no other `##` heading |
| 9 | Read `notes.md` | The line after `## Hiring` (blank lines aside) is `_None recorded._`; after `## Board meeting` it is `Not scheduled`; a line `### Fixed topic` is there; the header has `**Ended:** <HH:MM>` with the local time of `ended_at` |

## Expected Final State
The call's minutes follow Fixed check: every unconditional section in its order, the false conditional one left out, the user's heading as a topic inside a section.

## Teardown
- In `{{base_url}}/#settings/templates` open "Fixed check" and click "Delete".

## Notes
- The run may move the call's uncited frames to Echo's trash (existing behaviour); nothing after this case in S-30 reads them.
