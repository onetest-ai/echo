---
id: TC-010
title: The dictionary is in Settings → Vocabulary with Intake off, beside the every-call and learned terms, and a term opens to edit
priority: high
type: functional
requirements: [F-VOC-05, F-VOC-01]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, dictionary, settings]
---

# TC-010: The dictionary is in Settings → Vocabulary with Intake off, beside the every-call and learned terms, and a term opens to edit

**Module:** Vocabulary | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`): Intake (Recall) is off
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** a learned term "Okta" in the dictionary (`<data>/recall/taxonomy/`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `. <dir>/env && echoctl vocab add --call "$(ls "$ECHOCTL_HOME/calls" \| grep acme-weekly-sync)" Okta` | It prints `learned from <the folder>: Okta` |
| 2 | Load `{{base_url}}/`, navigate to `{{base_url}}/#settings/vocabulary` and wait for the heading "Vocabulary" | The left list has "Vocabulary" between "Transcription" and "AI & minutes"; the page shows "Every call", "Learned from calls 1" with "Okta", and a card "Dictionary" with a box "Find a term" and a group "Other 1" |
| 3 | Type `Okta` in "Find a term" and wait 1 s | "1 match"; the term "Okta" is listed with "Acme" and "learned" |
| 4 | Click the term "Okta" | The right panel opens on the tab "Topic": heading "Okta", buttons "Rename", "Merge into…", "Define…", "Remove" |

## Expected Final State
The dictionary is read and edited from Settings → Vocabulary while Intake is off.

## Teardown
- Close the panel (`[aria-label="Close the panel"]`).

## Notes
- That the dictionary's terms hint a call's transcription whether Intake is on or off is a Rust test (`projects::tests::the_dictionary_hints_a_meeting_with_intake_off`).
