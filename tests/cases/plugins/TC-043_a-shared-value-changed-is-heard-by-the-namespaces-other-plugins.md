---
id: TC-043
title: "A value a marketplace's plugins share, changed from a terminal's plugin call, is heard by the namespace's other plugins (shared_changed) — not by the one that set it, not by a copy of another source, not for the same value again"
priority: high
type: functional
requirements: [F-PLUG-29, F-PLUG-36]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, events, shared-values, host-half]
---

# TC-043: shared_changed

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (the served window stands in for the app and hears what a terminal queues); `{{base_url}}/` loaded once after the installs below; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`)
- The test marketplace `tests/fixtures/plugins` ("test-plugins", namespace `test-plugins`) with "share-a" and "share-b" (both hear `shared_changed`; `heardShared` answers what they heard)
- A copy under another source calling itself the same: `C=$(mktemp -d)`; `mkdir -p $C/plugins`; `cp -R tests/fixtures/plugins/plugins/share-a $C/plugins/share-c`; in `$C/plugins/share-c/echo-plugin.json` the id `share-c`, the name "Share C"; `$C/echo-marketplace.json` = `{"name": "test-plugins", "plugins": [{"id": "share-c", "source": "./plugins/share-c"}]}`
- A made-up thread: `$E project create "Alpha"` (id `alpha`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin marketplace add "$C" --name copy`; `$E plugin install share-a --from test-plugins` | The sheet has "Knows when a thread is renamed, merged or deleted and a value its marketplace's plugins share changes", then asks for `--yes` |
| 2 | Install share-a (`--yes`), share-b, share-c (`--from copy --yes`); load `{{base_url}}/` | Each installed |
| 3 | `$E plugin call share-a sset '{"key":"test-plugins.watch","value":["ACME-1"]}'`; wait 3 s; `heardShared` from share-b, share-a, share-c | share-b: `[{"namespace": "test-plugins", "key": "test-plugins.watch", "by": "share-a"}]`; share-a: `[]` (it set it); share-c: `[]` (another source) |
| 4 | The same `sset` again; wait 3 s; share-b's `heardShared` | Still one entry: the same value is no news |
| 5 | `$E plugin call share-b tset '{"thread":"alpha","key":"test-plugins.codes","value":["NWB-PAI"]}'`; wait 3 s; share-a's `heardShared` | `[{"namespace": "test-plugins", "key": "test-plugins.codes", "by": "share-b", "thread": "alpha"}]` |
| 6 | `$E plugin call share-a tset '{"thread":"alpha","key":"codes","value":"mine"}'`; wait 3 s; share-b's `heardShared` | Unchanged: a key with no dot is share-a's own, not shared |
| 7 | Teardown below | — |

## Teardown
- `$E plugin remove share-a --delete-data`; `$E plugin remove share-b --delete-data`; `$E plugin remove share-c --delete-data`; `$E plugin marketplace remove copy`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C"`; `$E project delete alpha`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A terminal's change waits in `<data>/.echo/plugin-events.jsonl` for the app (here the served window), which takes it within about a second, or at its next start. In the app itself, the plugins hear it at once: the Rust test `jshost::tests::a_shared_value_changed_is_told_to_its_namespaces_other_plugins_once_it_changes`.
- `meeting_soon` now names its threads (`threads`: ids, [] when none known): the Rust test `watch::tests` (for_plugins).
