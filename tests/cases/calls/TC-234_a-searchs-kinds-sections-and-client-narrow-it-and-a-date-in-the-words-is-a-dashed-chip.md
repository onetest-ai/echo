---
id: TC-234
title: A search's kinds, a section and a client narrow it as chips, and a date in the words is a dashed chip that takes them out when removed
priority: high
type: functional
requirements: [F-CALLS-21]
module: calls
run: agent
platform: any
tags: [calls, search, recall, filters]
---

# TC-234: A search's kinds, sections and client narrow it; a date in the words is a dashed chip

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- As TC-233: standard data served at `{{base_url}}`, Recall on and synced (`brain set --enabled true`, `brain sync`, run
  with the data's env and `ECHOCTL_CONFIG_DIR` checked inside `<dir>`), the page loaded again
- Intake's facts of the standard set: the mail "Re: SSO cutover window" and the post "Acme › Okta and SSO", both Acme's

## Test Data

| Field | Value |
|-------|-------|
| Words | sso cutover, then sso this week |
| A Summary line this week | Acme weekly sync (today): "SSO for the Berlin pilot is on track" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `sso cutover`, press Enter | "Said in calls, mail, channels · N"; among the sources "Re: SSO cutover window" (mail icon) and "Acme › Okta and SSO" (channel icon) |
| 2 | Click the Mail icon (tooltip "Mail") | It shows pressed (violet); the group reads "Said in mail · 1" with only "Re: SSO cutover window"; its moment "Mail · Dana Lee · <day>" marks "SSO cutover" |
| 3 | Click the Mail icon again, clear the box, type `sso this week`, press Enter | A dashed chip "Since <this week's Monday>" (e.g. "Since Nov 16") appears; hovering it reads "Read from “this week” in your words"; every source's day is in this week |
| 4 | Click the Calls icon | "Said in calls · N": only call sources |
| 5 | Click the filter icon ("Filters"), then "Summary" under Section | A violet chip "Summary" with × appears; every moment listed says "Summary"; "Acme weekly sync" (today) is listed with "SSO for the Berlin pilot is on track" |
| 6 | In the still open menu click "Acme" under Client | The menu closes; a violet chip "Acme" appears; every source row names Acme |
| 7 | Click × on the "Summary" chip | The chip goes; the moments are of every section again, still Acme's calls this week |
| 8 | Click × on the dashed date chip | The chip goes and the box reads `sso`; the sources are no longer limited to this week |

## Expected Final State
Each filter narrowed the search and showed as a chip; the guessed date came from the words and left with them.
