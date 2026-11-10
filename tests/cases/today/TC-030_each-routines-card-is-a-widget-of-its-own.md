---
id: TC-030
title: Each routine that sends Today a card is a widget of its own (core:routines.card.<name>, under Routines) — put where you want it, its card drawn there; Routine cards shows the rest; Got it hides it until the next
priority: medium
type: functional
requirements: [F-TODAY-20]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, routines]
---

# TC-030: Each routine's card is a widget of its own

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Write two short Markdown files; `target/debug/echoctl deliver send today --title "Morning brief" --file <a.md> --about "Routine · Morning brief"` and the same with "Friday review" / "Routine · Friday review" | Each prints "Sent" |
| 2 | `target/debug/echoctl widget list` | Two widgets `core:routines.card.routine-morning-brief-<hash>` and `…routine-friday-review-<hash>`, part "Routines", labelled by their source, sizes M and L |
| 3 | `target/debug/echoctl today add <the morning brief's id> --size M --before core:needs.now`; open `{{base_url}}/#today` | "Morning brief" in Next meeting's row, beside it (Needs you now wraps to the line under it: the row is full), with Got it; "Friday review" in Routine cards below — the brief not there twice |
| 4 | Click Got it on Morning brief | It is gone (its widget has nothing to show); Friday review stays |
| 5 | `deliver send today` the brief again (same `--about`); reload | The new brief shows in its own place again |
| 6 | `target/debug/echoctl today reset`; reload | Both cards in Routine cards |

## Notes
- The names and the data are the Rust test `today::each_card_source_is_a_widget_of_its_own_with_its_card`.
