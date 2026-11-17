---
id: TC-101
title: "A plugin reads a call's minutes with its facts — its times, app, who was invited and who they are in Echo, its meeting chat, its threads' client and its minutes by section"
priority: high
type: functional
requirements: [F-PLUG-70]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, echo-read, calls, chat]
---

# TC-101: A plugin reads a call's facts, its chat and who was invited

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- Data demo installed: in S-36, stage 1. On its own: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes`

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call data-demo calls '{"q":"globex pilot kickoff"}'`; note its `id` | `call:<yesterday>_140000-globex-pilot-kickoff` |
| 2 | `echoctl plugin call data-demo source '{"id":"<that id>"}'` | `"kind": "call"`, `"title": "Globex pilot kickoff"`, `"text"` starts with "# Globex pilot kickoff" (its minutes), `"transcript"` starts with "# Globex pilot kickoff", `"started_at"` yesterday 14:00, `"ended_at"` yesterday 14:45, `"app": "teams"`, `"platform": "macos"`, `"memo": false`, `"profile": "work"` |
| 3 | The same answer | `"attendees": [{"name": "Sam Rivera", "email": "sam@initech.example"}, {"name": "Kim Park", "email": "kim@globex.example"}, {"name": "Ben Okafor", "email": "ben@globex-labs.example"}]` (the organizer first); `"chat": [{"at": <yesterday 14:00>, "from": "Kim Park", "text": "Plan template?", "files": ["pilot-plan.xlsx"]}]`; `"threads": [{"id": "globex-pilot", "name": "Globex pilot", "client": "globex", "client_name": "Globex"}]` |
| 3a | The same answer | What Echo decided of it: `"duration_min": 45`; `"client": {"id": "globex", "name": "Globex"}`; `participants` the three attendees with `person` each — Kim Park's `kim@globex.example`, Ben Okafor's `ben@globex-labs.example`, Sam Rivera's (the user) `null`; `minutes`: `"summary": "- Globex wants a pilot plan in two weeks [00:05]"`, `"decisions": []`, `"questions": []`, `"actions": [{"id": "glob0001", "text": "Draft the Globex pilot plan", "owner": "Me", "due": null, "done": false}]`, `revision` 16 hex characters, and `body` starting "## Summary" (no "# Globex pilot kickoff" title line) |
| 3b | `echoctl plugin call data-demo source '{"id":"<that id>","opts":{"transcript":false,"chat":false}}'` | The same answer without the keys `transcript` and `chat` |
| 4 | `echoctl plugin call data-demo calls '{"q":"quick check-in","limit":1}'`; then `source` of its `id` | The row says `"minutes": false`; `source` fails: "…: no minutes yet" |
| 5 | `echoctl plugin call data-demo source '{"id":"call:../x"}'` | It fails: "not a call: call:../x" |

## Expected Final State
Nothing changed.

## Teardown
- In S-36: none. On its own: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A call's text may go only to its pinned AI profile: `profile` says which (as before); Echo hands the plugin's own JS the data its sheet named, and its Claude runs stay on `claudejob::run`.
