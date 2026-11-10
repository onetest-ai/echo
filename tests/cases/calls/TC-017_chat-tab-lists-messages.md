---
id: TC-017
title: The Chat tab lists each message with sender, time and attached file names
priority: medium
type: functional
requirements: [F-CALLS-14]
module: calls
run: agent
platform: any
data: standard
tags: [calls, chat]
---

# TC-017: The Chat tab lists each message with sender, time and attached file names

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Messages |
|---|---|
| Acme weekly sync (Today) | Dana Lee · before the call · "Agenda attached, see you in a minute" · agenda.docx / Omar Haddad · 1:35 · "Questionnaire link: https://acme.example/q" / Facilitator · 10:10 · "Recording notes are on" |
| Globex pilot kickoff | Kim Park · 0:12 · "Plan template?" · pilot-plan.xlsx |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then the tab "Chat" | Three `article.msg` are shown, in this order |
| 2 | Read the first message | Sender "Dana Lee"; its `time` says "before the call"; the text "Agenda attached, see you in a minute"; the files line "agenda.docx" |
| 3 | Read the second message | Sender "Omar Haddad"; time "1:35"; the text "Questionnaire link: https://acme.example/q"; no files line |
| 4 | Read the third message | Sender "Facilitator"; time "10:10"; the text "Recording notes are on" |
| 5 | Click the row Globex pilot kickoff (the Chat tab stays) | One message: "Kim Park", "0:12", "Plan template?", files "pilot-plan.xlsx" |

## Expected Final State
Globex pilot kickoff's Chat tab is open with its one message.

## Notes
- Creates or changes nothing.
