---
id: TC-090
title: A plugin's Claude run in a folder the user gave it starts there as that folder's own project with Echo's session — read (reading and the web), edit in a subfolder and copy (a snapshot it answers) with every tool
priority: high
type: functional
requirements: [F-PLUG-63]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders]
---

# TC-090: A plugin's Claude run in its folder: the three modes

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data served: `tests/fixtures/serve.sh up <dir> <port>` (in S-12, the scenario's own); CLI steps only, in Bash, each starting with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; E=target/debug/echoctl; S="$ECHOCTL_HOME/.echo/plugins/acme-tidy"` (inside the repository, built right before the run; a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The block of TC-090 to TC-092. A Claude AI profile of Echo's own on the data's fake (never a real account): `$E ai add-claude acme-work --email sam@initech.example`. **A fake `claude` that writes down how Echo starts it and answers every run "Tidied"**, in front of the data's fake: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # --version and auth go to the data's fake; a run writes its arguments one a line and its folder, then answers.
  d="$(dirname "$0")/.."
  case "$1" in --version|auth) exec "$d/bin/claude.fake" "$@";; esac
  printf '%s\n' "$@" > "$d/run-args"; pwd > "$d/run-cwd"
  echo '{"type":"result","subtype":"success","result":"Tidied","usage":{}}'
  ```
  A made-up folder with its own Claude Code setup, and the made-up plugin **Acme tidy** (Claude runs that may skip checks, a folder it may change) in a local marketplace `acme-tidy`:

```bash
mkdir -p "$D/tidy/Notes" "$D/tidy/.claude/skills/tidy"
printf '# Acme notes\nKeep every note under 20 lines.\n' > "$D/tidy/CLAUDE.md"
printf -- '---\nname: tidy\ndescription: Tidy a note.\n---\n\nShorten it.\n' > "$D/tidy/.claude/skills/tidy/SKILL.md"
printf '# Acme\nA long note.\n' > "$D/tidy/Notes/Acme.md"
M="$D/market-tidy"; mkdir -p "$M/plugins/acme-tidy"
echo '{"name":"acme-tidy","plugins":[{"id":"acme-tidy","source":"./plugins/acme-tidy"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-tidy","name":"Acme tidy","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"claude":{"skip":true},"folders":{"what":"notes","write":true}}}' > "$M/plugins/acme-tidy/echo-plugin.json"
cat > "$M/plugins/acme-tidy/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('your notes'))
  ctx.command('run', async (a) => {
    try {
      const r = await ctx.claude.run({ prompt: 'Tidy the notes', profile: a.profile || 'acme-work', tools: a.tools, folder: ctx.fs.folders()[0], path: a.path, mode: a.mode, dir: 'runs/' + a.dir })
      return { answer: r.answer, snapshot: r.snapshot ? r.snapshot.path : null }
    } catch (e) { return { error: String(e.message || e) } }
  })
}
JS
$E plugin marketplace add "$M" --name acme-tidy
```

**Changes:** `<config dir>/ai.json` (the profile), `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), its store's `runs/` and `cache/snapshots/`, `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install acme-tidy --yes`; `ECHOCTL_PICK_FOLDER="$D/tidy" $E plugin call acme-tidy add`; `$E plugin list` and find the installed `acme-tidy` | `Installed Acme tidy 0.1.0`; the folder's path; among its `uses` the Claude line ending `; a run you mark so may skip every permission check. In a folder of its own data, Claude works on the files it gathered with every tool. In a folder you gave it, Claude works with that folder's own Claude Code setup and Echo's: it only reads the folder (and Echo's data), or, if the plugin may change it, edits files there or in a copy for you to review with every tool — as you set routines; on your click in its screens it opens Claude Code there in Echo's Claude panel for you to ask, as you set Claude in the window` |
| 2 | `$E plugin call acme-tidy run '{"mode":"read","dir":"r1"}'`; `cat "$D/run-cwd"`; `grep -E -A1 '^--(tools\|permission-mode\|setting-sources\|add-dir)$' "$D/run-args"`; `tail -2 "$D/run-args"`; `grep -c -E '^--(settings\|allowedTools)$' "$D/run-args"`; `grep -c -E 'echoctl (compose\|project work):' "$D/run-args"`; `grep -c '^--strict-mcp-config$' "$D/run-args"`; `cat "$S/runs/r1/permissions.json"`; `grep -c "Echo's data folder is" "$S/runs/r1/prompt.md"` | `{"answer": "Tidied", "snapshot": null}`; the folder `$D/tidy` (its real path); `--tools` `Bash,Read,Grep,Glob,Skill,WebFetch,WebSearch` and `--permission-mode` `dontAsk` (it reads: only reading, Echo's reading commands and the web run, nothing asked), `--add-dir` the data folder (`$ECHOCTL_HOME`'s real path), `--setting-sources` `user,project` once (the folder's own CLAUDE.md, skills and settings load), the run's last two arguments; more than `0` (Echo's session: its settings file and Echo's reading commands); `0` (no edit of Echo's data, no draft); `0` (the user's MCP servers kept); its record's `"asked"` is `"read"`; `1` |
| 3 | `$E plugin call acme-tidy run '{"mode":"edit","path":"Notes","dir":"r2"}'`; `cat "$D/run-cwd"`; `grep -c '^--tools$' "$D/run-args"`; `cat "$S/runs/r2/permissions.json"`; `grep -c "Echo's CLI is" "$S/runs/r2/prompt.md"` | `{"answer": "Tidied", "snapshot": null}`; `$D/tidy/Notes`; `0` (every tool of Claude Code's, Bash and the web too); `"asked"` is `"edit"`, `"used"` starts `its folder's own Claude Code setup, edits in the folder`; `1` (Echo's CLI named) |
| 4 | `$E plugin call acme-tidy run '{"mode":"copy","dir":"r3"}'` (note its `snapshot` as `<snap>`); `cat "$D/run-cwd"`; `ls -A "$S/<snap>"`; `cat "$S/<snap>/.claude/skills/tidy/SKILL.md"`; `grep -c '^--tools$' "$D/run-args"`; `cat "$D/tidy/Notes/Acme.md"` | `{"answer": "Tidied", "snapshot": "cache/snapshots/<16 hex>"}`; the run's folder is that snapshot (a path ending `/.echo/plugins/acme-tidy/<snap>`), not `$D/tidy`; it holds `.claude`, `CLAUDE.md` and `Notes`; the skill as written; `0` (every tool); the note as written (the folder itself untouched) |

## Expected Final State
Acme tidy installed with `$D/tidy` given to it; `$D/tidy` as written; one snapshot in its store.

## Teardown
- `$E plugin remove acme-tidy --delete-data`; `$E plugin marketplace remove acme-tidy`; `$E ai remove acme-work`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; `rm -r "$D/tidy" "$D/market-tidy" "$D/run-args" "$D/run-cwd"`

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `tail`, `ls`, `printf`, `mkdir`, `rm` and `mv`.
- The fake stands for Claude Code: what a real one loads from `--setting-sources user,project` (the folder's CLAUDE.md, skills and hooks; with `path`, the subfolder's `.claude` and the CLAUDE.md files above it) is Claude Code's own; this case checks how Echo starts it. Its permissions: `plugins/TC-091`; what is refused before a run: `plugins/TC-092`.
