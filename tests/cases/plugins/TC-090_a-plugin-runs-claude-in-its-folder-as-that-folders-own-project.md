---
id: TC-090
title: A plugin runs Claude Code in a folder the user gave it as that folder's own project — read, edit (with folders.write) and copy (a snapshot it answers), only the mode's tools, Claude Code's auto mode, no prompts for edit and copy only with the user's "Claude acts on its own" on
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

# TC-090: A plugin's Claude run in its folder

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data served: `tests/fixtures/serve.sh up <dir> <port>` (in S-12, the scenario's own); the window is needed only for the setting in step 5; CLI steps in Bash, each starting with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; E=target/debug/echoctl` (inside the repository, built right before the run; a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A Claude AI profile of Echo's own on the data's fake (never a real account): `$E ai add-claude acme-work --email sam@initech.example`
- **A fake `claude` that writes down how Echo starts it and answers every run "Tidied"**, in front of the data's fake: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # --version and auth go to the data's fake; a run writes its arguments one a line and its folder, then answers.
  d="$(dirname "$0")/.."
  case "$1" in --version|auth) exec "$d/bin/claude.fake" "$@";; esac
  printf '%s\n' "$@" > "$d/run-args"; pwd > "$d/run-cwd"
  echo '{"type":"result","subtype":"success","result":"Tidied","usage":{}}'
  ```
- A made-up folder with its own Claude Code setup, and the made-up plugin **Acme tidy** (Claude runs, a folder it may change) in a local marketplace `acme-tidy`:

```bash
mkdir -p "$D/tidy/Notes" "$D/tidy/.claude/skills/tidy"
printf '# Acme notes\nKeep every note under 20 lines.\n' > "$D/tidy/CLAUDE.md"
printf -- '---\nname: tidy\ndescription: Tidy a note.\n---\n\nShorten it.\n' > "$D/tidy/.claude/skills/tidy/SKILL.md"
printf '# Acme\nA long note.\n' > "$D/tidy/Notes/Acme.md"
M="$D/market-tidy"; mkdir -p "$M/plugins/acme-tidy"
echo '{"name":"acme-tidy","plugins":[{"id":"acme-tidy","source":"./plugins/acme-tidy"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-tidy","name":"Acme tidy","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"claude":{},"folders":{"what":"notes","write":true}}}' > "$M/plugins/acme-tidy/echo-plugin.json"
cat > "$M/plugins/acme-tidy/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('your notes'))
  ctx.command('run', async (a) => {
    try {
      const r = await ctx.claude.run({ prompt: 'Tidy the notes', profile: 'acme-work', folder: ctx.fs.folders()[0], path: a.path, mode: a.mode, dir: 'runs/' + a.dir })
      return { answer: r.answer, snapshot: r.snapshot ? r.snapshot.path : null }
    } catch (e) { return { error: String(e.message || e) } }
  })
}
JS
$E plugin marketplace add "$M" --name acme-tidy
```

**Changes:** `<config dir>/ai.json` (the profile), `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), its store's `runs/` and `cache/snapshots/`, `$D/run-*`, `config.json` (step 5)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install acme-tidy --yes`; `ECHOCTL_PICK_FOLDER="$D/tidy" $E plugin call acme-tidy add`; `$E plugin list` and find the installed `acme-tidy` | `Installed Acme tidy 0.1.0`; the folder's path; among its `uses` the Claude line ending `In a folder you gave it, Claude works with that folder's own Claude Code setup: it reads the folder and, if the plugin may change it, edits files there or in a copy for you to review — without asking when Claude acts on its own` |
| 2 | `$E plugin call acme-tidy run '{"mode":"read","dir":"r1"}'`; `cat "$D/run-cwd"`; `grep -E -A1 '^--(tools\|permission-mode\|setting-sources)$' "$D/run-args"`; `grep -c -E '^--(dangerously-skip-permissions\|settings\|plugin-dir\|mcp-config\|allowedTools)$' "$D/run-args"`; `grep -c '^--strict-mcp-config$' "$D/run-args"` | `{"answer": "Tidied", "snapshot": null}`; the folder `$D/tidy` (its real path); `--tools` `Read,Grep,Glob,Skill`, `--permission-mode` `auto`, `--setting-sources` `user,project` (the folder's own CLAUDE.md, skills and settings load), each once; `0` (nothing of Echo's session, no prompts skipped); `1` |
| 3 | `$E plugin call acme-tidy run '{"mode":"edit","path":"Notes","dir":"r2"}'`; `cat "$D/run-cwd"`; `grep -E -A1 '^--(tools\|permission-mode)$' "$D/run-args"`; `grep -c -E '^--dangerously-skip-permissions$' "$D/run-args"` | `{"answer": "Tidied", "snapshot": null}`; `$D/tidy/Notes`; `--tools` `Read,Grep,Glob,Skill,Edit,Write` (no Bash), `--permission-mode` `auto`; `0` |
| 4 | `$E plugin call acme-tidy run '{"mode":"copy","dir":"r3"}'` (note its `snapshot` as `<snap>`); `cat "$D/run-cwd"`; `ls -A "$ECHOCTL_HOME/.echo/plugins/acme-tidy/<snap>"`; `cat "$ECHOCTL_HOME/.echo/plugins/acme-tidy/<snap>/.claude/skills/tidy/SKILL.md"`; `grep -A1 '^--tools$' "$D/run-args"` | `{"answer": "Tidied", "snapshot": "cache/snapshots/<16 hex>"}`; the run's folder is that snapshot (a path ending `/.echo/plugins/acme-tidy/<snap>`), not `$D/tidy`; it holds `.claude`, `CLAUDE.md` and `Notes`; the skill as written; `--tools` `Read,Grep,Glob,Skill,Edit,Write` |
| 5 | Switch "Claude acts on its own" on: `curl -s -c "$D/jar" {{base_url}}/ > /dev/null`, then `curl -s -b "$D/jar" -H "Origin: {{base_url}}" -X POST --data '{"cmd":"settings_set","key":"folder_skip_permissions","value":true}' {{base_url}}/ipc`; then for each of `edit` (`r4`) and `copy` (`r5`): `$E plugin call acme-tidy run '{"mode":"<mode>","dir":"<r>"}'`; `grep -c -E '^--dangerously-skip-permissions$' "$D/run-args"`; `grep -c '^--permission-mode$' "$D/run-args"` | Each answers `"answer": "Tidied"`; `1` and `0` for both: the user's switch, and only it, lets edits go without asking |
| 6 | With the setting still on: `$E plugin call acme-tidy run '{"mode":"read","dir":"r6"}'`; `grep -A1 '^--permission-mode$' "$D/run-args"`; `grep -c -E '^--dangerously-skip-permissions$' "$D/run-args"` | `--permission-mode` `auto`; `0`: a read run stays as it is |
| 7 | `rm "$D/run-args"`; `$E plugin call acme-tidy run '{"mode":"bypassPermissions","dir":"r7"}'`; `$E plugin call acme-tidy run '{"mode":"read","path":"../","dir":"r8"}'`; `$E plugin call acme-tidy run '{"mode":"read","path":"Notes/Acme.md","dir":"r9"}'`; `ls "$D/run-args"` | `{"error": "mode “bypassPermissions”: read, edit or copy"}`; an `error` containing `isn't a path inside`; one containing `isn't a folder`; `ls` fails (No such file or directory): nothing started |

## Expected Final State
Acme tidy installed with `$D/tidy` given to it; `$D/tidy` as written (the copy run worked on its snapshot); the setting is on.

## Teardown
- The setting off again (step 5's `curl` with `"value":false`); `$E plugin remove acme-tidy --delete-data`; `$E plugin marketplace remove acme-tidy`; `$E ai remove acme-work`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; `rm -r "$D/tidy" "$M"`

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `ls`, `printf`, `mkdir`, `rm`, `mv` and `curl` (the served window's own IPC, as the page sends it).
- The fake stands for Claude Code: what a real one loads from `--setting-sources user,project` (the folder's CLAUDE.md, skills and hooks) and how `auto` treats an edit are Claude Code's own; this case checks how Echo starts it. The refusals before a run (no `folders.write` for `edit`, a folder not given, a folder whose `.claude/settings.json` could route the run, while Claude prepares and asks) are the Rust tests `jshost::folders::a_claude_run_goes_into_a_folder_given_to_the_plugin_edits_only_with_write_and_copy_answers_its_snapshot` and `claudejob::a_plugins_folder_run_is_that_folders_own_project_with_its_modes_tools_and_skips_prompts_only_by_the_users_switch`.
