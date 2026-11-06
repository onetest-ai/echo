---
id: TC-008
title: Remove forgets a learned term
priority: medium
type: functional
requirements: [F-VOC-03]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, learned]
---

# TC-008: Remove forgets a learned term

**Module:** Vocabulary | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash (`. <dir>/env` first) run `C=$(ls "$ECHOCTL_HOME/calls" | grep acme-weekly-sync) && echoctl vocab add --call "$C" "Berlin pilot" "Okta SSO"` | Prints `learned from <folder>: Berlin pilot, Okta SSO` |
| 2 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title and click "Remove" in the row "Berlin pilot" | The row is gone; the count under "Learned from calls" is `1`; "Okta SSO" stays |
| 3 | Run `echoctl vocab list` | Prints `learned (1):` with a line for `Okta SSO` and none for `Berlin pilot` |

## Expected Final State
Berlin pilot is forgotten.

## Teardown
- `echoctl vocab remove "Okta SSO"`.
