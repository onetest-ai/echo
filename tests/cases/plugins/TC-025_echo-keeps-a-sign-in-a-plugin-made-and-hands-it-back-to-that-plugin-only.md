---
id: TC-025
title: A plugin with uses.signin.keep has Echo keep a sign-in it made — the sheet says so, Echo keeps it in a file of its own (0600) and hands it back first as "echo" to that plugin only, forgets it; keeping is refused for a host it didn't name
priority: high
type: functional
requirements: [F-PLUG-22]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, permissions, signin, made-up-tokens]
---

# TC-025: Echo keeps a sign-in a plugin made

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- `<dir>` is `<parent>/d` for a new empty `<parent>` (`P=$(mktemp -d)`). Made-up `gh` and `git` with no sign-in, first on `PATH` (the made-up `git` hands everything but `credential` to the real one, by its path, so `make_data.py` works):

```bash
P=$(mktemp -d); mkdir -p "$P/fakebin"; G="$(command -v git)"
printf '#!/bin/sh\necho "not logged in" >&2\nexit 1\n' > "$P/fakebin/gh"
printf '#!/bin/sh\ncase " $* " in *" credential "*) exit 128;; esac\nexec "%s" "$@"\n' "$G" > "$P/fakebin/git"; chmod +x "$P/fakebin/gh" "$P/fakebin/git"
PATH="$P/fakebin:$PATH" tests/fixtures/serve.sh up "$P/d" <port> --set standard || exit 1
```
- Every Bash call after starts with the prelude (a fresh shell each time): `. <dir>/env || exit 1; export PATH="$(dirname <dir>)/fakebin:$PATH"`; `echoctl` is `target/debug/echoctl` (by its path: the one on `PATH` may be the computer's own Echo)
- `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugins), `<config dir>/signin/`

## Test Data

| Field | Value |
|---|---|
| Plugin | Sign-in demo (`signin-demo`) 0.1.0: `uses.signin: {hosts: ["github.com"], keep: true}` |
| The sign-in it made | `madeup-sign-in-000000000000000000000000` (39 characters) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install signin-demo --yes` | It prints `• Gets the sign-in saved on this computer for github.com (GitHub CLI or git): it can act as you there and send it to the places it talks to; it may sign you in to github.com itself and have Echo keep that sign-in for it (new)`, then `Installed Sign-in demo 0.1.0` |
| 2 | `echoctl plugin call signin-demo who` | `null` (the made-up programs have no sign-in; Echo keeps none) |
| 3 | `echoctl plugin call signin-demo keep '{"token":"madeup-sign-in-000000000000000000000000"}'` | `true` |
| 4 | `ls -l "$ECHOCTL_CONFIG_DIR/signin/"`; `grep -rl madeup-sign-in "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR/plugins" <dir>/*.log 2>/dev/null` | One file `github.com.json`, `-rw-------`; nothing found (not in the plugin's store, not in a log) |
| 5 | `echoctl plugin call signin-demo who` (a new process: read from Echo's file) | `{"from": "echo", "length": 39}` |
| 6 | `echoctl plugin call signin-demo keep '{"host":"gitlab.com","token":"x"}'`; `echoctl plugin call signin-demo keep '{"token":""}'` | The first fails with `it didn't ask to use your sign-in to “gitlab.com” when it was installed (uses.signin.hosts)`; the second with `a sign-in is one line of text, 8 KB at most`; `github.com.json` unchanged |
| 7 | `echoctl plugin call signin-demo forget '{}'`; again; then `who` | `true`; `false`; `null`; no `github.com.json` |

## Expected Final State
Sign-in demo is installed; Echo keeps no sign-in (`<config dir>/signin/` empty). Only made-up sign-ins were used.

## Teardown
- `echoctl plugin remove signin-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`; remove `<parent>` when the run is over

## Notes
- Never run a step without the prelude's `. <dir>/env` (it stops the step when the data isn't there): without it `echoctl` is the computer's own Echo and its settings.
- Echo doesn't care which service a kept sign-in is for or how the plugin got it (a device code, a browser sign-in): that, and checking it, is the plugin's.
- A plugin whose sheet didn't say `keep` neither keeps nor gets one, a read-only routine keeps and forgets nothing, and a line break in a sign-in is refused: Rust test `jshost::tests::echo_keeps_a_sign_in_a_plugin_made_and_hands_it_back_only_to_a_plugin_that_said_keep`; the sheet's line and a `keep` that isn't true or false: `plugins::tests::the_sheet_says_whose_sign_in_a_plugin_uses_and_only_those_hosts_are_handed_over`.
