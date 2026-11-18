---
id: TC-237
title: A search of days past what the index keeps says "Older than the index keeps" with each kind's horizon
priority: medium
type: functional
requirements: [F-CALLS-21, F-CALLS-23]
module: calls
run: agent
platform: any
tags: [calls, search, recall, retention]
---

# TC-237: Days past the index say "Older than the index keeps"

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-233: standard data served at `{{base_url}}`, Recall on and synced, the page loaded again

## Test Data

| Field | Value |
|-------|-------|
| Words | acme kickoff 14 July |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `acme kickoff 14 July`, press Enter | A dashed chip "Jul 14" (tooltip "Read from “14 July” in your words") |
| 2 | Read the list | The group "Said in calls, mail, channels" holds one grey line with a clock icon: "**Older than the index keeps.** Transcripts stay 45 days (since <today − 45 days>), minutes and mail 90 (since <today − 90 days>). Call cards stay, so titles and people still match."; no "Nothing … says" line and no error |

## Expected Final State
A search before what the index keeps says so in one line, with the dates each kind reaches back to.
