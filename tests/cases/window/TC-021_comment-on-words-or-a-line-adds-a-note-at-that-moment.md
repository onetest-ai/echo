---
id: TC-021
title: Selecting words and Comment, or right-clicking a turn or a minutes line and Add note, adds a note at that moment with the words quoted
priority: medium
type: functional
requirements: [F-TR-05]
module: window
run: agent
platform: any
data: standard
tags: [transcript, notes, minutes]
---

# TC-021: Selecting words and Comment, or right-clicking a turn or a minutes line and Add note, adds a note at that moment with the words quoted

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); file reads run after `. <dir>/env`
- My notes is open in the right panel; keep the sidebar folded (app_profile → Fragile areas: the minutes frame is narrow)
- The popover is `#sel-pop`: "Comment" (after a selection), or a menu "Add note at mm:ss…" (after a right-click); then a quote, a box "Your note" (`#sel-note`), "Goes to My notes at mm:ss[ · on “<section>”]", "Cancel" and "Add note"
- Turns are `.transcript .turn` (`data-t` = seconds); the minutes are in `iframe#minutes-frame` (act inside with `page.frameLocator('#minutes-frame')`)
- **In S-29** My notes has been open since `window/TC-016` step 1, and the Calls list is already on screen, left by the stage before, so step 1's navigation is skipped and step 1 starts at its click. On its own: open a call, press `[aria-label="My notes"]`, then run step 1

## Test Data

| Where | Line | Note |
|---|---|---|
| Initech planning, transcript, 2nd turn (0:20, Remote) | "Sounds good. On our side the main concern is the SSO cutover for the Berlin office and the support hand-over." | `? Who runs the cutover weekend` |
| Initech planning, transcript, 4th turn (`data-t` 50.67, shown 0:50) | right-click | `Send them the answers first` |
| Acme weekly sync (today), minutes, Summary line "The questionnaire waits for legal" `00:31` | right-click | `[] Ask legal for a date` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="-initech-planning"]`, tab "Transcript" | The turns are shown; My notes is empty |
| 2 | Triple-click the text of the 2nd turn (`.transcript .turn >> nth=1 >> p`) to select it | A popover with one button "Comment" |
| 3 | Click "Comment" | The popover shows "00:20 “Sounds good. On our side the main concern is the SSO cutover for the Berlin office and the support hand-over.”", the box "Your note" and "Goes to My notes at 00:20" |
| 4 | Type `? Who runs the cutover weekend`, click "Add note"; wait 2 s | My notes lists "? 00:20 Who runs the cutover weekend" with the quote under it; the 2nd turn shows a note dot; `my-notes.md` has `- [00:20] ? Who runs the cutover weekend` and the line `  > "Sounds good. On our side the main concern is the SSO cutover for the Berlin office and the support hand-over."` |
| 5 | Right-click the text of the 4th turn | A menu with one item "Add note at 00:50…" (no "Copy": nothing is selected) |
| 6 | Click it, type `Send them the answers first`, press Enter; wait 2 s | My notes lists "• 00:50 Send them the answers first" (no quote); `my-notes.md` has `- [00:50] Send them the answers first` |
| 7 | Click the first row `[data-call$="-acme-weekly-sync"]`, tab "Minutes"; right-click the Summary line "The questionnaire waits for legal" in the minutes frame | A menu "Add note at 00:31…" |
| 8 | Click it | The quote reads exactly "00:31 “The questionnaire waits for legal”" and "Goes to My notes at 00:31 · on “Summary”" |
| 9 | Type `[] Ask legal for a date`, click "Add note"; wait 2 s | My notes lists a to-do "☐ 00:31 Ask legal for a date" with the quote “The questionnaire waits for legal”; `my-notes.md` has `- [ ] [00:31] Ask legal for a date` and `  > "The questionnaire waits for legal"` |

## Expected Final State
Initech planning has two notes and Acme weekly sync one more, each at its moment, the selected or right-clicked words quoted.

## Notes
- Steps 5–6: times are whole seconds rounded down everywhere — the turn's own time, the menu, the saved note, minutes chips — so they agree and the note stays on its turn (#128).
- Steps 8–9: the quote is the line's own words, without its time chip, the user's notes shown under it, or its sub-items (#129).
