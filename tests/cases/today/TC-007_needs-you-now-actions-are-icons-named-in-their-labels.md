---
id: TC-007
title: Today → Needs you now — each row's actions are icons named in their label and tooltip, and they act
priority: medium
type: functional
requirements: [F-TODAY-04]
module: today
run: agent
platform: macos
data: hostile
tags: [today, needs-you-now, layout]
---

# TC-007: Today → Needs you now — each row's actions are icons named in their label and tooltip, and they act

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Hostile data built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set hostile` (its `.echo/needs.json` has a mail item and a Teams item); the window is served at `{{base_url}}`, 1201×868

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today` and wait for "Needs you now" | Two rows (`section.nd .nd-row`) |
| 2 | Run `browser_evaluate` with `() => [...document.querySelectorAll('section.nd .nd-a')].map((a) => ({ w: Math.round(a.getBoundingClientRect().width), names: [...a.querySelectorAll('button')].map((b) => [b.getAttribute('aria-label'), b.title, b.innerText.trim()]) }))` | Each row's actions are at most 100 px wide; the mail row's buttons are `["Read and reply","Read and reply",""]`, `["I'll do it","I'll do it",""]`, `["Won't do","Won't do",""]`; the Teams row's first is `["Reply in Teams","Reply in Teams",""]` (no text inside any button, each an icon) |
| 3 | Click the first row's "I'll do it" (`[data-needs-todo]`, the first) | The dialog "A task of yours" opens with the item's words as the task's title |
| 4 | Click "Cancel" | The dialog closes; both rows are still listed |

## Expected Final State
Nothing changed: the dialog was cancelled.

## Teardown
- `tests/fixtures/serve.sh down <dir>`; leave one blank tab

## Notes
- The actions were text buttons ("Read and reply", "I'll do it", "Won't do") that took a third of the row and squeezed what was said; they are icons now, their names in `aria-label` and the tooltip (as the "Who does it" switch).
- "Reply in Teams" and "Read and reply" open Teams or a mail dialog that needs a sign-in: not clicked here (`hostile/TC-011`).
