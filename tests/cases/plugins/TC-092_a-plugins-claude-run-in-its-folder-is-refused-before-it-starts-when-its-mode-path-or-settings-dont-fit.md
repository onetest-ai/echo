---
id: TC-092
title: A plugin's Claude run in its folder is refused before Claude Code starts — a mode Echo doesn't know, a path out of the folder or not a folder, a copy into what a copy leaves out (its snapshot gone too), a folder whose Claude Code settings could route the run
priority: high
type: functional
requirements: [F-PLUG-63]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders, security]
---

# TC-092: A plugin's Claude run in its folder: what is refused

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-090`'s preconditions (the served data, the Bash prelude with `D`, `E` and `S`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/tidy` and the plugin Acme tidy), then its step 1 (Acme tidy installed with `$D/tidy` given to it)
- No run since: `rm -f "$D/run-args"`

**Changes:** `$D/tidy/.git/` and `$D/tidy/.claude/settings.json` (made and removed here), Acme tidy's store `cache/snapshots/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call acme-tidy run '{"mode":"bypassPermissions","dir":"x1"}'`; `$E plugin call acme-tidy run '{"mode":"read","path":"../","dir":"x2"}'`; `$E plugin call acme-tidy run '{"mode":"read","path":"Notes/Acme.md","dir":"x3"}'`; `ls "$D/run-args"` | `{"error": "mode “bypassPermissions”: read, edit or copy"}`; an `error` containing `isn't a path inside`; one containing `isn't a folder`; `ls` fails (No such file or directory): nothing started |
| 2 | `mkdir -p "$D/tidy/.git/refs"`; `ls "$S/cache/snapshots" 2>/dev/null \| wc -l` (note it as `<n>`); `$E plugin call acme-tidy run '{"mode":"copy","path":".git","dir":"x4"}'`; `ls "$S/cache/snapshots" 2>/dev/null \| wc -l`; `ls "$D/run-args"` | An `error` containing `isn't in the copy of` (a copy leaves `.git` out); still `<n>` snapshots: the one made for it is gone; `ls` fails: nothing started |
| 3 | `printf '{"env":{"ANTHROPIC_BASE_URL":"https://gw.acme.example"}}' > "$D/tidy/.claude/settings.json"`; `$E plugin call acme-tidy run '{"mode":"read","dir":"x5"}'`; `ls "$D/run-args"` | An `error` containing `this folder's Claude Code settings (.claude/settings.json) could send the run elsewhere than` and the profile's name; `ls` fails: nothing started |

## Expected Final State
Acme tidy installed with `$D/tidy` given to it; `$D/tidy` without `.git` and `.claude/settings.json` again (teardown).

## Teardown
- `rm -r "$D/tidy/.git" "$D/tidy/.claude/settings.json"`; then `plugins/TC-090`'s teardown

## Notes
- Besides `echoctl`, this case runs `ls`, `wc`, `mkdir`, `printf` and `rm`.
- Also refused before a run, by the Rust test `jshost::folders::a_claude_run_goes_into_a_folder_given_to_the_plugin_edits_only_with_write_and_copy_answers_its_snapshot`: no `uses.folders`, a folder not given to the plugin, a path or mode without a folder, `edit` without `folders.write`.
