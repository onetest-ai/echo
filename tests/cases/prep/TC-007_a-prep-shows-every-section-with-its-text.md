---
id: TC-007
title: A written prep shows In short, Expected from you, Last time, Open with these people, New since then, Who's coming and how it was written
priority: high
type: functional
requirements: [F-PREP-04, F-PREP-02]
module: prep
run: agent
platform: any
data: standard
tags: [prep, sections]
---

# TC-007: A written prep shows In short, Expected from you, Last time, Open with these people, New since then, Who's coming and how it was written

**Module:** Prep | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A prep's id is the one in `<dir>/manifest.json` → `preps` (stable for a given meeting id); Coming up today rows are `[data-up="<prep id>"]`

## Test Data

| Field | Value |
|---|---|
| Meeting | Globex sponsor update (to come, 25 min), prep written by `sonnet` |
| In short | Globex sponsor update: the SSO checklist is done, the questionnaire waits for legal, the access list is late. |
| Expected from you | Send the access list reminder · <today> / From last time Share the dry-run numbers / Reply to Dana Lee: Reply to Dana about the checklist |
| Last time · <7 days ago> | The Berlin pilot needs SSO first / Still to do Send the SSO checklist · Me |
| Open with these people | Task Get the access list · Omar Haddad · late / Risk The access list arrives late / Question Is Munich in scope? · <today> |
| New since then | <yesterday> · Mail from Dana Lee: Re: SSO checklist "Thanks, security has it." / <today> · Chat from Omar Haddad: Acme rollout chat "Access list by Monday." |

The dates are the build's: `<today>` its day, `<yesterday>` the day before, `<7 days ago>` the day of last week's Acme weekly sync. The prep writes a date as the browser's locale writes a day and a short month (`toLocaleDateString([], { day: 'numeric', month: 'short' })`): "Nov 10" in en-US, the Playwright browser's locale; "10 Nov" in en-GB.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync", click "Coming up today" and then its row "Globex sponsor update" | The URL becomes `#upcoming/<prep id>`; the header shows the title, the date with weekday and time, "25 min", "Kim Park", a button "Copy link"; the tabs are "Prep" (selected), "Transcript" and "Minutes" (both disabled) |
| 2 | Read the section "In short" | The sentence from Test Data |
| 3 | Read the section "Expected from you" | Three items: "Send the access list reminder" with the date <today> (no "late"); "From last time" "Share the dry-run numbers"; "Reply" to "Dana Lee": "Reply to Dana about the checklist" |
| 4 | Read the section "Last time · <7 days ago>" | The highlight "The Berlin pilot needs SSO first" and "Still to do" "Send the SSO checklist" · "Me" |
| 5 | Read the section "Open with these people" | "Task" "Get the access list" · "Omar Haddad" · "late" (class `warn`); "Risk" "The access list arrives late"; "Question" "Is Munich in scope?" · <today> |
| 6 | Read the section "New since then" | Two lines: the mail from Dana Lee "Re: SSO checklist" with "Thanks, security has it.", and the chat from Omar Haddad "Acme rollout chat" with "Access list by Monday."; only titles, senders and snippets |
| 7 | Read the footer of the side column | "Written <time> · sonnet", the time an hour before the build as the browser's locale writes hours and minutes ("07:32 AM" in en-US, "07:32" in en-GB) and a button "Write it again" (do not press it: it starts Claude Code) |

## Expected Final State
The prep of Globex sponsor update is open with all its sections.

## Notes
- Creates or changes nothing. The six sections come from the first ready prep of the data set (`prep.sections` all present).
