---
id: S-47
title: "Find what was said: the Calls box with Intake off, then on — results, filters, a moment in the panel, no match, older days and ⌘K"
run: agent
data: standard
cases:
  - tests/cases/calls/TC-232_with-intake-off-enter-in-the-calls-box-keeps-to-titles-and-people.md
  - tests/cases/calls/TC-233_enter-in-the-calls-box-lists-titles-and-people-then-what-was-said-by-source.md
  - tests/cases/calls/TC-234_a-searchs-kinds-sections-and-client-narrow-it-and-a-date-in-the-words-is-a-dashed-chip.md
  - tests/cases/calls/TC-235_a-moment-opens-in-what-was-said-with-the-turns-around-it-and-opens-the-call-there.md
  - tests/cases/calls/TC-236_a-search-with-no-match-says-so-in-one-line-with-search-every-source.md
  - tests/cases/calls/TC-237_a-search-of-days-past-the-index-says-older-than-the-index-keeps.md
  - tests/cases/calls/TC-238_cmd-k-search-what-was-said-opens-the-calls-box-in-search-mode.md
---

# S-47: Find what was said

**Story:** Sam searches his calls by title as always. Once he turns Intake on, the same box finds what was said in calls,
mail and channels: he narrows it, reads a moment beside the list, opens the call there, and starts the same search from ⌘K.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard`; every `echoctl` command is
`target/debug/echoctl` (built right before) after `set -a; . <dir>/env; set +a`, and only when `ECHOCTL_CONFIG_DIR`
points inside `<dir>` (otherwise stop: the real data folder and config are never touched).

**Length:** about 6 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build (with Intake's mail and post facts) | the lead (`serve.sh up`) | 1–7 |
| Recall on and indexed (`intake set --enabled true`, `intake sync`) | glue before stage 2 | 2–7 |

## Prelude
1. Open a new tab for this scenario; load `{{base_url}}/`.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-232_with-intake-off-enter-in-the-calls-box-keeps-to-titles-and-people.md` | — | — |
| 2 | `calls/TC-233_enter-in-the-calls-box-lists-titles-and-people-then-what-was-said-by-source.md` | — | Run `target/debug/echoctl intake set --enabled true`, then `target/debug/echoctl intake sync` (with the env, checked); load `{{base_url}}/` again |
| 3 | `calls/TC-234_a-searchs-kinds-sections-and-client-narrow-it-and-a-date-in-the-words-is-a-dashed-chip.md` | 2 | — |
| 4 | `calls/TC-235_a-moment-opens-in-what-was-said-with-the-turns-around-it-and-opens-the-call-there.md` | 2 | — |
| 5 | `calls/TC-236_a-search-with-no-match-says-so-in-one-line-with-search-every-source.md` | 2 | — |
| 6 | `calls/TC-237_a-search-of-days-past-the-index-says-older-than-the-index-keeps.md` | 2 | — |
| 7 | `calls/TC-238_cmd-k-search-what-was-said-opens-the-calls-box-in-search-mode.md` | 2 | — |

## Notes
- No embedding server is set on test data: results come in keyword order, as the cases expect.
- "Copy link" and "Copy what was said" put text on this computer's clipboard: the cases check the buttons, not a click.
- Close the tab and `serve.sh down <dir>` at the end.
