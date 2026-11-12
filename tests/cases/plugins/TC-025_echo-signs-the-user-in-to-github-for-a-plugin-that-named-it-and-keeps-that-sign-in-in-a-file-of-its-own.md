---
id: TC-025
title: A plugin that names github.com in uses.signin.device has Echo sign the user in itself — the code and GitHub's address (Echo's own) shown, opened only on the user's click, the sign-in kept in a file of Echo's (0600) and handed back as "echo", refused or signed out leaving nothing
priority: high
type: functional
requirements: [F-PLUG-22]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, client-half, permissions, signin, made-up-tokens]
---

# TC-025: Echo signs the user in to GitHub for a plugin, and keeps that sign-in itself

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- `<dir>` is `<parent>/d` for a new empty `<parent>` (`P=$(mktemp -d)`); the made-up programs and GitHub live next to it, in `<parent>`. Before `serve.sh up`, in that shell — made-up `gh` and `git` with no sign-in first on `PATH`, and the made-up GitHub (`tests/fixtures/fake-github-device.py`), so neither the computer's own sign-in nor the real GitHub is ever reached, by the served window or the CLI:

```bash
P=$(mktemp -d); mkdir -p "$P/fakebin"
G="$(command -v git)"   # the real git, by its path: the made-up one hands it everything but `credential`
printf '#!/bin/sh\necho "not logged in" >&2\nexit 1\n' > "$P/fakebin/gh"
printf '#!/bin/sh\ncase " $* " in *" credential "*) exit 128;; esac\nexec "%s" "$@"\n' "$G" > "$P/fakebin/git"; chmod +x "$P/fakebin/gh" "$P/fakebin/git"
nohup python3 tests/fixtures/fake-github-device.py 47995 "$P/fake-github" > /dev/null 2>&1 & echo $! > "$P/fake-github.pid"
PATH="$P/fakebin:$PATH" ECHOCTL_TEST_HOSTS="github.com=http://127.0.0.1:47995" tests/fixtures/serve.sh up "$P/d" <port> --set standard || exit 1
```
- Every Bash call after starts with the prelude (a fresh shell each time): `. <dir>/env || exit 1; P="$(dirname <dir>)"; F="$P/fake-github"; export PATH="$P/fakebin:$PATH" ECHOCTL_TEST_HOSTS="github.com=http://127.0.0.1:47995"`; `echoctl` is `target/debug/echoctl`
- The window served at `{{base_url}}` with `ECHOCTL_TEST_OPEN_LOG_ONLY` (set by the data's `env`): a link confirmed is written to `$ECHOCTL_TEST_OPEN_LOG`, never opened
- `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `<config dir>/signin/`

## Test Data

| Field | Value |
|---|---|
| Plugin | Sign-in demo (`signin-demo`) 0.1.0: `uses.signin.hosts: ["github.com"]`, `device.github.com`: client id `Ov23liMADEUPDEMO`, scopes `repo`, `read:org` |
| The made-up GitHub's code | `WDJB-MJHT` (device code `dev-madeup-1`) |
| Its token, once "confirmed" | `gho_madeupdevice0000000000000000000000000` (41 characters, scope `repo,read:org`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install signin-demo --yes` | It prints `• Gets the sign-in saved on this computer for github.com (GitHub CLI or git): it can act as you there and send it to the places it talks to; it may ask Echo to sign you in to github.com (you confirm a code on github.com; Echo keeps that sign-in, with access to: repo, read:org) (new)`, then `Installed Sign-in demo 0.1.0` |
| 2 | `echoctl plugin call signin-demo who` | `null` (the made-up programs have no sign-in; Echo has none of its own) |
| 3 | Reload `{{base_url}}/`, open `{{base_url}}/#settings/signin-demo:main` | The section reads "Not signed in to GitHub" with a button "Sign in with GitHub"; `$F/requests.log` doesn't exist (GitHub not asked) |
| 4 | Click "Sign in with GitHub" | The section shows "Enter **WDJB-MJHT** at https://github.com/login/device" and a "Cancel" button; `$F/requests.log`'s first line is `/login/device/code client_id=Ov23liMADEUPDEMO&scope=repo%20read%3Aorg`; then lines `/login/oauth/access_token client_id=Ov23liMADEUPDEMO&device_code=dev-madeup-1&grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code` about one a second; `$ECHOCTL_TEST_OPEN_LOG` has nothing (nothing opened by itself) |
| 5 | Click the link "https://github.com/login/device" | `$ECHOCTL_TEST_OPEN_LOG` has `https://github.com/login/device` (the link in the plugin's own section opens on the user's click; it is Echo's address, never one from GitHub's answer) |
| 6 | `touch "$F/approve"` (the user confirms the code on GitHub); wait up to 5 s | The section reads "Signed in to GitHub by Echo" with a "Sign out" button, no code; the access-token lines in `requests.log` stop |
| 7 | `ls -l "$ECHOCTL_CONFIG_DIR/signin/"`; `grep -c dev-madeup-1 "$ECHOCTL_CONFIG_DIR/signin/github.com.json"`; `grep -rl gho_madeupdevice "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR/plugins" <dir>/*.log 2>/dev/null` | One file `github.com.json`, `-rw-------`; `0` (the device code isn't kept); nothing found (the token is in no other file: not the plugin's store, not a log) |
| 8 | `echoctl plugin call signin-demo who` | `{"from": "echo", "length": 41, "scopes": "repo read:org"}` |
| 9 | Click "Sign out" | The section reads "Not signed in to GitHub"; `$ECHOCTL_CONFIG_DIR/signin/github.com.json` is gone; `echoctl plugin call signin-demo who` → `null` |
| 10 | `rm "$F/approve"; touch "$F/deny"`; click "Sign in with GitHub"; wait up to 5 s | The code shows, then "GitHub said no: not signed in." and "Sign in with GitHub" again; no `github.com.json` |
| 11 | `rm "$F/deny"`; click "Sign in with GitHub", then "Cancel"; `touch "$F/approve"`; wait 3 s | "Not signed in to GitHub"; no `github.com.json` (a sign-in cancelled isn't kept even when GitHub confirms it later); no new access-token line after the cancel |

## Expected Final State
Sign-in demo is installed; Echo keeps no sign-in of its own (`<config dir>/signin/` has no `github.com.json`). Only the made-up GitHub was asked; no real token was read or shown.

## Teardown
- `echoctl plugin remove signin-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`; stop the made-up GitHub (`kill "$(cat "$P/fake-github.pid")"`); close the test tab; remove `<parent>` when the run is over

## Notes
- Never run a step without the prelude's `. <dir>/env` (it stops the step when the data isn't there): without it `echoctl` is the computer's own Echo and its settings.
- A real token on screen or in any file of the run is a FAIL: `gh` and `git` are made-up ones first on `PATH` for the served window and the CLI, and `ECHOCTL_TEST_HOSTS` sends Echo's requests for github.com to the made-up GitHub (a development build only; fails closed).
- The sign-in waits in the process that started it: the window's (the served window here, the app for a user). A terminal's `echoctl plugin call signin-demo start` would end with its process.
- Another host or one named without `device`, `clientId` that isn't a client id, a read-only routine (start and sign-out refused), a code that runs out, GitHub's other errors and `slow_down` are covered by Rust tests (`jshost::tests::a_plugin_has_echo_sign_the_user_in_only_where_its_sheet_said`, `jshost::device::tests::*`); the sheet's line and bad `device` shapes by `plugins::tests::the_sheet_says_whose_sign_in_a_plugin_uses_and_only_those_hosts_are_handed_over`.
- GitHub keeps the app's grant after "Sign out" until the user revokes it at github.com → Settings → Applications; Echo only forgets its file.
