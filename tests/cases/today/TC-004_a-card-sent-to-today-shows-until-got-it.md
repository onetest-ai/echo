---
id: TC-004
title: A card sent to Today (a routine's message, echoctl deliver send today) shows until Got it, and comes back when its source sends again
priority: low
type: functional
requirements: [F-TODAY-05]
module: today
run: agent
platform: any
data: standard
tags: [today, cards, deliver]
---

# TC-004: A card sent to Today (a routine's message, echoctl deliver send today) shows until Got it, and comes back when its source sends again

**Module:** Today | **Priority:** Low | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- A card is what a routine or a plugin sends with the `today` way (`deliver::send("today", …)`); `echoctl deliver send today` sends one the same way, without a model. The text comes on stdin (`--file /dev/stdin`)
- Today's cards are `section.td-card` without the class `nd` (that one is "Needs you now"); none on fresh data

## Test Data

| Field | Value |
|---|---|
| Title | Morning brief |
| About (its source) | Routine · Weekdays at 08:30 |
| Text | `## Before the Acme demo` / `1. **Send the deck** to Dana` / `2. Check the SSO checklist` |
| Model | `--ai sonnet` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl deliver status` | `"today": true` (Today is always a way to send) |
| 2 | Run `printf '## Before the Acme demo\n\n1. **Send the deck** to Dana\n2. Check the SSO checklist\n' \| target/debug/echoctl deliver send today --title "Morning brief" --about "Routine · Weekdays at 08:30" --ai sonnet --file /dev/stdin` | Prints "Sent" |
| 3 | Reload `{{base_url}}/#today` and wait for "Your day" | At the top (under Needs you now) a card: the title "Morning brief", "Routine · Weekdays at 08:30 · <the time sent>", a button "Got it"; the text as HTML: a heading "Before the Acme demo", a numbered list with "**Send the deck** to Dana" (bold) and "Check the SSO checklist"; under it "Written by AI (Claude, sonnet) from Echo's data — it can contain mistakes. Check what matters before you act on it." |
| 4 | Click "Got it" | The card is gone |
| 5 | Reload `{{base_url}}/#today` | No card; `$ECHOCTL_HOME/.echo/today-cards.json` has the card with `"seen": true` |
| 6 | Run `printf '## Before the Acme demo\n\n- The deck went out\n' \| target/debug/echoctl deliver send today --title "Morning brief" --about "Routine · Weekdays at 08:30" --file /dev/stdin`, reload Today | The card is back with the new text "The deck went out" and no AI line (no `--ai`); one card only (the same source replaces its card) |
| 7 | Click "Got it" | The card is gone |

## Expected Final State
No card shows on Today; `today-cards.json` holds the source's last card, seen.

## Notes
- A plugin's own way to send, and a routine run on its schedule, need a plugin or a model run: not here. They end in the same `put_card`.
