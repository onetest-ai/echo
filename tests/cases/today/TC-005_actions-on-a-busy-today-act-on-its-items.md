---
id: TC-005
title: ⌘K on a busy Today lists its meetings' preps and its tasks, and Enter acts on the item: Show prep opens it, Close asks first
priority: low
type: functional
requirements: [F-TODAY-06]
module: today
run: agent
platform: any
data: standard
tags: [today, palette]
---

# TC-005: ⌘K on a busy Today lists its meetings' preps and its tasks, and Enter acts on the item: Show prep opens it, Close asks first

**Module:** Today | **Priority:** Low | **Type:** Functional

## Preconditions
- Standard data set built right before the run (and the run before 21:00 local: the meetings below must still be to come): `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- The palette: `[data-pal-open]` or `ControlOrMeta+k`; input `[aria-label="Action"]`; list `#pal-list`; dialog `.pal`
- **In S-29** Your day is already on screen, left by `today/TC-004` (the stage before), so step 1's navigation is skipped. On its own, step 1 navigates

## Test Data

| Item | Group | Enter does |
|---|---|---|
| Globex sponsor update (37 minutes after the build, with the other meetings to come; prep written) | TODAY: "Show prep: Globex sponsor update" | opens `#upcoming/e11de173ab857a8b` |
| Meetings still to come without a prep | TODAY: "Prepare: <subject>" | starts a prep run (not pressed) |
| Needs you now: Dana Lee (Teams 1:1), Kim Park (mail), Lena Vogel (@mention) | NEEDS YOU NOW: "Reply in Teams: …" / "Read and reply: …", "Claude: …", "I'll do it: …", "Ignore: …", "Won't do: …" | not pressed |
| Book the workshop room (I owe, due this week) | MY WORK: "Close: Book the workshop room", "Claude: Book the workshop room" | Close asks first ("Close this task?"); Claude starts Claude Code (not pressed) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day", press `ControlOrMeta+k` | The dialog with the chip "On: Today ×"; groups NEEDS YOU NOW, TODAY, MY WORK, ECHO first, in that order (then CLAUDE and INBOX as far as they fit: with no query the list holds its first 50 items only, so on this busy day it ends before GO TO); NEEDS YOU NOW holds "Reply in Teams", "Read and reply", "Claude", "I'll do it" and "Won't do" items for the three items of Needs you now ("Reply in Teams: Dana Lee — Can you send me the renewal price list for Acme?" first); TODAY holds "Show prep: Globex sponsor update" and "Prepare: …" items only for meetings that haven't ended (the live ones, begun before now, too; none for a meeting already over); MY WORK holds a "Close: <title>" and a "Claude: <title>" for each task of "My work · this week", "Close: Book the workshop room" first |
| 2 | Type `sponsor` | The list holds the group TODAY with one item "Show prep: Globex sponsor update" |
| 3 | Press Enter | The palette closes; the hash is `#upcoming/e11de173ab857a8b`; the heading "Globex sponsor update", the Prep tab with "IN SHORT" |
| 4 | Navigate to `{{base_url}}/#today`, press `ControlOrMeta+k`, type `Close: Book the workshop`, press Enter | The palette closes; a dialog "Close this task?" with "Book the workshop room", the box "A comment, if you like — why it's done, or where", "Cancel" and "Close it" |
| 5 | Click "Cancel" | The dialog closes; "Book the workshop room" is still in "My work · this week" |

## Expected Final State
Nothing changed: the prep was only shown, the close was cancelled.

## Notes
- The quiet day, with no items to act on, is `TC-006`.
- Prepare, Claude and "Close it" start Claude Code or change a task: not pressed (Claude's dialog is `tasks/` cases).
