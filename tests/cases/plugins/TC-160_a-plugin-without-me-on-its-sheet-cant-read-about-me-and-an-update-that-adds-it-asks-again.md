---
id: TC-160
title: A plugin without “me” on its sheet can't read About me; an update that adds it asks again with Echo's line, and only the user's yes takes it
priority: medium
type: functional
requirements: [F-PLUG-100]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, about-me, sheet, update]
---

# TC-160: A plugin without “me” can't read About me, and an update that adds it asks again

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A copy of the test marketplace (its me-demo gets updated): `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `$E plugin marketplace add "$C" --name test-plugins`

**Changes:** the installed plugins, `$C/plugins/me-demo/`

## Test Data

| Plugin | Me demo (`me-demo`) 0.1.0: a host half whose command `me` answers `ctx.me(<its arguments>)`, nothing in `uses`; 0.2.0 (`$C/next/me-demo`): the same, with `"uses": {"me": {}}` |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install me-demo --from test-plugins --yes` | The sheet has only "Runs its own code in Echo, with your rights"; `Installed Me demo 0.1.0` |
| 2 | `$E plugin call me-demo me` | Exit 1: `Error: me-demo didn't ask for “me” when it was installed` — nothing of About me in the output |
| 3 | `cp -R "$C/next/me-demo/." "$C/plugins/me-demo/"`; `$E plugin marketplace update test-plugins`; `$E plugin update me-demo` | Asks again: "Me demo 0.2.0", the line "Reads your About me in Echo (Settings → About me): your name and role, the people and terms of your calls, how you like things written and how you work (new)"; `Error: run again with --yes to agree` |
| 4 | `$E plugin call me-demo me` | Still refused as in step 2: the update wasn't taken |
| 5 | `$E plugin update me-demo --yes` | The same sheet; `Updated Me demo to 0.2.0` |
| 6 | `$E plugin call me-demo me` | Exit 0: a JSON object with `text`, `name`, `parts` and `hash` (its values: TC-161) |

## Expected Final State
Me demo 0.2.0 installed and on, asking for About me; About me unchanged.

## Teardown
- In S-41: keep Me demo and the marketplace copy for stage 2. On its own: `$E plugin remove me-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cp`, `mktemp` and `rm` on its own copy of the marketplace.
- Rust: `jshost::a_plugin_reads_about_me_only_with_uses_me_in_the_parts_it_asks_and_never_the_template` (refused, update asks again), `plugins::capabilities::a_plugin_asks_for_about_me_as_an_empty_object_and_the_sheet_says_all_it_may_read` (`me` is `{}` only).
