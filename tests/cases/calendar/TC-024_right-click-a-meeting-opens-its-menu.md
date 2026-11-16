---
id: TC-024
title: Right-click on a meeting opens a menu of what its card offers, at the pointer
priority: medium
type: functional
requirements: [F-CAL-16]
module: calendar
run: agent
platform: any
data: standard
tags: [calendar, card, context-menu]
---

# TC-024: Right-click on a meeting opens a menu of what its card offers, at the pointer

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run (and the run before 21:00 local: Globex weekly must still be to come): `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- A meeting block is `[data-ev="<id>"]`; the menu is `.ev-ctx` (`role="menu"`), its items `.menu-item`; the card is `aside.detail`
- Right-click with Playwright's `click({ button: 'right' })` on the block

## Test Data

| Meeting (id) | State | Menu items, in order |
|---|---|---|
| Globex weekly (`ev-globex-weekly`) | to come, Teams link, recurring (not recorded every time), thread Globex pilot, no prep | Record when it starts, Prepare · Add to a thread…, Record every meeting of this series · Copy the Teams link · Show details |
| Acme weekly sync (`ev-acme-sync`) | recorded, recurring (recorded every time) | Open notes · Stop recording every meeting of this series · Show details |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calendar/day`, wait for "Day"; close a card if one is open; right-click the block `[data-ev="ev-globex-weekly"]` | A menu opens at the pointer, fully inside the window: the label "Globex weekly", then the items of the table's first row in that order, separators between the groups; focus is on "Record when it starts"; no card opens |
| 2 | Press ArrowDown twice, then End, then Home; then press Escape | The focus moves to "Prepare", "Add to a thread…", then the last item "Show details", then back to "Record when it starts"; Escape closes the menu, no card opens, and the focus is back on the block `[data-ev="ev-globex-weekly"]` (`document.activeElement`) |
| 3 | Right-click the same block, click "Add to a thread…" | The menu is gone; the card of Globex weekly is open with the "+ Thread" menu open (open threads to pick, "New thread from this meeting…"); nothing was added |
| 4 | Press Escape, then click "Close" on the card | The thread menu, then the card, close |
| 5 | Right-click `[data-ev="ev-acme-sync"]` | The menu holds "Open notes", "Stop recording every meeting of this series" and "Show details"; no "Prepare", no "Add to a thread…" (a recorded meeting's threads are its call's) |
| 6 | Click "Show details" | The menu is gone; the card of Acme weekly sync is open |
| 7 | With the card of Acme weekly sync still open, right-click `[data-ev="ev-globex-weekly"]` and press Escape | Only the menu closes: the card of Acme weekly sync stays open |
| 8 | Click "Close" on the card; right-click `[data-ev="ev-globex-weekly"]`, click "Record when it starts" | The menu is gone; the block now reads "Will record"; open its card: "Don't record" and the line "Echo starts recording a minute before it begins…" |
| 9 | Click "Don't record" on the card (teardown) | The block no longer reads "Will record"; the card offers "Record when it starts" again |
| 10 | Right-click `[data-ev="ev-globex-weekly"]`, then click an empty spot of the grid | The menu is gone; nothing else changed |

## Expected Final State
Nothing changed: Globex weekly was scheduled from the menu and taken back; nothing was prepared, added to a thread or set for its series.

## Notes
- Each item is the card's own button (`data-schedule`, `data-prep-write`, `data-series-set`, …): pressing one does what the card's does (`TC-013`, `TC-018`, `TC-019`, `TC-021`, `prep/TC-012`). Step 8 presses one (Record when it starts) and step 9 takes it back.
- A mouse wheel or resizing the window also closes the menu.
