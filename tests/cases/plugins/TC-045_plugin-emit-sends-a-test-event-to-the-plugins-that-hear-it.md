---
id: TC-045
title: "Debug builds: echoctl plugin emit queues an event as Echo would tell it, and the running app (a served window) delivers it to the plugins that hear it — refused for an event Echo hasn't, or a payload that isn't an object"
priority: medium
type: functional
requirements: [F-PLUG-36]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, events, cli, test-tooling]
---

# TC-045: plugin emit (debug builds)

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- A debug build (`cargo build`): `target/debug/echoctl`
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (the served window stands in for the app: it takes what other processes queue for it); `{{base_url}}/` loaded once after the install below; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- The test plugin `event-demo` from `tests/fixtures/plugins` (marketplace "test-plugins"): it hears `meeting_soon` and keeps what it heard (`heard`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install event-demo --yes`; open `{{base_url}}/` in the browser and keep it open (its polling has the served window load the plugin just installed) | "Installed Event demo 0.1.0" (its sheet: "Knows when a meeting is about to start (its subject)") |
| 2 | `$E plugin emit meeting_soon '{"id":"m1","subject":"Acme sync","start":"2026-11-14T10:00:00Z","threads":["acme-rollout"]}'`; wait 3 s; `$E plugin call event-demo heard` | "Queued meeting_soon for the running app"; then `[{"id": "m1", "subject": "Acme sync", "start": "2026-11-14T10:00:00Z", "threads": ["acme-rollout"]}]` (the payload as given) |
| 3 | `$E plugin emit lunch_time '{}'`; `$E plugin emit meeting_soon '["x"]'`; `$E plugin emit meeting_soon 'not json'` | Refused: "Echo has no event “lunch_time” (meeting_soon, …)"; "the payload is a JSON object"; "the payload isn't JSON: …" — nothing more in `heard` |
| 4 | `$E plugin remove event-demo --delete-data`; `$E plugin marketplace remove test-plugins` | Removed |

## Teardown
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Without a browser on it, a served window loads a plugin installed after it started only when it starts again: `tests/fixtures/serve.sh down <dir>`, then `(. <dir>/env; ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve <port> > <dir>/serve.log 2>&1 & echo $! > <dir>/pid)` (as plugins/TC-036 does). Emitted before that, an event is taken and heard by nobody.
- A release build refuses `plugin emit` ("only a debug build has it") and leaves it out of `echoctl plugin --help`: the Rust test `plugins::thread_values::tests::plugin_emit_queues_an_event_echo_has_only_in_a_debug_build`.
- An event meant for some plugins only (`shared_changed`) names none when emitted, so no plugin hears it.
