---
id: TC-009
title: Each row's icon shows whether the call has minutes or only a transcript
priority: medium
type: functional
requirements: [F-CALLS-02]
module: calls
run: agent
platform: any
tags: [calls, list]
---

# TC-009: Each row's icon shows whether the call has minutes or only a transcript

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Calls | Acme weekly sync, Globex pilot kickoff (minutes); Initech planning (transcript) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" | The list has the groups Today and Yesterday |
| 2 | Read the icon of "Acme weekly sync" (a `.si` element with an `aria-label`) | Its label is "Minutes" |
| 3 | Read the icon of "Initech planning" | Its label is "Transcript" |
| 4 | Read the icon of "Globex pilot kickoff" | Its label is "Minutes" |

## Expected Final State
The three rows carry the labels Minutes, Transcript and Minutes.

## Notes
- Creates or changes nothing. The recording, transcribing, failed, audio, memo and Teams-transcript icons need other calls: `standard` data set.
