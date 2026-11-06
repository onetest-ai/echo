---
id: TC-005
title: The same term typed twice makes one chip
priority: low
type: functional
requirements: [F-VOC-02]
module: vocabulary
run: agent
platform: any
tags: [vocabulary]
---

# TC-005: The same term typed twice makes one chip

**Module:** Vocabulary | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `vocabulary`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title, type `Okta SSO` in "Add a term" and press Enter | One chip "Okta SSO" |
| 2 | Type `Okta SSO` again and press Enter | Still one chip "Okta SSO" |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"vocabulary"' "$ECHOCTL_HOME/.echo/config.json" -A3` | The list holds `"Okta SSO"` once |

## Expected Final State
One chip, one entry.

## Teardown
- Click the × of the chip "Okta SSO".
