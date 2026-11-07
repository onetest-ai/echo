---
id: TC-002
title: Search settings keeps the sections whose name has the words typed
priority: low
type: functional
requirements: [F-SET-02]
module: settings
run: agent
platform: any
tags: [settings, search]
---

# TC-002: Search settings keeps the sections whose name has the words typed

**Module:** Settings | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/general` | The left list shows 12 sections |
| 2 | Type `rec` in "Search settings" | The left list shows only "Recording" |
| 3 | Clear "Search settings" | The left list shows all 12 sections again |
| 4 | Type `zzz` in "Search settings" | The left list shows no section |
| 5 | Clear "Search settings" | The left list shows all 12 sections again |

## Expected Final State
The search box filters section names only and leaves nothing behind once cleared.

## Notes
- Known issue (app_profile.md): the search covers section names only, so "dock" finds nothing; this case does not search for a setting.
