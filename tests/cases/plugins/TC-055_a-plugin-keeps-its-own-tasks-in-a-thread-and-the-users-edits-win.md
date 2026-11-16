---
id: TC-055
title: "A plugin keeps its own tasks in a thread by its key — in its own story, the user's edits kept, a proposal only after the user's Yes, and after No the same stays quiet"
priority: high
type: functional
requirements: [F-PLUG-38]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, threads, work, to-review, host-half]
---

# TC-055: A plugin keeps its own tasks in a thread

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- The test marketplace with "tracker-demo" installed: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install tracker-demo --from test-plugins --yes` (its `task` command upserts a task, `close` closes one)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call tracker-demo task '{"thread":"globex-pilot","task":{"key":"GX-12","title":"Send Globex the pilot quote","due":"2030-01-15","url":"https://tracker.globex.example/p/12","reason":"next step in the tracker"}}'` | `"applied": true`, `"proposed": false`, `"ignored": []`, a `task` id (T…, call it `$T`) |
| 2 | Load `{{base_url}}/#project/globex-pilot/work`; Layout "Tree" | A story "Tracker Demo" with the task "Send Globex the pilot quote" under it |
| 3 | Click the task | Details: "General › Tracker Demo"; Due "Tue, Jan 15"; "From" "↗ Tracker Demo"; History "Tracker Demo created it — next step in the tracker", with no "verified" mark |
| 4 | Step 1's call again | `"applied": false`, the same `task`; reload: History still one entry |
| 5 | `$E project work globex-pilot '{"op":"edit","node":"<$T>","set":{"due":"2030-02-01"}}'`; then `$E plugin call tracker-demo task '{"thread":"globex-pilot","task":{"key":"GX-12","title":"Send Globex the pilot quote","due":"2030-01-20"}}'` | The plugin's answer: `"applied": false`, `"ignored": ["due"]`; reload, the task's row says "pinned: due", its Due "Fri, Feb 1" (the user's) |
| 6 | `$E plugin call tracker-demo task '{"thread":"globex-pilot","task":{"key":"GX-13","title":"Book the Globex demo","due":"2030-01-22","propose":true}}'`; reload the Work tab | `"task": null`, `"proposed": true`; a "To review" group "Tracker Demo suggests a task: Book the Globex demo · due 2030-01-22" with "Yes" and "No"; no such task in the tree |
| 7 | Open the Inbox (right panel) | An item "1 to review in Globex pilot's work", "1 plugin suggestion" |
| 8 | Click "No" on the suggestion; step 6's call again; reload | The suggestion goes; the plugin's answer `"proposed": false`, `"why": "the user said no to this"`; no "To review" |
| 9 | Step 6's call with `"due":"2030-01-29"`; reload; click "Yes" | A suggestion "… · due 2030-01-29"; after Yes it goes and "Book the Globex demo" is under the story "Tracker Demo" |
| 10 | `$E plugin call tracker-demo close '{"thread":"globex-pilot","key":"GX-12","reason":"pursuit won"}'`; reload; turn "Open only" off; click "Send Globex the pilot quote" | `"applied": true`; its status "done"; History "Tracker Demo: done — pursuit won" |
| 11 | Teardown below | — |

## Teardown
- `$E plugin remove tracker-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A task the user closed, deleted or merged; another plugin's key; withdrawing a proposal by closing it: the Rust tests in `ledger::plugin::tests`; the refusals: `jshost::tests::a_plugin_sets_a_threads_stage_and_its_own_tasks_only_as_its_sheet_says_and_never_while_claude_prepares`; the To review rows and History lines: `work-status.test.mjs`.
