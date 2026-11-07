---
id: TC-017
title: Found for you with nothing found: no badge, an explanation, and the New thread row
priority: medium
type: functional
requirements: [F-THR-03, F-THR-18]
module: threads
run: agent
platform: any
tags: [threads, found]
---

# TC-017: Found for you with nothing found: no badge, an explanation, and the New thread row

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- No two calls outside every thread share a series, an organisation or a chat (the test data)

## Test Data

| Field | Value |
|-------|-------|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#projects` | The button "Found for you" is in the left list with no count badge (no `.pj-badge`); the Threads activity button has no badge |
| 2 | Click "Found for you" | The URL ends with `#found`; heading "Found for you" with the text "From your calls of the last 90 days that are in no thread — a recurring meeting, one organisation's calls, one Teams chat. Mail joins later." |
| 3 | Read the page body | The row with the input "New thread name" and "Create"; the text "Nothing found right now. Echo suggests a thread when two or more calls that are in none share a meeting series, one outside organisation, or a Teams chat."; no card with "Accept" / "Reject" |

## Expected Final State
Found for you shows its empty state and no badge.

## Teardown
- None needed

## Changes
- Nothing: read only.

## Notes
Accept, Reject, Add to a thread and the count badge need two or more unthreaded calls that share a series or organisation; the test data has only one (Initech planning).
