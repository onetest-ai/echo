---
id: TC-160
title: A plugin adds a row to each client's Profile and keeps a value per client; the value shows on its client only, stays when the client is renamed, goes when it is removed, and a row that throws leaves the Profile drawn
priority: medium
type: functional
requirements: [F-PLUG-96]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, clients, client-half, slots, host-half]
---

# TC-160: A plugin's row in a client's Profile, and its values per client

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- The test plugin `tests/fixtures/plugins` (marketplace "test-plugins", plugin "client-demo": a "Vault" row in each client's Profile, the vault kept per client; it throws on a client named "Boom")
- The standard set's clients Acme (`acme`) and Globex (`globex`); a made-up client without threads: `echoctl client add "Hooli"` (id `hooli`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `<data>/.echo/plugins/client-demo/`, the clients Hooli and Boom

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install client-demo --yes` | "Installed Client demo 0.1.0" |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#client/acme/profile` | The Profile card has a row "Vault", right under "Threads", reading "The default vault" with a box and "Save" |
| 3 | Type `Acme notes` in the box "Vault", click "Save" | The row shows the chip "▤ Acme notes" |
| 4 | `echoctl plugin call client-demo get '{"client":"acme"}'`; the same for `globex`; then `echoctl plugin call client-demo list` | `"Acme notes"`; `null`; `[{"client": "acme", "value": "Acme notes"}]` |
| 5 | Navigate to `{{base_url}}/#client/globex/profile` | Its "Vault" row reads "The default vault" (Acme's value isn't Globex's) |
| 6 | `echoctl plugin call client-demo set '{"client":"nope","vault":"x"}' 2>&1 \|\| true` | It fails with "there is no client “nope”" |
| 7 | `echoctl plugin call client-demo set '{"client":"hooli","vault":"Hooli notes"}'`; `echoctl client set hooli --name "Hooli XYZ"`; `echoctl plugin call client-demo get '{"client":"hooli"}'` | `"Hooli notes"` after the rename: the value follows the client; `echoctl client list` names it "Hooli XYZ" |
| 8 | `echoctl client remove hooli`; `echoctl plugin call client-demo list`; `echoctl client add "Hooli"`; `echoctl plugin call client-demo get '{"client":"hooli"}'` | `list` is `[{"client": "acme", "value": "Acme notes"}]`; the new Hooli (the id `hooli` again) gets `null`: it inherits nothing |
| 9 | `echoctl client add "Boom"`; reload, navigate to `{{base_url}}/#client/boom/profile` | The Profile draws (Name, Other names, Mail domains, Kind, Logo, Threads, "Remove this client…") without a "Vault" row |
| 10 | Navigate to `{{base_url}}/#settings/plugins`, switch "Client demo on" off; navigate to `{{base_url}}/#client/acme/profile` | No "Vault" row |

## Expected Final State
Client demo is installed and off; its value for Acme is kept; Hooli (new) and Boom have none.

## Teardown
- `echoctl plugin remove client-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `echoctl client remove hooli`; `echoctl client remove boom`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- A client is one with a page (an account); an organisation that isn't one is refused like a client there isn't (`plugins::client_values::a_plugins_values_per_client_are_kept_listed_and_follow_the_client`).
- The limits (key 1–80 characters, value JSON ≤ 16 KB, 64 keys a client, 4 MB a plugin, a key with a dot refused, a file that isn't its shape moved aside) are the Rust tests' (`plugins::client_values::client_values_have_the_thread_values_limits`); `client-values.json` isn't the plugin's to write or remove through `ctx.store` (`jshost::a_plugin_keeps_its_own_values_per_client_and_they_go_with_the_client`).
- Step 2's row: `after(box, client)` runs once per box drawn afresh, and again for another client (`node crates/echoctl/ui/client-slot.test.mjs`).
