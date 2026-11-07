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

## Test Data

| Where | Line | Note |
|---|---|---|
| Initech planning, transcript, 2nd turn (0:20, Remote) | "Sounds good. On our side the main concern is the SSO cutover for the Berlin office and the support hand-over." | `? Who runs the cutover weekend` |
| Initech planning, transcript, 4th turn (`data-t` 50.67, shown 0:51) | right-click | `Send them the answers first` |
| Acme weekly sync (today), minutes, Summary line "The questionnaire waits for legal" `00:31` | right-click | `[] Ask legal for a date` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="-initech-planning"]`, tab "Transcript" | The turns are shown; My notes is empty |
| 2 | Triple-click the text of the 2nd turn (`.transcript .turn >> nth=1 >> p`) to select it | A popover with one button "Comment" |
| 3 | Click "Comment" | The popover shows "00:20 “Sounds good. On our side the main concern is the SSO cutover for the Berlin office and the support hand-over.”", the box "Your note" and "Goes to My notes at 00:20" |
| 4 | Type `? Who runs the cutover weekend`, click "Add note"; wait 2 s | My notes lists "? 00:20 Who runs the cutover weekend" with the quote under it; the 2nd turn shows a note dot; `my-notes.md` has `- [00:20] ? Who runs the cutover weekend` and the line `  > "Sounds good. On our side the main concern is the SSO cutover for the Berlin office and the support hand-over."` |
| 5 | Right-click the text of the 4th turn | A menu with one item "Add note at 00:51…" (no "Copy": nothing is selected) |
| 6 | Click it, type `Send them the answers first`, press Enter; wait 2 s | My notes lists "• 00:51 Send them the answers first" (no quote); `my-notes.md` has `- [00:51] Send them the answers first` |
| 7 | Click the first row `[data-call$="-acme-weekly-sync"]`, tab "Minutes"; right-click the Summary line "The questionnaire waits for legal" in the minutes frame | A menu "Add note at 00:31…" |
| 8 | Click it | The quote reads exactly "00:31 “The questionnaire waits for legal”" and "Goes to My notes at 00:31 · on “Summary”" |
| 9 | Type `[] Ask legal for a date`, click "Add note"; wait 2 s | My notes lists a to-do "☐ 00:31 Ask legal for a date" with the quote “The questionnaire waits for legal”; `my-notes.md` has `- [ ] [00:31] Ask legal for a date` and `  > "The questionnaire waits for legal"` |

## Expected Final State
Initech planning has two notes and Acme weekly sync one more, each at its moment, the selected or right-clicked words quoted.

## Notes
- Expected to FAIL today:
  - step 6: the menu says 00:51 (the turn starts at 50.67 s, the label rounds) but the note is saved at `[00:50]` (`Math.floor`);
  - steps 8–9: the quote of a minutes line is the line's whole `textContent`, with its time chip and any of the user's own notes the frame shows under the line: here "The questionnaire waits for legal 00:31" in this scenario (after `TC-017` deleted the 00:31 question); a line with a note of the user's under it also quotes that note (seen: "SSO for the Berlin pilot is on track 00:13? Your question 00:12 Dana says the checklist went out on Tuesday"). Not the line's words alone.
  Both reported as suspected bugs.
