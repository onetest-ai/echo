---
id: TC-003
title: A term is added to Every call and saved
priority: high
type: functional
requirements: [F-VOC-02]
module: vocabulary
run: agent
platform: any
tags: [vocabulary]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-003: A term is added to Every call and saved

**Module:** Vocabulary | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `vocabulary`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title | The panel shows "None yet." under "Every call" |
| 2 | Type `Kubernetes` in "Add a term" and click "Add" | A chip "Kubernetes" with a × shows under "Every call"; the box is empty |
| 3 | Type `Berlin pilot` and press Enter | A second chip "Berlin pilot" shows |
| 4 | Wait 1 s, run `. <dir>/env && grep -n '"vocabulary"' "$ECHOCTL_HOME/.echo/config.json" -A4` | The list holds `"Kubernetes"` and `"Berlin pilot"` |
| 5 | In Bash run `echoctl vocab list`, then step 4's `grep` again | Prints `always (2): Kubernetes, Berlin pilot` and `learned (0):`, no `moved … learned terms` line; `config.json` still holds both under `"vocabulary"` (#106: a CLI load moved them to the learned terms) |
| 6 | Reload the page and open the Vocabulary panel again | Both chips are still there under "Every call"; "Learned from calls" has no "Kubernetes" or "Berlin pilot" row |

## Expected Final State
Two terms are in Every call and in `config.json`.

## Teardown
- Click the × of each chip in the panel ("Remove Kubernetes", "Remove Berlin pilot").
