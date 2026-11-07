---
id: TC-014
title: echoctl prep lists today's coming meetings with their prep state and shows a written prep and an error prep
priority: medium
type: functional
requirements: [F-PREP-08]
module: prep
run: agent
platform: any
data: standard
tags: [prep, cli]
---

# TC-014: echoctl prep lists today's coming meetings with their prep state and shows a written prep and an error prep

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A prep's id is the one in `<dir>/manifest.json` → `preps` (stable for a given meeting id); Coming up today rows are `[data-up="<prep id>"]`

## Test Data

| Command | Result |
|---|---|
| `echoctl prep --upcoming` | `meetings`: 11 entries, `prep` null for ten and `{error: null, written_at}` for Globex sponsor update |
| `echoctl prep --show <Globex sponsor update's id>` | keys include `summary`, `expected`, `last_time`, `open`, `new_since`, `people` (12) |
| `echoctl prep --show <Analyst roundtable's id>` | `error` set, no `summary` |
| `echoctl prep --show nope` | exit code 1, message `no prep "nope"` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `echoctl prep --upcoming` | Exit code 0; JSON `{"meetings": [...]}` with 11 entries, each with `id`, `event`, `subject`, `start`, `end`, `names`, `threads`, `prep`, `writing` false |
| 2 | Read `prep` of each entry | null except for the entry `Globex sponsor update`, whose `prep` has `error` null and `written_at` |
| 3 | Run `echoctl prep --show` with the id of Globex sponsor update | Exit code 0; JSON with `summary` "Globex sponsor update: the SSO checklist is done…", 3 `expected`, 3 `open`, 2 `new_since`, 12 `people`, `model` "sonnet" |
| 4 | Run `echoctl prep --show` with the id of Analyst roundtable (in `preps`, state error) | JSON with `error` "its threads run on different AI profiles: no summary, the facts only", `model` null, no `summary` |
| 5 | Run `echoctl prep --show nope` | Exit code 1 and the message `no prep "nope"` |

## Expected Final State
The CLI agrees with the window's Coming up today.

## Notes
- Creates or changes nothing.
