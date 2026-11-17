---
id: TC-091
title: "A plugin's Claude run in its folder takes the user's permission setting for routines, never the plugin's: Auto — auto mode on a Claude profile, edits accepted or a read asking nothing on a local model; Skip all permission checks — every mode skips them"
priority: high
type: functional
requirements: [F-PLUG-63]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders, permissions]
---

# TC-091: A plugin's Claude run in its folder takes the routines' permission setting

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-090`'s preconditions (the served data, the Bash prelude with `D`, `E` and `S`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/tidy` and the plugin Acme tidy, whose sheet allows `claude.skip`), then its step 1 (Acme tidy installed with `$D/tidy` given to it)
- A local model profile of Echo's own whose server is never reached (the fake answers): `$E ai add-local acme-local --model qwen3 --url http://127.0.0.1:9`; routines' permission setting unset (Auto, the default): `grep -c routine_permissions "$ECHOCTL_HOME/.echo/config.json"` prints `0`

**Changes:** `<config dir>/ai.json` (the profile `acme-local`), `config.json` (`routine_permissions`), Acme tidy's store `runs/` and `cache/snapshots/`, `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call acme-tidy run '{"mode":"edit","tools":"skip","dir":"p1"}'`; `grep -A1 '^--permission-mode$' "$D/run-args"`; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `cat "$S/runs/p1/permissions.json"`; `head -1 "$S/runs/p1/log.txt"` | `"answer": "Tidied"`; `--permission-mode` `auto`; `0` — the plugin's own `skip` isn't taken; the record's `"used"` ends `(auto mode: Claude Code decides each action)`; the log's first line ends `(asked: edit; runs with <the record's "used">)` |
| 2 | `$E config --routine-permissions skip > /dev/null 2>&1`; then for each of `read` (`p2`), `edit` (`p3`) and `copy` (`p4`): `$E plugin call acme-tidy run '{"mode":"<mode>","dir":"<p>"}'`; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `grep -c '^--permission-mode$' "$D/run-args"` | Each answers `"answer": "Tidied"`; `1` and `0` for all three; each record's `"used"` ends `(every permission check skipped)` |
| 3 | `$E config --routine-permissions auto > /dev/null 2>&1`; `$E plugin call acme-tidy run '{"mode":"read","profile":"acme-local","dir":"p5"}'`; `grep -A1 '^--permission-mode$' "$D/run-args"`; `$E plugin call acme-tidy run '{"mode":"edit","profile":"acme-local","dir":"p6"}'`; `grep -A1 '^--permission-mode$' "$D/run-args"`; `cat "$S/runs/p6/permissions.json"` | Both answer `"answer": "Tidied"`; `--permission-mode` `dontAsk` for the read, `acceptEdits` for the edit (the local model has no auto mode: never Claude Code's manual mode, where every edit would be refused); the record's `"used"` ends `(edits accepted: this profile's model has no auto mode)` |

## Expected Final State
Routines' setting written as Auto (its default); Acme tidy installed with `$D/tidy` given to it.

## Teardown
- `$E ai remove acme-local`; then `plugins/TC-090`'s teardown

## Notes
- Besides `echoctl`, this case runs `cat`, `grep` and `head`.
- The folder session's and Yolo's settings don't reach a folder run, and a start in another permission mode than asked is said in the run's log and record: the Rust test `claudejob::a_plugins_folder_run_is_that_folders_own_project_with_its_modes_tools_and_the_routines_permissions`. The setting itself, from the window and the CLI: `settings/TC-036`.
