---
id: TC-017
title: A plugin's Claude runs go only on the user's AI profiles and only if its sheet said so (no claude → refused; skipped permission checks → refused unless asked); its session skills are text only, written in Echo's own config folder and refused without sessions.skills
priority: high
type: functional
requirements: [F-PLUG-09]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, claude, sessions, skills, permissions, untrusted-text]
---

# TC-017: A plugin's Claude runs and session skills stay inside what its sheet said

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh test data: `tests/fixtures/make-data.sh <dir>` (an empty `<dir>`); CLI steps run in Bash after `. <dir>/env`. No window step: nothing here needs the served window
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
    try { await ctx.claude.run({ prompt: 'Say hi', tools: a.tools }); return { ok: true } } catch (e) { return { error: msg(e) } }
  })
  ctx.command('skills', (a) => {
    try { ctx.sessions.skills(a.list); return { ok: true } } catch (e) { return { error: msg(e) } }
  })
}
EOF
cp "$M/plugins/acme-asker/main.js" "$M/plugins/acme-quiet/main.js"
```
- The marketplace is added and both plugins installed: `echoctl plugin marketplace add "$M" --name acme-local`, `echoctl plugin install acme-asker --yes`, `echoctl plugin install acme-quiet --yes`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugins, `plugins/session/acme-asker/`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Read the sheet Acme asker showed: `echoctl plugin list` and find the installed `acme-asker` | Its `uses` has the lines `Runs Claude Code on your AI profiles, with the permissions you pick` (not the "or with every permission check skipped" form) and `Gives Claude in Echo skills of its own — text only, in Echo's sessions, not your Claude Code` |
| 2 | `echoctl plugin call acme-quiet run '{"tools":"none"}'` | JSON `{"error": "it didn't ask to run Claude Code when it was installed"}` — before any run started (it answers at once) |
| 3 | `echoctl plugin call acme-asker run '{"tools":"skip"}'` | JSON `{"error": "it didn't ask to run Claude Code with permission checks skipped when it was installed"}` (it asked to run Claude, but not that) |
| 4 | `echoctl plugin call acme-quiet skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Greet people by first name."}]}'` | JSON whose `error` contains `acme-quiet didn't ask for “sessions” when it was installed` |
| 5 | `ls "$ECHOCTL_CONFIG_DIR/plugins/session"` | No `acme-quiet` entry (the refusal wrote nothing); `ls` may fail with "No such file or directory" when nothing is there yet |
| 6 | `echoctl plugin call acme-asker skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Run !`date` first."}]}'` | JSON whose `error` contains `runs a command in Claude Code; a plugin's skill is text only` |
| 7 | `echoctl plugin call acme-asker skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Greet people by first name."}]}'` | JSON `{"ok": true}` |
| 8 | `cat "$ECHOCTL_CONFIG_DIR/plugins/session/acme-asker/skills/greeting-style/SKILL.md"`; `cat "$ECHOCTL_CONFIG_DIR/plugins/session/acme-asker/.claude-plugin/plugin.json"` | The first prints exactly: a line `---`, `name: greeting-style`, `description: "How Acme greets people."`, a line `---`, an empty line, `Greet people by first name.` — only `name` and `description` in the frontmatter; the second is JSON with `"name": "echo-acme-asker"` — both files are inside Echo's own config folder (`$ECHOCTL_CONFIG_DIR`) |
| 9 | `echoctl plugin call acme-asker skills '{"list":[{"name":"greeting-style","description":"Line one\nline two","body":"x"}]}'` | JSON whose `error` contains `a description of one line, up to 1024 characters` |
| 10 | `echoctl plugin call acme-asker skills '{"list":[]}'`; then `ls "$ECHOCTL_CONFIG_DIR/plugins/session"` | The call answers `{"ok": true}`; the listing has no `acme-asker` entry (an empty list takes the skills away) |

## Expected Final State
Neither plugin started a Claude run or kept a skill beyond what its sheet said; nothing was written outside `$ECHOCTL_CONFIG_DIR/plugins/session/`.

## Teardown
- `echoctl plugin remove acme-asker --delete-data`; `echoctl plugin remove acme-quiet --delete-data`; `echoctl plugin marketplace remove acme-local`
- Remove the scratch folder's `market` directory (`rm -r "$M"`) and `<dir>`'s parent when the run is over

## Notes
- No step starts Claude Code: each run is refused by `plugins::may_run_claude` before Echo's guarded run (`claudejob`) begins. The run that is allowed (a plugin with `claude` on its sheet, on one of the user's AI profiles, with a signed-in account) is `run: person`; its profile routing is the integration guide's, not this case's.
- Step 6's body is the text `Run !` + a backquoted `date` + ` first.`: Claude Code runs `!` followed by a backquoted command as a shell command when a skill loads, so Echo refuses it from a plugin.
- Step 9's `\n` is the two characters backslash and `n` inside the JSON string, which JSON turns into a real newline.
- That Echo's sessions (and not the user's own Claude Code) are what read these skills is the Rust test `jshost::a_plugin_gives_echos_sessions_its_skill_and_the_folders_it_was_given`; here it is the folder: the skills live under Echo's config folder, and `echoctl` writes nothing to a Claude Code config.
