---
id: TC-011
title: context, actions, action, type, title and html work on one call by its folder name or path, and the window shows the kind and the title
priority: high
type: functional
requirements: [F-CLI-08]
module: cli
run: agent
platform: any
data: standard
tags: [cli, calls]
auto: [tests/auto/specs/S-28.spec.mjs]
---

# TC-011: context, actions, action, type, title and html work on one call by its folder name or path, and the window shows the kind and the title

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `C=$(ls "$ECHOCTL_HOME/calls" | grep -- '-globex-pilot-kickoff$')` (yesterday's call at 14:00; each call's folder name starts with its date, which follows the day the data is built: look it up, never type a date)
- Reload the page fully (`{{base_url}}/`) after a CLI change before reading the window

## Test Data

| Field | Value |
|---|---|
| Call | Globex pilot kickoff, 45 min, in Globex pilot, kind Client (yours) |
| Its action item | `glob0001` "Draft the Globex pilot plan", owner Me, `status` "open" |
| People | Kim Park <kim@globex.example>, Ben Okafor <ben@globex-labs.example> |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl context $C` | Exit code 0; JSON with `dir` ending `/calls/$C`, `duration_min` 45.0, `dominant_language` "en", `external` `["Kim Park <kim@globex.example>","Ben Okafor <ben@globex-labs.example>"]`, `colleagues` [], `meeting_type.type` "client" |
| 2 | Run `target/debug/echoctl context "$ECHOCTL_HOME/calls/$C"` (the path) | The same JSON as step 1 |
| 3 | Run `target/debug/echoctl actions $C` | A JSON array of one item: `id` "glob0001", `task` "Draft the Globex pilot plan", `owner` "Me", `status` "open" |
| 4 | Run `target/debug/echoctl action $C glob0001 done`, then `target/debug/echoctl actions $C` | The first prints `done`; the list now has `status` "done" |
| 5 | Run `target/debug/echoctl action $C glob0001 todo` | Prints `todo`; `actions` shows `status` "todo" |
| 6 | Run `target/debug/echoctl type $C partner` | `{"by":"you","label":"Partner","type":"partner"}` |
| 7 | Run `target/debug/echoctl type $C bogus` | Exit code 1; "Error: a meeting type is one of: client, partner, one-on-one, internal, industry, other, steering" |
| 8 | Reload, click the row Globex pilot kickoff (`[data-call="$C"]`) | The call's header shows the kind chip "Partner ▾" |
| 9 | Run `target/debug/echoctl title $C "Globex pilot kickoff (signed)"` | Prints the new folder path ending `/calls/${C}-signed`; `ls "$ECHOCTL_HOME/calls" \| grep -- '-globex-pilot-kickoff'` lists only that name |
| 10 | Reload `{{base_url}}/#calls`, find the row `[role=option][data-call="${C}-signed"]` (yesterday's call at 14:00, which the row writes in the browser's locale: "02:00 PM" in en-US) | It reads "Globex pilot kickoff (signed)"; opened, its header shows that title, "Partner ▾" and the thread chip "Globex pilot" |
| 11 | Run `target/debug/echoctl html ${C}-signed` | Prints the path of `Globex pilot kickoff (signed)-minutes.html` inside that folder; the file exists |
| 12 | Run `target/debug/echoctl title ${C}-signed "Globex pilot kickoff"` and `target/debug/echoctl type $C client` | The folder is `$C` again; the kind is `client` |
| 13 | Run `target/debug/echoctl context` (no call: "latest") and read `dir` | `dir` is the newest call: the last name of `ls "$ECHOCTL_HOME/calls" \| grep '^20' \| tail -1` (today's newest recording) |

## Expected Final State
The call has its title, folder and kind back; its action item is to do (`todo`); a minutes page was written beside its notes.

## Teardown
- Steps 5 and 12 restore what the case changed (the minutes page `Globex pilot kickoff (signed)-minutes.html` stays: harmless)

## Notes
- The test data writes the action's status as "open"; the app's own words are `todo` and `done` (`calls.rs`, `set_action_status`), so after step 5 it reads `todo`.
- Step 13: "latest" is the last call in the app's one order (`calls::all_calls`: folders with no date stamp oldest, then the dated ones by their stamp), so the undated call `old-dictation` never wins over a dated one (#147); the window's list and the tray use the same order.
- A call's folder may be given by name or by path; an old name stops working after `title` renames it (by its stamp it is still found by threads and links).
- `note` and `delete`/`restore` are in `TC-012`.
