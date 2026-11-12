---
id: TC-024
title: A plugin that names github.com in uses.signin gets the sign-in saved on this computer from the GitHub CLI, else from git, else null — the sheet says it can act as you there, any other host is refused and no network comes with it
priority: high
type: functional
requirements: [F-PLUG-21]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, permissions, signin, made-up-tokens]
---

# TC-024: A plugin uses the sign-in on this computer, only for the hosts it named

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- Made-up `gh` and `git` in `$D/fakebin`, put first on `PATH` for each `echoctl plugin call` (so the computer's own sign-in is never read); every token is made up. A made-up plugin **Acme hub** in a local marketplace `acme-local` that answers `who` with where the sign-in came from and its length, never the token itself:

```bash
mkdir -p "$D/fakebin"; M="$D/market"; mkdir -p "$M/plugins/acme-hub"
echo '{"name":"acme-local","plugins":[{"id":"acme-hub","source":"./plugins/acme-hub"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-hub","name":"Acme hub","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"signin":{"hosts":["github.com"]}}}' > "$M/plugins/acme-hub/echo-plugin.json"
cat > "$M/plugins/acme-hub/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('who', async (a) => { const s = await ctx.signin.get(a.host); return s && { from: s.from, user: s.user || null, length: s.token.length } })
  ctx.command('sources', (a) => ctx.signin.sources(a.host))
  ctx.command('fetch', async () => (await ctx.http.fetch('https://api.github.com/zen')).status)
}
JS
fake() { printf '#!/bin/sh\necho "$*" >> "%s/fakebin/%s.log"; cat >> "%s/fakebin/%s.log"\n%s\n' "$D" "$1" "$D" "$1" "$2" > "$D/fakebin/$1"; chmod +x "$D/fakebin/$1"; }
```
- `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `$D/fakebin`

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme hub (`acme-hub`) 0.1.0, `uses.signin.hosts: ["github.com"]`, no `network` |
| GitHub CLI token | `gho_madeup0000000000000000000000000000000` (41 characters) |
| git's credential | user `ann`, password `ghp_madeup11111` (15 characters) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-hub --yes` | It prints `• Gets the sign-in saved on this computer for github.com (GitHub CLI or git): it can act as you there and send it to the places it talks to (new)` and no `Talks to` line; then `Installed Acme hub 0.1.0` |
| 2 | `fake gh 'echo gho_madeup0000000000000000000000000000000'; fake git "printf 'protocol=https\nhost=github.com\nusername=ann\npassword=ghp_madeup11111\n'"`; `PATH="$D/fakebin:$PATH" echoctl plugin call acme-hub sources '{"host":"github.com"}'` | `{"gh": true, "git": true}`; neither log file exists yet (nothing was read) |
| 3 | `PATH="$D/fakebin:$PATH" echoctl plugin call acme-hub who '{"host":"github.com"}'` | `{"from": "gh", "length": 41, "user": null}`; `gh.log` has `auth token --hostname github.com`; there is no `git.log` (git wasn't asked) |
| 4 | `fake gh 'echo "not logged in" >&2; exit 1'`; run step 3's call again | `{"from": "git", "length": 15, "user": "ann"}`; `git.log` has `credential fill` and the lines `protocol=https`, `host=github.com`, and no `approve`, `reject` or `erase` |
| 5 | `fake git 'exit 128'`; run step 3's call again | `null` |
| 6 | `PATH="$D/fakebin:$PATH" echoctl plugin call acme-hub who '{"host":"gitlab.com"}'`, then the same with `sources` | Both fail with `it didn't ask to use your sign-in to “gitlab.com” when it was installed (uses.signin.hosts)`; no new line in either log |
| 7 | `echoctl plugin call acme-hub fetch` | It fails with `acme-hub may reach only the hosts its manifest names` — the sign-in brought no network with it |
| 8 | Reload `{{base_url}}/`, open `{{base_url}}/#settings/plugins`, click the row "Acme hub" | "What it can do" lists "Gets the sign-in saved on this computer for github.com (GitHub CLI or git): it can act as you there and send it to the places it talks to" |

## Expected Final State
Acme hub is installed; it was handed the made-up CLI token, then git's made-up credential, then nothing, and was refused for any other host and any network it didn't name. Echo asked for no password at any step (the made-up programs use no keychain).

## Teardown
- `echoctl plugin remove acme-hub --delete-data`; `echoctl plugin marketplace remove acme-local`; `rm -rf "$D/fakebin"`
- `tests/fixtures/serve.sh down <dir>`; close the test tab; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-PLUG-21). A real token on screen or in any file of the run is a FAIL: every program asked here is a made-up one first on `PATH`.
- A read-only routine refusing `ctx.signin.get`, a program that hangs past 10 s (stopped with what it started), a helper planted in a repository or in an inherited `GIT_CONFIG_PARAMETERS` (never run) and one lookup per host at a time, kept a minute in the running app (each `echoctl plugin call` is a new process), are covered by Rust tests (`jshost::a_plugin_is_handed_a_sign_in_only_for_the_hosts_its_sheet_named_and_never_in_a_read_only_routine`, `plugins::signin::tests::*`).
- With a real GitHub CLI or git, the system's keychain may ask once to let it hand the sign-in over; Echo itself never asks for a password.
