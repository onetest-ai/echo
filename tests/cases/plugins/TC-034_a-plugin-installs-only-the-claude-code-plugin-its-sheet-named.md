---
id: TC-034
title: A plugin may install only the Claude Code plugin its install sheet named (its channel plugin); the sheet says it; any other is refused before Claude Code runs
priority: high
type: functional
requirements: [F-PLUG-30]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, claude-code, permissions, security]
---

# TC-034: A plugin installs only the Claude Code plugin its sheet named

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps in Bash after `. <dir>/env` (`E=target/debug/echoctl`)
- The test marketplace `tests/fixtures/plugins` with the plugin "chan-demo" (its manifest names the channel plugin `telegram@claude-plugins-official`; its command `install` asks Echo to install the spec it is given)

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install chan-demo --yes` | The sheet lists "Lets you reach your Claude · Echo folder session through Telegram while it is open in Echo, and installs Claude Code's telegram@claude-plugins-official plugin for it"; "Installed Channel demo 0.1.0" |
| 2 | `$E plugin call chan-demo install '{"spec":"evil@somewhere"}' 2>&1 \|\| true` | Refused: "chan-demo may install only the Claude Code plugin its install sheet named (uses.channel.plugin), not “evil@somewhere”"; nothing installed in Claude Code (the refusal comes before it runs) |
| 3 | `ECHOCTL_RUN_MODE=routine $E plugin call chan-demo install '{"spec":"telegram@claude-plugins-official"}' 2>&1 \|\| true` | Refused: "installing isn't allowed in a read-only routine" |

## Expected Final State
chan-demo installed; nothing installed in any Claude Code.

## Teardown
- `$E plugin remove chan-demo --delete-data`; `$E plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- The named plugin itself isn't installed here (that would change the test profile's Claude Code); the Rust test `jshost::a_plugin_installs_only_the_claude_code_plugin_its_sheet_named_and_never_in_a_routine` covers the same refusals.
