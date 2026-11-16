---
id: TC-001
title: The Vocabulary panel opens from the book button beside Calls
priority: medium
type: functional
requirements: [F-VOC-01]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, panel]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-001: The Vocabulary panel opens from the book button beside Calls

**Module:** Vocabulary | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/#calls` (a fresh load) and click the book button "Vocabulary" beside the list title | The load opens the newest call and rewrites the URL to `#call/<its folder>/minutes`; after the click the right panel opens with the tab "Vocabulary" selected |
| 2 | Read the panel | "Names and terms the transcription should hear right."; the heading "Every call" with "None yet."; a box "Add a term" and a button "Add"; the heading "Learned from calls" with the count `0` and "None yet — the minutes add the names they had to fix." |
| 3 | Click "Close the panel" | The right panel closes |

## Expected Final State
The empty vocabulary panel was shown and closed.
