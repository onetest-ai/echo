---
id: TC-014
title: Details → Who does it switches Claude can / By hand and the CLI shows the mode
priority: medium
type: functional
requirements: [F-WORK-12]
module: work
run: agent
platform: any
tags: [work, details, mode]
---

# TC-014: Details → Who does it switches Claude can / By hand and the CLI shows the mode

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `echoctl project tasks` shows `"mode": null` for T001

## Test Data

| Field | Value |
|-------|-------|
| Task | T001 Send Acme the SSO checklist |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T001 | The row "Who does it" with two icon buttons, named "Claude can" and "By hand" by their `aria-label` and tooltip (`data-mode-set`), no text inside them; the switch stays on one line |
| 2 | Click "By hand". Wait up to 3 s after an action for the window to redraw before reading. | The button "By hand" is the selected one |
| 3 | Run `echoctl project tasks` | T001 of `acme-rollout` has a `mode` object whose `mode` is `manual` |
| 4 | Navigate to `{{base_url}}/#tasks`, choose "Who does it" `By hand` | The pile "I owe" shows the card "Send Acme the SSO checklist"; choosing `Claude can do` leaves "I owe" at 0 ("Nothing here") |

## Expected Final State
T001 is set to By hand.

## Teardown
- Choose "Who does it" `Anyone` on Tasks
- `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T001","set":{"mode":null}}'`, then `echoctl project tasks`: T001 of `acme-rollout` has `"mode": {"by": "you", "mode": null}` (no one chosen). The window cannot undo the choice: "Claude can" and "By hand" each set their own mode, and clicking the pressed one again sets it again

## Changes
- Sets the mode of T001.

## Notes
"Send to Claude" is never pressed (it starts Claude Code). The faded guess and the prompt text depend on Claude; only the toggle is checked.
