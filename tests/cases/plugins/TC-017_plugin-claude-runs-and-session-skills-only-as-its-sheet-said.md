---
id: TC-017
title: A plugin's Claude runs go only on the user's AI profiles and only if its sheet said so (no claude → refused; skipped permission checks → refused unless asked); its session skills are text only, written in Echo's own config folder and refused without sessions.skills
priority: high
type: functional
requirements: [F-PLUG-09]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, claude, sessions, skills, permissions, untrusted-text]
---

# TC-017: A plugin's Claude runs and session skills stay inside what its sheet said

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data (it has the AI profiles Claude · work, Claude · other, Claude · out and a fake Claude Code): `python3 tests/fixtures/make_data.py <dir> --set standard --ai fake` (an empty `<dir>`); CLI steps run in Bash after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; `echoctl` is `target/debug/echoctl`. No window step: nothing here needs the served window
- `echoctl ai default work` (the default AI profile is Claude · work)
- **A fake `claude` that tells which profile ran it**, in front of the data's fake (never a real account): `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # Any run (not --version, not auth status) writes the folder of the AI profile it runs on to $D/ran-on, then the data's fake answers.
  d="$(dirname "$0")/.."
  case "$1 $2" in "auth status") ;; *) case "$1" in --version) ;; *) echo "$CLAUDE_CONFIG_DIR" >> "$d/ran-on";; esac;; esac
  exec "$d/bin/claude.fake" "$@"
  ```
- Two made-up plugins in a local marketplace `acme-local`: **Acme asker** asks for Claude runs (without skipped permission checks) and for session skills; **Acme quiet** asks for neither. Both share one host half. The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-asker" "$M/plugins/acme-quiet"
echo '{"name":"acme-local","plugins":[{"id":"acme-asker","source":"./plugins/acme-asker"},{"id":"acme-quiet","source":"./plugins/acme-quiet"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-asker","name":"Acme asker","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"claude":{},"sessions":{"skills":true}}}' > "$M/plugins/acme-asker/echo-plugin.json"
echo '{"id":"acme-quiet","name":"Acme quiet","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"network":{"hosts":["api.acme.example"]}}}' > "$M/plugins/acme-quiet/echo-plugin.json"
cat > "$M/plugins/acme-asker/main.js" <<'EOF'
export function apply(ctx) {
  const msg = (e) => String((e && e.message) || e)
  ctx.command('run', async (a) => {
    try { await ctx.claude.run({ prompt: 'Say hi', tools: a.tools, profile: a.profile }); return { ok: true } } catch (e) { return { error: msg(e) } }
  })
  ctx.command('skills', (a) => {
    try { ctx.sessions.skills(a.list); return { ok: true } } catch (e) { return { error: msg(e) } }
  })
}
EOF
cp "$M/plugins/acme-asker/main.js" "$M/plugins/acme-quiet/main.js"
```
- The marketplace is added and both plugins installed: `echoctl plugin marketplace add "$M" --name acme-local`, `echoctl plugin install acme-asker --yes`, `echoctl plugin install acme-quiet --yes`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugins, `plugins/session/acme-asker/`), `<config dir>/ai.json` (the default profile), `$D/ran-on`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Read the sheet Acme asker showed: `echoctl plugin list` and find the installed `acme-asker` | Its `uses` has the lines `Runs Claude Code on your AI profiles, with the permissions you pick` (not the "or with every permission check skipped" form) and `Gives Claude in Echo skills of its own — text only, in Echo's sessions, not your Claude Code` |
| 2 | `echoctl plugin call acme-quiet run '{"tools":"none"}'` | JSON `{"error": "it didn't ask to run Claude Code when it was installed"}` — before any run started (it answers at once) |
| 3 | `echoctl plugin call acme-asker run '{"tools":"skip"}'; ls "$D/ran-on"` | JSON `{"error": "it didn't ask to run Claude Code with permission checks skipped when it was installed"}` (it asked to run Claude, but not that); `ls` finds no `$D/ran-on`: neither refused run reached Claude Code (steps 2 and 3) |
| 4 | `echoctl plugin call acme-asker run '{"tools":"none"}'; cat "$D/ran-on"` (a run its sheet allows) | The run is let through to Claude Code, on the data's fake: the answer is `{"error": …}` whose text does not contain `didn't ask` (the fake refuses the run itself: `test data: Claude Code is a fake here`); `$D/ran-on` holds one line, the folder of the profile **Claude · work**, the default: a path ending `/claude/work` |
| 5 | `echoctl plugin call acme-quiet skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Greet people by first name."}]}'` | JSON whose `error` contains `acme-quiet didn't ask for “sessions” when it was installed` |
| 6 | `ls "$ECHOCTL_CONFIG_DIR/plugins/session"` | No `acme-quiet` entry (the refusal wrote nothing); `ls` may fail with "No such file or directory" when nothing is there yet |
| 7 | `echoctl plugin call acme-asker skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Run !`date` first."}]}'` | JSON whose `error` contains `runs a command in Claude Code; a plugin's skill is text only` |
| 8 | `echoctl plugin call acme-asker skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Greet people by first name."}]}'` | JSON `{"ok": true}` |
| 9 | `cat "$ECHOCTL_CONFIG_DIR/plugins/session/acme-asker/skills/greeting-style/SKILL.md"`; `cat "$ECHOCTL_CONFIG_DIR/plugins/session/acme-asker/.claude-plugin/plugin.json"` | The first prints exactly: a line `---`, `name: greeting-style`, `description: "How Acme greets people."`, a line `---`, an empty line, `Greet people by first name.` — only `name` and `description` in the frontmatter; the second is JSON with `"name": "echo-acme-asker"` — both files are inside Echo's own config folder (`$ECHOCTL_CONFIG_DIR`) |
| 10 | `echoctl plugin call acme-asker skills '{"list":[{"name":"greeting-style","description":"Line one\nline two","body":"x"}]}'` | JSON whose `error` contains `a description of one line, up to 1024 characters` |
| 11 | `echoctl plugin call acme-asker skills '{"list":[]}'`; then `ls "$ECHOCTL_CONFIG_DIR/plugins/session"` | The call answers `{"ok": true}`; the listing has no `acme-asker` entry (an empty list takes the skills away) |

## Expected Final State
The run the sheet allowed went to the user's AI profile (the default one, Claude · work) and the runs it did not allow started nothing; neither plugin kept a skill beyond what its sheet said; nothing was written outside `$ECHOCTL_CONFIG_DIR/plugins/session/`.

## Teardown
- `echoctl plugin remove acme-asker --delete-data`; `echoctl plugin remove acme-quiet --delete-data`; `echoctl plugin marketplace remove acme-local`
- `mv "$D/bin/claude.fake" "$D/bin/claude"`; remove the scratch folder's `market` directory (`rm -r "$M"`) and `<dir>`'s parent when the run is over

## Notes
- The refused runs (steps 2 and 3) start nothing: `$D/ran-on` does not exist after them. The allowed run (step 4) ends in the data's fake Claude Code refusing it; what is checked is where Echo sent it. A signed-in real account, and what the run writes, are `run: person` and the integration guide's, not this case's.
- Step 7's body is the text `Run !` + a backquoted `date` + ` first.`: Claude Code runs `!` followed by a backquoted command as a shell command when a skill loads, so Echo refuses it from a plugin.
- Step 10's `\n` is the two characters backslash and `n` inside the JSON string, which JSON turns into a real newline.
- That Echo's sessions (and not the user's own Claude Code) are what read these skills is the Rust test `jshost::a_plugin_gives_echos_sessions_its_skill_and_the_folders_it_was_given`; here it is the folder: the skills live under Echo's config folder, and `echoctl` writes nothing to a Claude Code config.
