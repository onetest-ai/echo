---
id: TC-100
title: A plugin lists Echo's calls newest first, with or without minutes, with their client and participants, narrowed by words, days, a thread, a client and a count, and its sources by client and words, as its sheet says
priority: high
type: functional
requirements: [F-PLUG-70, F-PLUG-107]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, echo-read, calls]
---

# TC-100: A plugin lists the calls as its sheet says

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- The test plugin `tests/fixtures/plugins` (marketplace "test-plugins", plugin "data-demo": its commands read Echo's calls, calendar and the user, and Microsoft 365)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes` | The sheet lists "Reads the list of your calls (titles, times, who was invited, their threads), your calls' minutes, transcripts, meeting chat and who was invited, and the facts of your threads' mail and channels, the meetings Echo has read from your calendar, your name and email and who the people in your threads are (their names, addresses and organisations) in Echo"; "Installed Data demo 0.1.0" |
| 2 | `echoctl plugin call data-demo calls '{"q":"globex pilot kickoff"}'` | One row: `"id": "call:<yesterday>_140000-globex-pilot-kickoff"`, `"title": "Globex pilot kickoff"`, `"date"` yesterday, `"started_at"` and `"ended_at"` 45 min apart, `"memo": false`, `"minutes": true`, `"transcript": true`, `"chat": true`, `"people": ["Sam Rivera", "Kim Park", "Ben Okafor"]`, `"threads": [{"id": "globex-pilot", "name": "Globex pilot", "client": "globex", "client_name": "Globex"}]`, `"profile": "work"` — and no minutes' or transcript's text |
| 3 | `echoctl plugin call data-demo calls '{"limit":2}'` | Two rows, today's latest calls, newest first (their `started_at` going down); among the first rows a call with `"minutes": false` (a call without minutes is listed too) |
| 4 | `echoctl plugin call data-demo calls '{"q":"memo","limit":2}'` | Two rows, each `"memo": true` |
| 5 | `echoctl plugin call data-demo calls '{"q":"kim globex"}'` | Only calls whose title, people or subject hold both words (Globex pilot kickoff among them), newest first |
| 6 | `echoctl plugin call data-demo calls '{"since":"2099-01-01"}'`; `echoctl plugin call data-demo calls '{"q":"no call is called this"}'` | `[]`; `[]` |
| 6a | The row of step 2 again | Also `"duration_min": 45`, `"app": "teams"`, `"client": {"id": "globex", "name": "Globex"}` and `participants`: Sam Rivera (`"person": null`, the user), Kim Park (`"person": "kim@globex.example"`), Ben Okafor (`"person": "ben@globex-labs.example"`), each with their email |
| 6b | `echoctl plugin call data-demo calls '{"thread":"globex-pilot"}'`; `echoctl plugin call data-demo calls '{"client":"globex","until":"<yesterday>"}'`; `echoctl plugin call data-demo calls '{"since":"<yesterday>","until":"<yesterday>"}'` | Every row of the first has `globex-pilot` among its `threads`; every row of the second has `"client": {"id": "globex", "name": "Globex"}` and a `date` no later than yesterday (Globex pilot kickoff among them); every row of the third is dated yesterday. A call in threads of two clients (the memo pinned to Acme rollout and Globex pilot) says `"client": null` |
| 6c | `echoctl plugin call data-demo sources '{"client":"globex","q":"kickoff"}'` | One row: `{"id": "call:<yesterday>_140000-globex-pilot-kickoff", "kind": "call", "title": "Globex pilot kickoff", "date": <yesterday>, "threads": ["globex-pilot"], "client": {"id": "globex", "name": "Globex"}}` — no text (Recall is off on test data: no mail rows) |
| 7 | `echoctl plugin call data-demo threads` | It fails: "data-demo didn't ask to read your threads when it was installed" |

## Expected Final State
Data demo is installed and on; Echo's calls are as they were.

## Teardown
- In S-36: keep the plugin for the next stages. On its own: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The list is the calls' own facts; their words come only from `source` (TC-101). Rust: `jshost::a_plugin_reads_calls_the_calendar_and_who_you_are_only_as_its_sheet_said`.
