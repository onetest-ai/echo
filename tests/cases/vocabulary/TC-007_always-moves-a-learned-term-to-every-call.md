---
id: TC-007
title: Always moves a learned term to Every call
priority: medium
type: functional
requirements: [F-VOC-03]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, learned]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-007: Always moves a learned term to Every call

**Module:** Vocabulary | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the learned terms "Berlin pilot" and "Okta SSO" (from Acme weekly sync), the Vocabulary panel open on them. In S-11: made by stage 33 (vocabulary/TC-006, its teardown skipped), so step 1 and step 2's reload and book button are skipped there. On its own: steps 1–2 make it.

**Changes:** `<data>/.echo/config.json` key `vocabulary`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash (`. <dir>/env` first) run `C=$(ls "$ECHOCTL_HOME/calls" | grep acme-weekly-sync) && echoctl vocab add --call "$C" "Berlin pilot" "Okta SSO"` | Prints `learned from <folder>: Berlin pilot, Okta SSO` |
| 2 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title and click "Always" in the row "Okta SSO" | A chip "Okta SSO" shows under "Every call"; the row "Okta SSO" leaves the learned list; the row "Berlin pilot" stays |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"vocabulary"' "$ECHOCTL_HOME/.echo/config.json" -A3` | The list holds `"Okta SSO"` |

## Expected Final State
Okta SSO is used in every call.

## Teardown
- `echoctl vocab remove "Berlin pilot" "Okta SSO"` (it removes a term from both lists).
