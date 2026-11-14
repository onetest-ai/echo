---
id: TC-033
title: Plugins of one marketplace share a value per thread and one for the computer under <ns>.<name>; another source with the same name only reads; a deleted thread's shared values go and its sharers hear them
priority: medium
type: functional
requirements: [F-PLUG-29]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, threads, host-half, shared]
---

# TC-033: Plugins of a marketplace share values

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps in Bash after `. <dir>/env` (`E=target/debug/echoctl`)
- The test marketplace `tests/fixtures/plugins` ("test-plugins": its namespace is `test-plugins`) with the plugins "share-a" and "share-b" (commands `tset`, `tget`, `tlist`, `sset`, `sget`, `heard`)
- A copy of it under another source that calls itself the same: `C=$(mktemp -d)`; `mkdir -p $C/plugins`; `cp -R tests/fixtures/plugins/plugins/share-a $C/plugins/share-c`; in `$C/plugins/share-c/echo-plugin.json` the id `share-c`, the name "Share C"; `$C/echo-marketplace.json` = `{"name": "test-plugins", "plugins": [{"id": "share-c", "source": "./plugins/share-c"}]}`
- A made-up thread: `$E project create "Alpha"` (id `alpha`)

**Changes:** `<config dir>/plugins/` (two marketplaces, three plugins), `<data>/.echo/plugins/@test-plugins/`, the thread Alpha

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install share-a --yes`; `$E plugin install share-b --yes`; then `$E plugin marketplace add "$C" --name copy`; `$E plugin install share-c --from copy --yes`; reload `{{base_url}}/` (the served window loads the plugins, standing in for the app) | "Installed Share A 0.1.0", "Installed Share B 0.1.0", "Installed Share C 0.1.0" |
| 2 | `$E plugin call share-a tset '{"thread":"alpha","key":"test-plugins.codes","value":["ACME-1"]}'`; then `tget` with `{"thread":"alpha","key":"test-plugins.codes"}` from share-b and from share-c | Both answer `["ACME-1"]`: one value, every plugin reads it |
| 3 | `$E plugin call share-b tset '{"thread":"alpha","key":"test-plugins.codes","value":["ACME-1","ACME-2"]}'`; `$E plugin call share-a tlist '{"key":"test-plugins.codes"}'` | `[{"thread": "alpha", "value": ["ACME-1", "ACME-2"]}]` |
| 4 | `$E plugin call share-c tset '{"thread":"alpha","key":"test-plugins.codes","value":[]}' 2>&1 \|\| true` | Refused: "only the plugins of the marketplace that owns “test-plugins” (…tests/fixtures/plugins) set its values"; share-a's `tget` still `["ACME-1", "ACME-2"]` |
| 5 | `$E plugin call share-a tset '{"thread":"alpha","key":"codes","value":"mine"}'`; `$E plugin call share-b tget '{"thread":"alpha","key":"codes"}'` | `null`: a key without a dot is share-a's own |
| 6 | `$E plugin call share-a tset '{"thread":"alpha","key":"nobody.x","value":1}' 2>&1 \|\| true`; `$E plugin call share-a tget '{"thread":"alpha","key":"nobody.x"}'`; the `tset` with the key `Test-plugins.x` | "no installed marketplace is named “nobody”"; `null` (a namespace nobody owns reads empty); "a shared key is <namespace>.<name>, …" |
| 7 | `$E plugin call share-b sget '{"key":"test-plugins.watch"}'`; `$E plugin call share-b sset '{"key":"test-plugins.watch","value":["ACME-1"]}'`; `sget` from share-a and share-c | `null` first; then `["ACME-1"]` from both |
| 8 | `$E plugin call share-c sset '{"key":"test-plugins.watch","value":[]}' 2>&1 \|\| true` | Refused as in step 4 |
| 9 | `$E project delete alpha`; wait 3 s; `$E plugin call share-a heard`; `$E plugin call share-b heard`; `$E plugin call share-c heard`; `$E plugin call share-a tlist '{"key":"test-plugins.codes"}'` | share-a heard `deleted` for alpha with `values` `{"codes": "mine", "test-plugins.codes": ["ACME-1", "ACME-2"]}`; share-b with `{"test-plugins.codes": ["ACME-1", "ACME-2"]}`; share-c with `{}`; `tlist` `[]` |
| 10 | `$E plugin remove share-a --delete-data`; `$E plugin remove share-b --delete-data`; `$E plugin call share-c sget '{"key":"test-plugins.watch"}'`; `$E plugin call share-c sset '{"key":"test-plugins.watch","value":[]}'` | `["ACME-1"]` (removing the plugins left it); the set now passes: the copy owns the name |

## Expected Final State
share-c installed from `copy`; `test-plugins.watch` is `[]`; Alpha in the trash.

## Teardown
- `$E plugin remove share-c --delete-data`; `$E plugin marketplace remove copy`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C"`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Step 9 needs the served window running (it stands in for the app and hears a terminal's delete, as TC-023's step 8a). The Rust test `jshost::a_marketplaces_plugins_share_values_under_its_name_and_only_they_set_them` covers the same in-process.
