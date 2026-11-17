---
id: TC-054
title: "A plugin moves a thread's stage with its name and reason; once the user changes the stage it leaves it alone, until the user lets it set the stage again"
priority: high
type: functional
requirements: [F-PLUG-37]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, threads, stage, host-half]
---

# TC-054: A plugin moves a thread's stage

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- The test marketplace `tests/fixtures/plugins` ("test-plugins") with "tracker-demo" (Tracker Demo: `threads` reads Echo's threads, `stage` sets one's stage)
- Globex pilot set to Lead by the user before the plugin ever wrote: `$E project set globex-pilot --stage lead`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install tracker-demo --from test-plugins` | The sheet has "Reads your threads' names and stages in Echo. Changes your threads' stage. Adds and updates tasks in your threads", then asks for `--yes` |
| 2 | `$E plugin install tracker-demo --from test-plugins --yes`; `$E plugin call tracker-demo threads` | Installed; the Globex pilot row has `"stage": "lead"`, `"group": "pursuit"` |
| 3 | `$E plugin call tracker-demo stage '{"thread":"globex-pilot","stage":"proposal","reason":"proposal sent in the tracker"}'` | `{"applied": true}`: the user's Lead from before the plugin's first write holds nothing |
| 4 | Load `{{base_url}}/#project/globex-pilot/profile` | The Stage group's "Proposal" is pressed; the line under it reads "Proposal since <today> · Lead … (set by Tracker Demo: proposal sent in the tracker)"; no "Let Tracker Demo set it again" button |
| 5 | Click "Negotiation" in the Stage group | "Negotiation" is pressed; the line has no "set by"; a button "Let Tracker Demo set it again" is there |
| 6 | `$E plugin call tracker-demo stage '{"thread":"globex-pilot","stage":"lost","reason":"lost in the tracker"}'`; reload `{{base_url}}/#project/globex-pilot/profile` | `{"applied": false, "why": "the user set this thread's stage themselves"}`; "Negotiation" still pressed |
| 7 | Click "Let Tracker Demo set it again" | The button goes; the stage stays Negotiation |
| 8 | Step 6's call again; reload the Profile | `{"applied": true}`; "Lost" (group Closed) pressed, the line "(set by Tracker Demo: lost in the tracker)" |
| 9 | `$E plugin call tracker-demo threads` | Globex pilot is no longer listed (a closed thread isn't an open one) |
| 10 | Teardown below | — |

## Teardown
- `$E project set globex-pilot --stage active`; `$E plugin remove tracker-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Refusals (no sheet line, a Yolo run on another thread) and the stage rule itself: the Rust tests `jshost::tests::a_plugin_sets_a_threads_stage_and_its_own_tasks_only_as_its_sheet_says` and `projects::stage::tests::a_plugin_moves_the_stage_until_the_user_overrides_it_and_again_once_they_hand_it_back`; the window's rule `projects.test.mjs`.
- The CLI's way back: `$E project set globex-pilot --stage-back-to tracker-demo`.
