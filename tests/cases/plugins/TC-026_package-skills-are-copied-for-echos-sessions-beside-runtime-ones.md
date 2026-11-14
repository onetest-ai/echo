---
id: TC-026
title: A plugin's package skills (the manifest's skills folders) are named on the sheet, copied as shipped into Echo's own config folder beside its runtime skills, refused with frontmatter beyond name and description, copied again on update and removed with the plugin; a runtime skill can't take a package skill's name
priority: high
type: functional
requirements: [F-PLUG-24]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, sessions, skills, permissions, untrusted-text]
---

# TC-026: A plugin's package skills reach Echo's sessions as its package ships them

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data: `python3 tests/fixtures/make_data.py <dir> --set standard` (an empty `<dir>`); CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`, built right before the run. No window step: nothing here needs the served window
- A made-up plugin **Acme kit** in a local marketplace `acme-local`: one package skill `staffing` (a `SKILL.md` and a `references/fields.md`) and a host half that sets runtime skills. The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market"; K="$M/plugins/acme-kit"; S="$ECHOCTL_CONFIG_DIR/plugins/session/acme-kit"
mkdir -p "$K/skills/staffing/references"
echo '{"name":"acme-local","plugins":[{"id":"acme-kit","source":"./plugins/acme-kit"}]}' > "$M/echo-marketplace.json"
kit() { echo "{\"id\":\"acme-kit\",\"name\":\"Acme kit\",\"version\":\"$1\",\"summary\":\"A made-up plugin for tests\",\"main\":\"main.js\",\"skills\":[\"skills/staffing\"],\"uses\":{\"sessions\":{\"skills\":true}}}" > "$K/echo-plugin.json"; }
kit 0.1.0
printf -- '---\nname: staffing\ndescription: Reads the staffing of an Acme project.\n---\n\nAsk for the project code first.\n' > "$K/skills/staffing/SKILL.md"
echo '# Fields: person, role, from, to' > "$K/skills/staffing/references/fields.md"
cat > "$K/main.js" <<'EOF'
export function apply(ctx) {
  ctx.command('skills', (a) => {
    try { ctx.sessions.skills(a.list); return { ok: true } } catch (e) { return { error: String((e && e.message) || e) } }
  })
}
EOF
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, `plugins/session/acme-kit/`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-kit --yes`; then `echoctl plugin list` and find the installed `acme-kit` | The install succeeds; its `uses` has the line `Tells Claude in Echo how to work with it (the skill “staffing”), loaded only when you work with it (its files: …)` (an 8-character hash) |
| 2 | `diff "$K/skills/staffing/SKILL.md" "$S/skills/staffing/SKILL.md" && diff "$K/skills/staffing/references/fields.md" "$S/skills/staffing/references/fields.md" && echo same`; `cat "$S/.claude-plugin/plugin.json"` | `same` (the folder is copied as the package ships it, before the plugin's code ever ran); the JSON has `"name": "echo-acme-kit"`; `$S` is inside Echo's own config folder (`$ECHOCTL_CONFIG_DIR`) |
| 3 | `echoctl plugin call acme-kit skills '{"list":[{"name":"staffing","description":"Mine.","body":"Do it my way."}]}'` | JSON whose `error` contains `the plugin's package has a skill of that name`; `cat "$S/skills/staffing/SKILL.md"` still prints the package's text (`Ask for the project code first.`) |
| 4 | `echoctl plugin call acme-kit skills '{"list":[{"name":"greeting-style","description":"How Acme greets people.","body":"Greet people by first name."}]}'`; then `ls "$S/skills"` | JSON `{"ok": true}`; the listing is `greeting-style` and `staffing` (the runtime skill beside the package's) |
| 5 | `echoctl plugin call acme-kit skills '{"list":[]}'`; then `ls "$S/skills"` | JSON `{"ok": true}`; the listing is `staffing` only (an empty list takes away its runtime skills, not its package's) |
| 6 | A version whose skill carries more frontmatter: `kit 0.2.0; printf -- '---\nname: staffing\ndescription: Reads the staffing.\nhooks: x\n---\n\nText.\n' > "$K/skills/staffing/SKILL.md"; echoctl plugin marketplace update acme-local; echoctl plugin update acme-kit --yes; echoctl plugin install acme-kit --yes` | The update installs nothing (`Acme kit 0.1.0 is up to date`: a version Echo can't read isn't offered); the install fails with `“acme-kit” can't be installed: skill “skills/staffing”: SKILL.md's frontmatter holds only name and description (not “hooks”)`; the installed `acme-kit` is still `0.1.0` and `$S/skills/staffing/SKILL.md` unchanged |
| 7 | A good new version: `kit 0.3.0; printf -- '---\nname: staffing\ndescription: Reads the staffing of an Acme project.\n---\n\nAsk for the project code, then the dates.\n' > "$K/skills/staffing/SKILL.md"; echoctl plugin marketplace update acme-local; echoctl plugin update acme-kit --yes`; then `cat "$S/skills/staffing/SKILL.md"` | The update prints the sheet with the skill's line marked new (its files changed: `--yes` agrees); the installed `acme-kit` is `0.3.0`; the copy ends with `Ask for the project code, then the dates.` |
| 8 | `echoctl plugin remove acme-kit --delete-data`; then `ls "$ECHOCTL_CONFIG_DIR/plugins/session"` | No `acme-kit` entry (`ls` may fail with "No such file or directory" when nothing is there) |

## Expected Final State
Acme kit is removed and its session folder is gone; while it was installed, its package skill was in Echo's config folder exactly as the installed version shipped it, beside the runtime skill it set and never replaced by one.

## Teardown
- `echoctl plugin marketplace remove acme-local`; remove the scratch folder's `market` directory (`rm -r "$M"`) and `<dir>`'s parent when the run is over

## Notes
- That a routine loads this folder (`--plugin-dir`; the folder session gets it laid out in the data folder instead, TC-035) and names the skill `echo-acme-kit:staffing` — and that a copy older than the installed files is copied again before a session starts, and that a switched-off plugin gives none — is the Rust test `jshost::a_plugins_package_skill_reaches_the_sessions_beside_its_runtime_skills`.
- Step 6's `hooks:` stands for anything beyond `name` and `description` a Claude Code skill could carry (hooks, allowed tools, a model): Echo refuses the version whole.
