---
id: TC-071
title: A plugin starts a repository in its vault (once) and commits exactly the notes it names — not the others, a `*` in a name as that one file, never a file the repository ignores
priority: high
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git]
---

# TC-071: A plugin starts a repository and commits only the notes it names

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- git 2.31 or newer where the system installs it (`/usr/bin/git` with developer tools, or Homebrew's)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault and a made-up plugin **Acme history** (a folder it may change, a host half) in a local marketplace `acme-git` — the same block in TC-071 to TC-074:

```bash
mkdir -p "$D/history"
M="$D/market-git"; mkdir -p "$M/plugins/acme-history"
echo '{"name":"acme-git","plugins":[{"id":"acme-history","source":"./plugins/acme-history"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-history","name":"Acme history","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"folders":{"what":"vaults","write":true}}}' > "$M/plugins/acme-history/echo-plugin.json"
cat > "$M/plugins/acme-history/main.js" <<'JS'
export function apply(ctx) {
  const r = () => ctx.fs.folders()[0]
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('put', (a) => ctx.fs.write(r(), a.path, a.text))
  ctx.command('info', async () => await ctx.git.info(r()))
  ctx.command('init', async () => await ctx.git.init(r()))
  ctx.command('status', async () => await ctx.git.status(r()))
  ctx.command('commit', async (a) => await ctx.git.commit(r(), a))
  ctx.command('log', async (a) => await ctx.git.log(r(), a))
  ctx.command('show', async (a) => await ctx.git.show(r(), a.rev, a.path))
}
JS
echoctl plugin marketplace add "$M" --name acme-git
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/history/` (a repository), the plugin's store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-history --yes`; `ECHOCTL_PICK_FOLDER="$D/history" echoctl plugin call acme-history add`; `echoctl plugin call acme-history info` | `Installed Acme history 0.1.0`; the vault's path; `{"repo": false}` |
| 2 | `echoctl plugin call acme-history init`; `echoctl plugin call acme-history init` | `{"repo": true, "branch": "main", "head": null, "detached": false, "busy": null, "subfolder": false}`; the second fails: `this folder is in a git repository already` |
| 3 | `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot planned.\n"}'`; `echoctl plugin call acme-history put '{"path":"Notes/Globex.md","text":"# Globex\n"}'`; `echoctl plugin call acme-history status` | `null` twice; `[{"path": "Notes/Acme.md", "code": "??"}, {"path": "Notes/Globex.md", "code": "??"}]` |
| 4 | `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: first notes"}'`; `echoctl plugin call acme-history status` | `{"rev": "<40 hex>"}`; only `[{"path": "Notes/Globex.md", "code": "??"}]` — the note it didn't name is left as it was |
| 5 | `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"again"}'` | `{"rev": null}`: nothing changed in that note |
| 6 | `echoctl plugin call acme-history put '{"path":"a*.md","text":"star\n"}'`; `echoctl plugin call acme-history put '{"path":"ab.md","text":"plain\n"}'`; `echoctl plugin call acme-history commit '{"paths":["a*.md"],"message":"a star"}'`; `echoctl plugin call acme-history status` | The commit's `{"rev": "<40 hex>"}`; the status still lists `ab.md` (`??`) and `Notes/Globex.md` (`??`): `a*.md` was that one file, not a pattern |
| 7 | `printf 'scratch.md\n' > "$D/history/.gitignore"`; `echoctl plugin call acme-history put '{"path":"scratch.md","text":"draft\n"}'`; `echoctl plugin call acme-history commit '{"paths":["scratch.md"],"message":"x"}'` | `null`; the commit fails: `“scratch.md” is ignored by this repository (.gitignore): Echo doesn't commit it` (not `{"rev": null}`) |
| 8 | `git -C "$D/history" log --format='%an <%ae> / %cn <%ce>'` | Two lines, each `Sam Rivera <sam@initech.example> / Echo (acme-history) <acme-history@plugins.echo.invalid>`: the author is the test data's made-up user (its `GIT_CONFIG_GLOBAL`), never this computer's |

## Expected Final State
`$D/history` is a repository with two commits by `Echo (acme-history)`, authored by `Sam Rivera` (`Acme: first notes`, `a star`); `ab.md`, `Notes/Globex.md`, `.gitignore` and `scratch.md` uncommitted.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`
