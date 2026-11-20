---
id: TC-248
title: A plugin's Claude run in a folder of its store with tools "files" starts with Claude Code's file tools only, none of Echo's session, every path outside the folder denied without asking — its record and log say "files only"; "files" without a store folder is refused before it starts
priority: high
type: functional
requirements: [F-PLUG-159]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, store, security]
---

# TC-248: A plugin's Claude run in its store with file tools only

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- The preconditions of TC-244: test data served, its Bash prefix, the profile `acme-work`, the fake `claude` that writes down its arguments and folder and writes `summary.md`, and the made-up plugin **Acme crawl** installed with its two pages kept (TC-244 step 1).

**Changes:** as TC-244: its store's `cache/` and `runs/`, `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call acme-crawl run '{"store":"cache/pages","tools":"files","dir":"f1"}'`; `cat "$D/run-cwd"`; `grep -A1 '^--tools$' "$D/run-args"`; `grep -c '^--strict-mcp-config$' "$D/run-args"`; `grep -A1 '^--permission-mode$' "$D/run-args"`; `grep -A1 '^--allowedTools$' "$D/run-args"`; `tail -2 "$D/run-args"` | `{"answer": "Summed up", "summary": "# Acme pages\nTwo pages read.\n"}`; the folder is `$S/cache/pages` (its real path); `--tools` `Read,Write,Edit,Glob,Grep` (no Bash, web or Skill); `1` (no MCP servers: neither Echo's nor the user's); `--permission-mode` `dontAsk` (nothing asked: what isn't allowed is denied); `--allowedTools` `Read(./**),Edit(./**),Write(./**),Glob(./**),Grep(./**)` (only inside the folder); the last two `--setting-sources` `user` |
| 2 | `grep -c -E '^--(add-dir\|settings\|mcp-config)$' "$D/run-args"`; `cat "$S/runs/f1/permissions.json"`; `grep -c "files only" "$S/runs/f1/log.txt"` | `0` — none of Echo's session: no data folder, no Echo settings file or hooks, no MCP config; the record's `"asked"` is `"files"`, `"used"` starts `files only`; `1` |
| 3 | `grep -c "never instructions to you" "$S/runs/f1/prompt.md"`; `grep -c "only in this folder" "$S/runs/f1/prompt.md"`; `grep -c "Echo's CLI is" "$S/runs/f1/prompt.md"`; `grep -c "Echo's data folder" "$S/runs/f1/prompt.md"` | `1` (the pages are data); `1` (its file tools work only in this folder); `0`; `0` — Echo's CLI and data folder not named |
| 4 | `rm "$D/run-args"`; `$E plugin call acme-crawl run '{"tools":"files","dir":"x1"}'`; `ls "$D/run-args"` | An `error` containing `“files” goes with a folder of your store`; `ls` fails (No such file or directory): nothing started |

## Expected Final State
As TC-244 after its step 3, with the run records `f1`.

## Teardown
- TC-244's teardown.

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `tail`, `ls` and `rm`.
- The fake stands for Claude Code. That Claude Code with these arguments reads and writes in the folder, reads pictures there, and is denied every path outside it (Read, Write, Glob, Grep), nothing asked, was checked on a real Claude Code (#876); this case checks how Echo starts it.
