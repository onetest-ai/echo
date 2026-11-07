---
id: TC-004
title: A term is removed from Every call with its ×
priority: high
type: functional
requirements: [F-VOC-02]
module: vocabulary
run: agent
platform: any
tags: [vocabulary]
---

# TC-004: A term is removed from Every call with its ×

**Module:** Vocabulary | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `vocabulary`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title, type `Kubernetes` in "Add a term" and press Enter | A chip "Kubernetes" shows |
| 2 | Click the × of the chip (button "Remove Kubernetes") | The chip is gone; "None yet." shows under "Every call" |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"vocabulary"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"vocabulary": [],` |

## Expected Final State
Every call is empty again.
