---
id: TC-023
title: A plugin adds a row to each thread's Profile and keeps a value per thread; the value shows on its thread only, goes with a deleted thread (the plugin hears it), and a row that throws leaves the Profile drawn
priority: medium
type: functional
requirements: [F-PLUG-19]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, threads, client-half, slots, host-half, events]
---

# TC-023: A plugin's row in a thread's Profile, and its values per thread

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- The test plugin `tests/fixtures/plugins` (marketplace "test-plugins", plugin "thread-demo": a "Repositories" row in each thread's Profile, the repositories kept per thread; it throws on a thread named "Boom")
- Two made-up threads: `echoctl project create "Alpha"`, `echoctl project create "Beta"` (ids `alpha`, `beta`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `<data>/.echo/plugins/thread-demo/`, the threads Alpha, Beta and Boom

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install thread-demo --yes` | The sheet lists "Runs its own code in Echo and in the Echo window, with your rights" and "Knows when a thread is renamed, merged or deleted"; "Installed Thread demo 0.1.0" |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#project/alpha/profile` | The Profile card has a row "Repositories", right under "Sources", reading "None yet" with a box and "Save" |
| 3 | Type `acme-test/api` in the box "Repositories", click "Save" | The row shows the chip "⎇ acme-test/api" |
| 4 | `echoctl plugin call thread-demo get '{"thread":"alpha"}'`; then the same for `beta`; then `echoctl plugin call thread-demo list` | `["acme-test/api"]`; `null`; `[{"thread": "alpha", "value": ["acme-test/api"]}]` |
| 5 | Navigate to `{{base_url}}/#project/beta/profile` | Its "Repositories" row reads "None yet" (Alpha's value isn't Beta's) |
| 6 | `echoctl plugin call thread-demo set '{"thread":"nope","repos":["x"]}' 2>&1 \|\| true` | It fails with "there is no thread “nope”" |
| 7 | In the window, Alpha's Profile → ⋯ → "Delete…" → "Move to trash" | Alpha is gone from the list |
| 8 | `echoctl plugin call thread-demo heard`; `echoctl plugin call thread-demo list` | `heard` includes `{"thread": "alpha", "born": "<Alpha's first stage time>", "change": "deleted", "values": {"repos": ["acme-test/api"]}}` (the plugin's own values for it; `born` is a date-time); `list` is `[]` |
| 8a | `echoctl plugin call thread-demo set '{"thread":"beta","repos":["acme-test/web"]}'`; in a terminal, `echoctl project set beta --name "Beta Two"`; then `echoctl project delete beta`; wait 3 s; `echoctl plugin call thread-demo heard` | `heard` also includes `{"thread": "beta", "born": …, "change": "renamed", "name": "Beta Two", "from": "Beta"}` and `{"thread": "beta", "born": …, "change": "deleted", "values": {"repos": ["acme-test/web"]}}` (the same `born` in both) — the running app's plugin heard the terminal's changes, once each; `<data>/.echo/plugin-events.jsonl` is gone |
| 8b | `echoctl project create "Beta"` (the id `beta` again); navigate to `{{base_url}}/#project/beta/profile` | Its "Repositories" row reads "None yet": a new thread under a used id inherits nothing |
| 9 | `echoctl project create "Boom"`; reload, navigate to `{{base_url}}/#project/boom/profile` | The Profile draws (Stage, Client, … Sources, Notes for Claude, Look, AI) without a "Repositories" row |
| 10 | Navigate to `{{base_url}}/#settings/plugins`, switch "Thread demo on" off; navigate to `{{base_url}}/#project/beta/profile` | No "Repositories" row |

## Expected Final State
Thread demo is installed and off; Alpha and the first Beta are in the trash with no value of the plugin's left; the new Beta and Boom have none.

## Teardown
- `echoctl plugin remove thread-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `echoctl project delete beta`; `echoctl project delete boom`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Step 8's `heard` is what the plugin heard while the app ran (the window's delete runs in the app, where host halves run). A rename or delete from a terminal (step 8a) waits in `<data>/.echo/plugin-events.jsonl` and the running app's plugins hear it within about a second; with the app not running, they hear it when it next starts.
- Step 2's row: `after(box, thread)` runs once per box drawn afresh — a plugin that fetches in `after` and calls `ctx.render()` doesn't draw forever (`node crates/echoctl/ui/thread-slot.test.mjs`).
- Only the app takes `plugin-events.jsonl`: a terminal's `echoctl plugin call` or `echoctl widget show` loads a host half too, but never takes it (`jshost::a_plugin_keeps_values_per_thread_and_hears_when_a_thread_goes`). Taking is at least once: the file is renamed to `plugin-events.jsonl.taking` and removed only after its events were handed on; one left over is handed on first at the next start. A queued `deleted` or `renamed` whose id has another thread by then (another `born`) is dropped (`plugins::thread_values::a_thread_id_used_again_inherits_nothing`).
- `threads.json` is `{"v": 2, "threads": {id: {born, values}}}`; #465's `{id: values}` is read as it was, whatever keys its values have, and rewritten at the next change. A file over 4 MB (from before the cap) still loses values (a key set to null, a thread deleted), never gains them. `ctx.store.remove('')` / `'./'` (the store itself) is refused.
- Echo has no thread-into-thread merge yet: `merged` (with `dropped`: the plugin's keys that didn't fit under 64) is covered by the Rust tests `plugins::thread_values::a_plugins_values_per_thread_are_kept_listed_and_follow_the_thread`.
