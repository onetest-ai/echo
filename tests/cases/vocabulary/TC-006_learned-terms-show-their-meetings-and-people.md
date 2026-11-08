---
id: TC-006
title: Learned from calls lists each term with its meeting, people and calls
priority: medium
type: functional
requirements: [F-VOC-03]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, learned]
---

# TC-006: Learned from calls lists each term with its meeting, people and calls

**Module:** Vocabulary | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** the learned terms (`<data>/.echo/` taxonomy)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash (`. <dir>/env` first) run `C=$(ls "$ECHOCTL_HOME/calls" | grep acme-weekly-sync) && echoctl vocab add --call "$C" "Berlin pilot" "Okta SSO"` | Prints `learned from <folder>: Berlin pilot, Okta SSO` |
| 2 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title | Under "Learned from calls" the count is `2` and two rows show, "Berlin pilot" then "Okta SSO" (alphabetical) |
| 3 | Read the row "Okta SSO" | Its buttons are "Always" and "Remove"; its line reads "Meetings: acme weekly sync · People: dana@acme.example, omar@acme.example · 1 call" |
| 4 | Read "Every call" | "None yet." |

## Expected Final State
Two learned terms with their scope; Every call is empty.

## Teardown
- `echoctl vocab remove "Berlin pilot" "Okta SSO"`.
