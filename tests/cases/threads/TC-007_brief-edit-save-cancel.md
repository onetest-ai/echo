---
id: TC-007
title: The Brief is edited in place: ⌘S saves it as Markdown, Esc cancels, Latest calls lists the thread's calls
priority: high
type: functional
requirements: [F-THR-10]
module: threads
run: agent
platform: any
tags: [threads, brief]
---

# TC-007: The Brief is edited in place: ⌘S saves it as Markdown, Esc cancels, Latest calls lists the thread's calls

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Globex pilot has an empty brief (`echoctl project show globex-pilot` → `"brief": ""`)

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |
| Brief text | Pilot for **Globex** design review. |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/brief` | A card "What it is" with a button "Edit" and the text "Nothing yet — say what this thread is for, so the minutes and Claude know."; a section "LATEST CALLS" listing "Globex pilot kickoff" |
| 2 | Click "Edit" | A text area named "The thread's brief (Markdown)" and buttons "Cancel", "Save"; the hint "⌘S / Ctrl+S saves, Esc cancels." |
| 3 | Type `Draft to throw away` and press Escape | The text area is gone; "Nothing yet — say what this thread is for…" is shown again |
| 4 | Click "Edit", type `Pilot for **Globex** design review.` and press ⌘S (Ctrl+S on Windows) | The editor closes; the card shows the text "Pilot for Globex design review." with "Globex" in bold (a `strong` element) |
| 5 | Run `echoctl project show globex-pilot` | `"brief"` is `Pilot for **Globex** design review.` |

## Expected Final State
The brief of Globex pilot reads "Pilot for Globex design review." and is stored as Markdown.

## Teardown
- Click "Edit", clear the text area, press ⌘S; confirm "Nothing yet…" is shown again

## Changes
- Changes `brief.md` of `globex-pilot`.
