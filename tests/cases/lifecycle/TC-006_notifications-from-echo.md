---
id: TC-006
title: Notifications come from Echo and open its window
priority: high
type: functional
requirements: [F-LIFE-06]
module: lifecycle
run: agent
platform: macos
tags: [lifecycle, notifications]
external_id: TC-LIFE-006
user_steps:
  - 'at the notifications prompt the user clicks Allow when the run asks'
  - 'the user confirms each banner''s icon (or grants the terminal Screen Recording, their choice)'
  - 'the user turns Echo''s notifications off in System Settings when the run asks'
---

# TC-006: Notifications come from Echo and open its window

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Echo opened for the first time on a Mac where it has never been (S-15); `E=/Applications/Echo.app/Contents/MacOS/echoctl`
- **A notification Echo makes anyway (shared in S-15):** in S-15, `cli/TC-003`'s "Command-line tool installed" (its step 1): steps 2–3 are done on it there, and nothing is installed again for this case. On its own: any of those step 2 names

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** when the notifications prompt for Echo appears, click Allow when the run asks | — |
| 2 | Trigger one with another app in front (no banner is shown for the app in front): "Command-line tool installed" or "Claude Code connected" (cli/TC-003, claude/TC-001), or the model-ready one | A banner, or the item in Notification Center, with Echo's icon (purple rounded square with the octopus), not a blank one: the user confirms |
| 3 | **The user does:** click it (the banner or the Notification Center item) before dismissing it | The Echo window opens |
| 4 | **The user does:** turn Echo's notifications off in System Settings → Notifications when the run asks; then the run triggers one again: `$E notify "Echo" "Notifications test"` (the app's way of posting one; run it once before turning them off too, to see the banner it makes), or as in step 2 | Nothing shows (the user confirms) |

## Expected Final State
Echo's notifications carry its icon and open Echo.
