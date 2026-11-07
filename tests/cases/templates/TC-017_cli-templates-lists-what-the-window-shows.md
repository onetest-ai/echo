---
id: TC-017
title: echoctl templates lists the templates, meeting types, skills and values the window shows
priority: low
type: functional
requirements: [F-TPL-07]
module: templates
run: agent
platform: any
tags: [templates, cli]
---

# TC-017: echoctl templates lists the templates, meeting types, skills and values the window shows

**Module:** Templates | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Bash (`. <dir>/env` first) run `echoctl templates` | JSON with the keys `folder`, `meeting_types`, `skills`, `templates`, `values`; `folder` ends with `/.echo/templates` |
| 2 | Read the ids under `templates` | `default`, `discussion`, `status-sync`, `memo`, each with `"kind": "builtin"` |
| 3 | Read the ids under `meeting_types` | `client`, `partner`, `one-on-one`, `internal`, `industry`, `other` |
| 4 | Read `skills` | One skill, id `follow-up-mail`, `"kind": "builtin"`, `"on": true` |
| 5 | Read the names under `values` | title, date, time, duration, participants, colleagues, external, organizer, type, thread, client, previous, me, language |

## Expected Final State
The CLI shows the same four, six and one items as the window.
