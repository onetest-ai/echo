---
id: TC-002
title: Search settings finds sections by their names and by their settings' own labels
priority: low
type: functional
requirements: [F-SET-02]
module: settings
run: agent
platform: any
tags: [settings, search]
---

# TC-002: Search settings finds sections by their names and by their settings' own labels

**Module:** Settings | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/general` | The left list (`.set-nav`) shows 12 sections (count them there: the top bar's account chip also carries `data-setsec`) |
| 2 | Type `keep audio` in "Search settings" | The left list shows only "Transcription"; the page shows Transcription, its row "Keep audio files" picked out (`.set-row.hit`) and no other row picked out |
| 3 | Clear "Search settings" | The left list shows all 12 sections again; the page shows General, no row picked out |
| 4 | Type `calendar` in "Search settings" | The left list shows "Recording" and "Calendar & chat", in that order; the page shows Recording with "Calendar meetings" picked out |
| 5 | Clear "Search settings", then type `zzz` | The left list shows no section and the line `Nothing matches “zzz”.` |
| 6 | Clear "Search settings" | The left list shows all 12 sections again and no "Nothing matches" line |

## Expected Final State
The search finds a section by its name or by the label of a setting in it, picks those settings out, says so when nothing matches, and leaves nothing behind once cleared.

## Notes
- #103: the search used to cover section names only ("Dock" found nothing) and showed no message when nothing matched.
- A label is a row's title or a card's head, not its description. Templates, Plugins and Sends to you are found by their names only (their lists load when drawn); plugins' own sections by their names.
