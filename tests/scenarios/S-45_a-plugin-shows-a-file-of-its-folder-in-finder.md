---
id: S-45
title: "A plugin shows a file of its folder in Finder: on a click only, never outside the folder, and on test data only in the open log"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-200_a-plugin-shows-a-file-of-its-folder-in-finder-on-your-click-and-nothing-outside-it.md
---

# S-45: A plugin shows a file of its folder in Finder

**Story:** Sam gives a notes plugin his Acme notes folder. In its view, "Show in Finder" shows the note he is on,
selected in Finder; the plugin can't do it by itself while it loads, nor show anything outside the folder he gave it.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard`; every `echoctl` command is
`target/debug/echoctl` (built right before) after `. <dir>/env`.

**Length:** about 2 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build | the lead (`serve.sh up`) | 1 |
| The marketplace `acme-local` with Acme reveal, given `$D/vault` | stage 1 (its preconditions and step 1) | 1 |

## Prelude
1. Open a new tab for this scenario; load `{{base_url}}/`.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-200_a-plugin-shows-a-file-of-its-folder-in-finder-on-your-click-and-nothing-outside-it.md` | — | Run its precondition lines as written; its teardown removes the plugin and `acme-local` |

## Notes
- The test data's `env` sets `ECHOCTL_TEST_OPEN_LOG_ONLY`: Finder never opens, the line goes to `<dir>/open.log`. Close the tab and `serve.sh down <dir>` at the end.
