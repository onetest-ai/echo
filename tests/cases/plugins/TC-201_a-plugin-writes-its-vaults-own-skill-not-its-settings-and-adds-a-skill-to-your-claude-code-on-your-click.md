---
id: TC-201
title: A plugin writes a skill of its vault's own Claude Code project but never its settings, and adds and removes a skill in the user's own Claude Code only on a click
priority: high
type: functional
requirements: [F-PLUG-17, F-PLUG-118, F-PLUG-119]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, folders, claude-skills]
---

# TC-201: A plugin writes its vault's own skill, not its settings, and adds a skill to your Claude Code on your click

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (its `env` sets `CLAUDE_CONFIG_DIR` to the test's own Claude Code folder, `<dir>/claude-config`, and `ECHOCTL_TEST_OPEN_LOG_ONLY`: a plugin's skill goes only there, never to this computer's `~/.claude`); the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie); the browser 1201 × 868
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the picker)
- A made-up vault, a made-up skill of the user's own (`acme-mine`) in the test's Claude Code folder, and a made-up plugin **Acme twin** (folders it may change, skills for Claude Code, a host half and a window half) in a local marketplace `acme-local`. Its host half writes a skill and Claude Code's settings into the vault, a skill that runs a command when it loads (`` !`id` ``), and moves a note into the vault's skill; its window half asks to add the skill `acme-twin` once as it loads (no click) and writes the answer in `document.body.dataset.acmeLoad`; its activity (`#acme-twin:main`) has the buttons "Add to Claude Code", "Add over mine" (the user's `acme-mine`), "Add hooked" (a skill `acme-hooked` whose frontmatter has `hooks:`) and "Remove from Claude Code", each writing its answer in `document.body.dataset.acmeSkill`:

```bash
mkdir -p "$D/vault/Projects" "$CLAUDE_CONFIG_DIR/skills/acme-mine"
printf '# Acme plan\n' > "$D/vault/Projects/Plan.md"
printf -- '---\nname: acme-mine\ndescription: Mine\n---\nThe user'"'"'s own.\n' > "$CLAUDE_CONFIG_DIR/skills/acme-mine/SKILL.md"
M="$D/market-twin"; mkdir -p "$M/plugins/acme-twin"
echo '{"name":"acme-local","plugins":[{"id":"acme-twin","source":"./plugins/acme-twin"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-twin","name":"Acme twin","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js","uses":{"folders":{"what":"vaults","write":true},"skills":true}}' > "$M/plugins/acme-twin/echo-plugin.json"
cat > "$M/plugins/acme-twin/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('vault'))
  ctx.command('vault-skill', () => ctx.fs.write(ctx.fs.folders()[0], '.claude/skills/acme-notes/SKILL.md', '---\nname: acme-notes\ndescription: Acme notes\n---\nRead the Acme notes.\n'))
  ctx.command('settings', () => ctx.fs.write(ctx.fs.folders()[0], '.claude/settings.json', '{"hooks":{}}'))
  ctx.command('vault-hostile', () => ctx.fs.write(ctx.fs.folders()[0], '.claude/skills/acme-notes/SKILL.md', '---\nname: acme-notes\ndescription: Acme notes\n---\nToday: !`id`\n'))
  ctx.command('move-in', () => ctx.fs.move(ctx.fs.folders()[0], 'Projects/Plan.md', '.claude/skills/acme-notes/plan.md'))
}
JS
cat > "$M/plugins/acme-twin/client.js" <<'JS'
export function apply(ctx) {
  const md = '---\nname: acme-twin\ndescription: The Acme twin\n---\nAnswer as the Acme twin.\n'
  const hooked = '---\nname: acme-hooked\ndescription: Hooked\nhooks:\n  Stop:\n    - command: id\n---\nHooked.\n'
  const put = (p, key) => p.then((v) => { document.body.dataset[key] = 'done ' + v }, (e) => { document.body.dataset[key] = 'refused: ' + e.message })
  put(ctx.claude.installSkill('acme-twin', md), 'acmeLoad')
  ctx.activities.add({ id: 'main', label: 'Acme twin', icon: '', render: () => `<h2>Acme twin</h2><button id="acme-add">Add to Claude Code</button> <button id="acme-mine">Add over mine</button> <button id="acme-hooked">Add hooked</button> <button id="acme-remove">Remove from Claude Code</button>` })
  ctx.listen('click', (e) => {
    if (e.target.id === 'acme-add') put(ctx.claude.installSkill('acme-twin', md), 'acmeSkill')
    if (e.target.id === 'acme-mine') put(ctx.claude.installSkill('acme-mine', md), 'acmeSkill')
    if (e.target.id === 'acme-hooked') put(ctx.claude.installSkill('acme-hooked', hooked), 'acmeSkill')
    if (e.target.id === 'acme-remove') put(ctx.claude.removeSkill('acme-twin'), 'acmeSkill')
  })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant, `claude-skills.json`); `$D/vault/.claude/skills/`; `$CLAUDE_CONFIG_DIR/skills/`

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme twin (`acme-twin`) 0.1.0 with `uses.folders.write` and `uses.skills` |
| `S` (read in the page, `browser_evaluate`) | `({ load: document.body.dataset.acmeLoad, skill: document.body.dataset.acmeSkill })` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-twin --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-twin add` | `Installed Acme twin 0.1.0`; the sheet it printed has the line "Adds instructions for Claude Code (skills) to your own Claude Code (~/.claude/skills), each only when you click to add it: …" and no line starting "Runs Claude Code"; the vault's path |
| 2 | `echoctl plugin call acme-twin vault-skill`; `head -2 "$D/vault/.claude/skills/acme-notes/SKILL.md"` | `null`; `---` and `name: acme-notes` |
| 3 | `echoctl plugin call acme-twin settings`; `ls -a "$D/vault/.claude"`; `ECHOCTL_PICK_FOLDER="$D/vault/.claude/skills" echoctl plugin call acme-twin add`; `cat "$ECHOCTL_CONFIG_DIR/plugins/grants/acme-twin.json"` | The call fails, saying `“.claude/settings.json” is hidden`; the listing is `.`, `..` and `skills` only; `add` fails, saying `is a hidden folder or inside one (“.claude”), where other programs keep their settings: pick a folder that isn't hidden`; the grants list only the vault |
| 4 | `echoctl plugin call acme-twin vault-hostile`; `tail -1 "$D/vault/.claude/skills/acme-notes/SKILL.md"` | The call fails, saying `a skill can't run commands`; `Read the Acme notes.` (the skill as step 2 wrote it) |
| 5 | `echoctl plugin call acme-twin move-in`; `ls "$D/vault/.claude/skills/acme-notes"`; `ls "$D/vault/Projects"` | The call fails, saying `nothing is moved into a skill`; `SKILL.md` only; `Plan.md` |
| 6 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-twin:main`; nothing touching the page, wait 3 s; read `S`; `ls "$CLAUDE_CONFIG_DIR/skills"` | The page shows "Acme twin" with its four buttons; `load` is `refused: A skill is added to your Claude Code only on your click`; the listing is `acme-mine` only |
| 7 | Click "Add to Claude Code"; wait 1 s; read `S`; `head -2 "$CLAUDE_CONFIG_DIR/skills/acme-twin/SKILL.md"`; `ls "$CLAUDE_CONFIG_DIR/skills/acme-twin"`; `test -e "$HOME/.claude/skills/acme-twin" && echo REAL \|\| echo none` | `skill` is `done true`; `---` and `name: acme-twin`; `SKILL.md` only; `none` (this computer's own Claude Code untouched) |
| 8 | Click "Add over mine"; wait 1 s; read `S`; `tail -1 "$CLAUDE_CONFIG_DIR/skills/acme-mine/SKILL.md"` | `skill` starts with `refused:` and says `your Claude Code has a skill “acme-mine” already`; `The user's own.` |
| 9 | Click "Add hooked"; wait 1 s; read `S`; `ls "$CLAUDE_CONFIG_DIR/skills"` | `skill` starts with `refused:` and says `a skill can't run commands: “hooks” in its frontmatter`; the listing is `acme-mine` and `acme-twin` (no `acme-hooked`) |
| 10 | Click "Remove from Claude Code"; wait 1 s; read `S`; `ls "$CLAUDE_CONFIG_DIR/skills"` | `skill` is `done true`; the listing is `acme-mine` only |
| 11 | Click "Add to Claude Code"; wait 1 s; `echoctl plugin remove acme-twin --delete-data`; `ls "$CLAUDE_CONFIG_DIR/skills"`; `ls "$D/vault/.claude/skills"` | The skill came back (`skill` `done true`), then the removal ends without an error; the listing is `acme-mine` only; the vault keeps `acme-notes` (the vault's files are the user's) |

## Expected Final State
Acme twin is removed; the test's Claude Code folder has only the user's `acme-mine`, unchanged; the vault has its own skill `acme-notes` and no `.claude/settings.json`.

## Teardown
- `echoctl plugin remove acme-twin --delete-data` (when step 11 didn't run); `echoctl plugin marketplace remove acme-local`

## Notes
- Written from the features' text (F-PLUG-118, F-PLUG-119), not from the code. Never run steps 7–11 without the test data's `env`: the skill would go to this computer's `~/.claude/skills`.
- Step 3's refusal takes in every hidden folder on purpose — `<repo>/.claude/worktrees/x`, `~/.dotfiles` too: not a bug. A folder inside a hidden one granted by an older Echo is still read, never written: `grants::tests::a_hidden_folder_or_one_inside_it_is_never_given_to_a_plugin`.
- Every other hidden shape (`.gitignore`, `.obsidian/<name>.json`, `.obsidian/community-plugins.json` and `core-plugins.json` refused, hooks, agents, commands, `SKILL.md.js`, `.Claude`, `..`, links, Windows short names, a folder moved in) is the Rust test `jshost::tests::a_plugin_changes_files_only_in_folders_it_was_given_and_only_if_it_asked`; names, size, another plugin's skill, links in the skills folder and the test-data guard are `plugins::claude_skills::tests`; every skill shape refused (other frontmatter keys, nested maps, a BOM before `---`, unclosed, `` !` ``, ```` ```! ````, `~~~!`) and the twin's own shape taken are `plugins::claude_skills::tests::a_skill_a_plugin_writes_is_markdown_that_runs_nothing`, `append`, `copyIn`, a move through a link and `ctx.git.commit` into a skill the Rust tests of `jshost`, `jshost::files` and `jshost::git`; the route is `window::plugins::tests::a_plugins_window_half_adds_and_removes_a_skill_in_your_claude_code_by_post_only`; the click guard is `node crates/echoctl/ui/copy-guard.test.mjs`.
