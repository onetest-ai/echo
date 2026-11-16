---
id: TC-033
title: Yolo runs' filters in one row above the list — state, task or item, thread, from and to, changed or sent through Echo, and a search over what Claude said — narrow the list, add up, say how many show, and Clear resets them; only the list scrolls
priority: medium
type: functional
requirements: [F-TASK-16]
module: tasks
run: agent
platform: macos
data: standard
ai: virtual
tags: [tasks, yolo, yolo-runs, filters]
---

# TC-033: Yolo runs' filters

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-032's four runs on the same data (`ask-runs-1` Book the workshop room: done, a change; `ask-runs-2` Collect the user list: done, nothing through Echo; `ask-runs-3` Mail to you · Contract redlines for the Globex pilot: done, a draft; `ask-runs-4` Map the user groups: failed); `{{base_url}}/#yolo` open. Other Yolo runs of the scenario may be there too: the expected rows below are among TC-032's four, and the counts say "N of M runs" with M every run listed

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Read the state filter's options; choose "Failed" (`select[data-yo-f="state"]`) | "All states", "Working", "Done", "Failed" — nothing else; then only "Map the user groups" of TC-032's runs; the chip reads "Failed" (highlighted); the count "N of M runs"; a "Clear" link at the row's end |
| 2 | Click "Clear" (`.yo-filters [data-yo-clear]`); choose "Globex pilot" in the thread filter | The state chip is "All states" again; of TC-032's runs: "Mail to you · Contract redlines for the Globex pilot" and "Collect the user list" |
| 3 | Clear; choose "Book the workshop room" in the task filter (`select[data-yo-f="task"]`) | Only "Book the workshop room"; the task filter also offers "Mail to you · Contract redlines for the Globex pilot", "Collect the user list" and "Map the user groups"; no label twice |
| 4 | Clear; tick "Changed or sent through Echo" | Of TC-032's runs only "Book the workshop room" (a draft is neither a change nor a send) |
| 5 | Clear; type "kim" in "Search what Claude said" (`#yo-q`), slowly | The box keeps the focus and the text while the list narrows; of TC-032's runs: "Mail to you · Contract redlines for the Globex pilot" and "Collect the user list" (their lines name Kim Park) |
| 6 | Clear; set From to tomorrow (Echo's date picker or the box) | "No run matches these filters" with "Clear the filters"; a click on it shows every run again |
| 7 | Set From and To both to today; then the thread filter to "Acme rollout" | First every run of today; then of TC-032's runs "Map the user groups" and "Book the workshop room"; then the state filter to "Done": only "Book the workshop room" (filters add up) |
| 8 | Resize the window to 560 px wide; then switch the system to dark mode (`prefers-color-scheme: dark`) | The filters wrap onto more lines, the search box takes a line of its own, nothing is cut or scrolls sideways; the head and the filters stay put and only the list scrolls (`.yo` doesn't scroll, `.yo-list` does); in dark mode the page, chips, rows and the details use the dark colours (no light boxes) |

## Expected Final State
Filters cleared by the user's last Clear or left set; nothing in the data changed.

## Teardown
- Resize back and switch the colour scheme back to light (no data to restore)
