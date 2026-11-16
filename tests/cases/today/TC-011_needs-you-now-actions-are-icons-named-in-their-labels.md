---
id: TC-011
title: Today → Needs you now — Reply shows where it goes (Teams' logo or an envelope), My work's ✦ opens Claude's dialog on the item, and Ignore is an icon beside it
priority: medium
type: functional
requirements: [F-TODAY-04]
module: today
run: agent
platform: macos
data: hostile
tags: [today, needs-you-now, layout]
---

# TC-011: Today → Needs you now — Reply shows where it goes (Teams' logo or an envelope), and My work's ✦ opens Claude's dialog on the item

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Hostile data built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set hostile` (its `.echo/needs.json` has a mail item and a Teams item); the window is served at `{{base_url}}`, 1201×868

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today` and wait for "Needs you now" | Two rows (`section.nd .nd-row`) |
| 2 | Run `browser_evaluate` with `() => [...document.querySelectorAll('section.nd .nd-a')].map((a) => ({ w: Math.round(a.getBoundingClientRect().width), names: [...a.querySelectorAll('button')].map((b) => [b.getAttribute('aria-label'), b.title, b.innerText.trim()]) }))` | Each row's actions are at most 140 px wide; the mail row's buttons are `["Read and reply","Read and reply","Reply"]` (the word Reply and an envelope) and `["Claude — work on this, or Yolo","Claude — work on this, or Yolo",""]` (My work's ✦, an icon), then `["Ignore — take it off Needs you now","Ignore — take it off Needs you now",""]` (an eye crossed out); the Teams row's first is `["Reply in Teams","Reply in Teams","Reply"]` with Teams' logo (`svg.nd-logo`), then the same ✦ and Ignore |
| 3 | Click the first row's ✦ ("Claude — work on this, or Yolo") | My work's Claude dialog opens: "Claude", the item's source and words where a task's title goes (escaped: the hostile words show as text), the skill field (the skill Echo finds the best fit already in it, or, when none fits, empty with "Just the message — or type to find a skill (N)"), "Work on it", "Yolo", "Cancel" |
| 4 | Click "Cancel" | The dialog closes; both rows are still listed |

## Expected Final State
Nothing changed: the dialog was cancelled.

## Teardown
- `tests/fixtures/serve.sh down <dir>`; leave one blank tab

## Notes
- Reply is the word and where it goes (Teams' logo from Simple Icons 11.15.0, CC0, or a mail's envelope); the other action is My work's ✦ (Claude's dialog on the item, `TC-014`). I'll do it and Won't do are in ⌘K's NEEDS YOU NOW group (`TC-005`, `TC-007`). Every full name is in `aria-label` and the tooltip.
- "Reply in Teams" and "Read and reply" open Teams or a mail dialog that needs a sign-in: not clicked here (`hostile/TC-011`).
