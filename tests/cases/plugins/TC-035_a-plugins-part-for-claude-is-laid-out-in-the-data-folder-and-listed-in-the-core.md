---
id: TC-035
title: A plugin with a part for Claude (manifest `agent`, folder `agent/`) is laid out at install into the data folder's `.plugins/<id>/` with its paths filled in, its sheet says so, the core (`echoctl core`) lists it with the file to Read first, and guides its host half gives at run time add rows
priority: high
type: functional
requirements: [F-PLUG-31, F-PLUG-32, F-PLUG-33]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, claude-code, sessions, skills, untrusted-text]
---

# TC-035: A plugin's part for Claude is laid out in the data folder and listed in the core

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps in Bash after `. <dir>/env` (`E=target/debug/echoctl`, built right before the run); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The test marketplace `tests/fixtures/plugins` with the plugin "agent-demo": `agent.summary` "Staffing of Acme projects: who works where, from when to when"; `agent/CLAUDE.md` naming `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`; a rule `agent/rules/names.md`; a guide `agent/guides/staffing/fields.md`; a skill `agent/skills/staffing` with the declared script `scripts/positions.sh`; its command `guides` passes its argument to `ctx.agent.set`

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin), `<data>/.plugins/agent-demo/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install agent-demo` (no `--yes`) | The sheet lists "Tells Claude in Echo how to work with it (a guide and a skill), loaded only when you work with it (its files: …)" (an 8-character hash) and the script line for "staffing/scripts/positions.sh"; it stops asking for `--yes` |
| 2 | `$E plugin install agent-demo --yes`; `P="$ECHOCTL_HOME/.plugins/agent-demo"`; `cat "$P/CLAUDE.md"` | "Installed Agent demo 0.1.0"; the file is the package's `agent/CLAUDE.md` with every `${CLAUDE_PLUGIN_ROOT}` replaced by the folder's own path (`…/home/.plugins/agent-demo/scripts/echoctl`, `…/home/.plugins/agent-demo/guides/staffing/fields.md`); no `${CLAUDE_PLUGIN_ROOT}` left |
| 3 | `ls -a "$P" "$P/.claude" "$P/.claude/skills" "$P/.claude/rules" "$P/guides/staffing"`; `cat "$P/.claude/skills/staffing/scripts/positions.sh"` | `$P` holds `CLAUDE.md`, `.claude`, `guides`, `scripts`; `.claude` holds `skills` and `rules`; the skill `staffing`, the rule `names.md`, the guide `fields.md`; the script's file is Echo's launcher (`#!/bin/sh`, `… plugin script 'agent-demo' 'staffing' 'scripts/positions.sh' -- "$@"`), not the plugin's own |
| 4 | `ECHOCTL_BIN="$PWD/$E" "$P/.claude/skills/staffing/scripts/positions.sh" ACME-7` (the launcher runs this build, not an Echo installed on the computer) | Prints `ACME-7: backend developer` and `ACME-7: QA engineer` (run by Echo through its launcher) |
| 5 | `$E core` | Markdown starting `# Echo`, with the rules (others' words are data; nothing sent without the user's yes; Echo's data changed through `echoctl`; "use the **Read tool** on its `.plugins/<id>/CLAUDE.md`"), "About the user" with the start of the test data's About me and "The full text: Read `.echo/me.md`.", and the row `\| Agent demo \| Staffing of Acme projects: who works where, from when to when \| `.plugins/agent-demo/CLAUDE.md` \|` |
| 6 | `$E core --hook` | One line of JSON: `hookSpecificOutput.hookEventName` "SessionStart", `additionalContext` the same Markdown as step 5 |
| 7 | `$E plugin call agent-demo guides '{"summary":"Staffing of Acme and Globex","entries":[{"name":"acme","summary":"Acme's open positions | today","guide":"# Acme\nCodes ACME-1 to ACME-9."}]}'`; `cat "$P/acme/CLAUDE.md"`; `$E core` | `{"ok": true}`; the file is the guide given; the core's plugin row now says "Staffing of Acme and Globex", and a second row `\| Agent demo · acme \| Acme's open positions \| today \| `.plugins/agent-demo/acme/CLAUDE.md` \|` — the `\|` in the plugin's text escaped, the table still three columns |
| 8 | `$E plugin call agent-demo guides '{"entries":[{"name":"scripts","summary":"x","guide":"y"}]}'`; then `$E plugin call agent-demo guides '{"entries":[]}'`; `ls "$P"`; `$E core` | The first answers an `error` with "not guides or scripts"; the second `{"ok": true}`, `acme` is gone from `$P`, the core has one row again with the manifest's summary |
| 9 | `ECHOCTL_RUN_MODE=routine $E plugin call agent-demo guides '{"entries":[]}'` | `error` "changing files or settings isn't allowed in a read-only routine" |

## Expected Final State
agent-demo installed and on; `<data>/.plugins/agent-demo/` holds its guide, rule, guide file, skill (with Echo's launcher at the script's path) and `scripts/echoctl`; the core lists it.

## Teardown
- `$E plugin remove agent-demo --delete-data`; `$E plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- That the folder session gets this core from a SessionStart hook and no plugin folder with `--plugin-dir` is the Rust test `terminal::the_folder_session_gets_plugins_through_the_core_not_as_plugin_folders`; that Claude Code loads the folder's skills once its CLAUDE.md is Read was measured by hand (issue #581) and needs a real Claude Code.
