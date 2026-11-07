---
id: TC-009
title: Past six learned terms a filter box appears
priority: low
type: functional
requirements: [F-VOC-03]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, learned]
---

# TC-009: Past six learned terms a filter box appears

**Module:** Vocabulary | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash (`. <dir>/env` first) run `C=$(ls "$ECHOCTL_HOME/calls" | grep acme-weekly-sync) && echoctl vocab add --call "$C" "Berlin pilot" "Okta SSO" "Entra ID" "SCIM" "Zero trust" "Data residency"` | Prints `learned from <folder>: ` with the six terms |
| 2 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title | Under "Learned from calls" the count is `6`; there is no box "Filter learned terms" |
| 3 | Run `echoctl vocab add --call "$C" "Pen test"`, then close and reopen the panel (or reload) | The count is `7`; a box "Filter learned terms" shows |
| 4 | Type `okta` in "Filter learned terms" | Only the row "Okta SSO" shows |
| 5 | Type `nomatch` in the box | The text "Nothing matches." shows |

## Expected Final State
The filter narrows the learned terms.

## Teardown
- `echoctl vocab remove "Berlin pilot" "Okta SSO" "Entra ID" "SCIM" "Zero trust" "Data residency" "Pen test"`.
