---
id: TC-019
title: Keep every section on a template is saved at once, shows "all kept" in the list, and goes with Reset to built-in; the memo has none
priority: medium
type: functional
requirements: [F-TPL-08, F-TPL-02]
module: templates
run: agent
platform: any
tags: [templates]
---

# TC-019: Keep every section on a template is saved at once, shows "all kept" in the list, and goes with Reset to built-in; the memo has none

**Module:** Templates | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/templates/default.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates` and click the row "Default" | The editor shows the tag "Built in"; after the sections, before "Add a section", an unticked checkbox "Keep every section" with the words "each one stays, in this order" |
| 2 | Tick "Keep every section"; wait 1 s and read the row "Default" | The row shows the tag "Edited" and reads "… sections, all kept" |
| 3 | Run `target/debug/echoctl templates` and `grep -c '^fixed: true$' "$ECHOCTL_HOME/.echo/templates/default.md"` | Default's entry under `templates` has `"fixed": true`; the grep prints `1` |
| 4 | Untick "Keep every section"; wait 1 s; run the two commands of step 3 again | The row no longer says "all kept"; Default's entry has `"fixed": false`; the grep prints `0` |
| 5 | Tick it again, then click "Reset to built-in" | The tag is "Built in" again and the checkbox is unticked; `target/debug/echoctl templates` shows `"fixed": false` for Default |
| 6 | Click the row "Memo" | The memo's editor has no "Keep every section" |

## Expected Final State
Default is Echo's own again (not fixed); no template file of the user's is left.

## Teardown
- If the row "Default" still shows "Edited": open it and click "Reset to built-in".
