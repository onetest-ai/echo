---
id: TC-037
title: `echoctl plugin check <folder>…` checks plugin packages as an install would and more, with no data folder — a clean one "ok" (exit 0), a broken one each problem by file (exit 1), a hidden folder and a skill named past 64 characters refused, `--json` as {ok, problems, plugins}, several folders each by id
priority: high
type: functional
requirements: [F-PLUG-34]
module: plugins
run: agent
platform: any
data: none
tags: [plugins, cli, authoring]
---

# TC-037: `plugin check` names each problem of a package

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- `E=target/debug/echoctl`, built right before the run. No test data and **no** `<dir>/env`: the command reads only the folders it is given; every step runs it in an empty environment (`env -i HOME=/nonexistent PATH=/usr/bin:/bin`), so it can reach no data folder or settings
- The fixtures: `tests/fixtures/plugins/plugins/agent-demo` (a clean plugin with an `agent/` part), `tests/fixtures/plugin-check/agent-hidden` (`agent/guides/.claude/skills/evil/SKILL.md` and `agent/skills/ok/.claude/skills/evil2/SKILL.md`), `tests/fixtures/plugin-check/agent-long-skill-names-test` (a 27-character id and two 40-character skills: 68 as Echo names them) and `tests/fixtures/plugin-check/agent-broken` (each file breaks one rule: no `agent/CLAUDE.md`, `agent/notes.md` and `agent/rules/style.txt` where they can't be, a skill folder `lookup` whose `SKILL.md` says `name: search`, a widget of kind `gauge`, `${CLAUDE_PLUGIN_ROOT}` in its window half `client.js`)

**Changes:** nothing

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `env -i HOME=/nonexistent PATH=/usr/bin:/bin $E plugin check tests/fixtures/plugins/plugins/agent-demo; echo "exit $?"` | `agent-demo (tests/fixtures/plugins/plugins/agent-demo): ok`; `exit 0` |
| 2 | `env -i HOME=/nonexistent PATH=/usr/bin:/bin $E plugin check tests/fixtures/plugin-check/agent-broken; echo "exit $?"` | `agent-broken (…): 6 problems`, then one line each: `agent/notes.md: agent/ holds only CLAUDE.md, skills/, rules/ and guides/`; `agent/rules/style.txt: agent/rules/ holds .md files only`; `agent/CLAUDE.md: missing: agent/ needs its CLAUDE.md`; `agent/skills/lookup/SKILL.md: skill “agent/skills/lookup”: SKILL.md's name is its folder's, “lookup”`; `echo-plugin.json: widget “count” has a kind Echo doesn't draw (…)`; `client.js: ${CLAUDE_PLUGIN_ROOT} is filled in only in what Claude reads (…)`; `exit 1` |
| 3 | `env -i HOME=/nonexistent PATH=/usr/bin:/bin $E plugin check --json tests/fixtures/plugin-check/agent-broken tests/fixtures/plugins/plugins/agent-demo; echo "exit $?"` | JSON: `ok` false; `problems` the six of step 2, each `{id: "agent-broken", file, what}`; `plugins` `[{id: "agent-broken", ok: false, …}, {id: "agent-demo", ok: true, …}]`; `exit 1` |
| 4 | `env -i HOME=/nonexistent PATH=/usr/bin:/bin $E plugin check tests/fixtures/plugins/plugins/*; echo "exit $?"` | One `…: ok` line per test plugin, each by its id (agent-demo, chan-demo, chart-demo, hang-demo, share-a, share-b, signin-demo, table-demo, thread-demo, widget-demo, work-demo); `exit 0` |
| 4a | `env -i HOME=/nonexistent PATH=/usr/bin:/bin $E plugin check tests/fixtures/plugin-check/agent-hidden; echo "exit $?"` | `agent-hidden (…): 2 problems`: `agent/skills/ok/.claude: hidden: agent/ holds no hidden file or folder` and `agent/guides/.claude: hidden: …` (skills hidden in nested `.claude/` folders, which Claude Code would load unchecked); `exit 1` |
| 4b | `env -i HOME=/nonexistent PATH=/usr/bin:/bin $E plugin check tests/fixtures/plugin-check/agent-long-skill-names-test; echo "exit $?"` | `agent-long-skill-names-test (…): 2 problems`, one per skill — the manifest's `skills/a-manifest-skill-name-of-forty-character` and `agent/skills/a-skill-name-of-exactly-forty-characters` —, each `… Echo's sessions name it “agent-long-skill-names-test-<skill>”, 68 characters: a skill's name is 64 at most (the plugin's id, “-”, the skill's)`; `exit 1` |
| 5 | `ls /nonexistent` | Fails ("No such file or directory"): nothing was written anywhere for the checks |

## Expected Final State
Nothing changed.

## Teardown
- None
