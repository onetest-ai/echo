---
id: TC-045
title: "Debug builds: echoctl plugin emit queues an event as Echo would tell it, and the running app (a served window) delivers it to the plugins that hear it — minutes_written built from a call carries its routing, people and minutes only to a plugin that reads sources; refused for an event Echo hasn't, or a payload that isn't an object"
priority: medium
type: functional
requirements: [F-PLUG-36, F-PLUG-106]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, events, cli, test-tooling, echo-read]
---

# TC-045: plugin emit (debug builds)

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- A debug build (`cargo build`): `target/debug/echoctl`
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (the served window stands in for the app: it takes what other processes queue for it); `{{base_url}}/` loaded once after the install below; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- The test plugins from `tests/fixtures/plugins` (marketplace "test-plugins"): `event-demo` hears `meeting_soon` and `minutes_written` and reads no sources, keeping what it heard (`heard`: meetings, `minutes`: minutes); `data-demo` reads sources (so it hears `minutes_written` without naming it) and keeps what it heard (`minutes`)

**Changes:** the two plugins and their stores; `.minutes-told` in the Globex pilot kickoff call's folder (the revision of its minutes Echo told)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install event-demo --yes`; `$E plugin install data-demo --yes`; open `{{base_url}}/` in the browser and keep it open (its polling has the served window load the plugins just installed) | "Installed Event demo 0.1.0" (its sheet: "Knows when a meeting is about to start (its subject) and a call's minutes are written (the call's title)"); "Installed Data demo 0.1.0" |
| 2 | `$E plugin emit meeting_soon '{"id":"m1","subject":"Acme sync","start":"2026-11-14T10:00:00Z","threads":["acme-rollout"]}'`; wait 3 s; `$E plugin call event-demo heard` | "Queued meeting_soon: the running app takes it within a second, or at its next start"; then `[{"id": "m1", "subject": "Acme sync", "start": "2026-11-14T10:00:00Z", "threads": ["acme-rollout"]}]` (the payload as given) |
| 3 | `$E plugin emit minutes_written --from <yesterday>_140000-globex-pilot-kickoff`; wait 3 s; `$E plugin call event-demo minutes` | "Queued minutes_written: …"; one entry with exactly the keys `id` (`call:<yesterday>_140000-globex-pilot-kickoff`), `link` (`echo://call/…`), `call` (the folder), `title` "Globex pilot kickoff", `started_at` yesterday 14:00, `ended_at` yesterday 14:45, `duration_min` 45, `app` "teams", `memo` false, `revision` (16 hex characters), `rewritten` false — no `threads`, `client`, `profile`, `participants` or `minutes` (it reads no sources) |
| 4 | `$E plugin call data-demo minutes` | One entry: the same keys and values as step 3, plus `"threads": [{"id": "globex-pilot", "name": "Globex pilot", "client": "globex", "client_name": "Globex"}]`, `"client": {"id": "globex", "name": "Globex"}`, `"profile": "work"`, `participants` (Sam Rivera, Kim Park, Ben Okafor with their emails; Kim's `person` `kim@globex.example`, Ben's `ben@globex-labs.example`), and `minutes`: `summary` "- Globex wants a pilot plan in two weeks [00:05]", `decisions` `[]`, `questions` `[]`, `actions` `[{"id": "glob0001", "text": "Draft the Globex pilot plan", "owner": "Me", "due": null, "done": false}]`, the same `revision`, and no `body`; nothing of the transcript ("We'd like a pilot plan in two weeks.") anywhere |
| 5 | Step 3's emit again; wait 3 s; `$E plugin call data-demo minutes` | Two entries; the second has `rewritten` true and the same `revision` as the first |
| 6 | `$E plugin emit lunch_time '{}'`; `$E plugin emit meeting_soon '["x"]'`; `$E plugin emit meeting_soon 'not json'`; `$E plugin emit meeting_soon --from x`; `$E plugin emit minutes_written '{}' --from x` | Refused: "Echo has no event “lunch_time” (meeting_soon, …)"; "the payload is a JSON object"; "the payload isn't JSON: …"; "--from builds minutes_written or sources_changed, not meeting_soon"; "the argument '[PAYLOAD]' cannot be used with '--from <FROM>'" — nothing more in `heard` |
| 7 | `$E plugin remove event-demo --delete-data`; `$E plugin remove data-demo --delete-data`; `$E plugin marketplace remove test-plugins` | Removed |

## Teardown
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Without a browser on it, a served window loads a plugin installed after it started only when it starts again: `tests/fixtures/serve.sh down <dir>`, then `(. <dir>/env; ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve <port> > <dir>/serve.log 2>&1 & echo $! > <dir>/pid)` (as plugins/TC-036 does). Emitted before that, an event is taken and heard by nobody.
- A release build refuses `plugin emit` ("only a debug build has it"): the Rust test `plugins::thread_values::tests::plugin_emit_queues_an_event_echo_has_only_in_a_debug_build` (that a release build also leaves it out of `echoctl plugin --help` is `hide = !cfg!(debug_assertions)`, read, not tested).
- Run it only after `. <dir>/env`: a debug build has no data folder of its own, so without the test data's environment the event goes to the queue of the Echo you use.
- `shared_changed` is refused: it goes only to its namespace's plugins, which an emit can't name (set the shared value with `plugin call`, plugins/TC-043).
- `sources_changed`'s `items` need Recall's mail and posts, and Recall is off on test data: the Rust tests `share::tests::sources_changed_carries_what_recall_decided_and_the_read_says_the_same` and `share::tests::a_plugin_that_doesnt_read_sources_hears_no_routing_people_minutes_or_mail` cover them.
