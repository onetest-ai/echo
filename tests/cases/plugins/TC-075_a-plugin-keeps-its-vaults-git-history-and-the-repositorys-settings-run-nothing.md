---
id: TC-075
title: A plugin starts a repository in its vault, commits exactly the notes it names, reads their history and an old version — and nothing the repository's own settings, hooks or attributes name runs; commits are refused while Claude prepares and asks
priority: high
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git]
---

# TC-075: A plugin keeps its vault's git history

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- git 2.31 or newer on this computer (`/usr/bin/git` or Homebrew's)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault, a script that leaves a mark were anything to run it, and a made-up plugin **Acme history** (a folder it may change, a host half) in a local marketplace `acme-git`:

```bash
mkdir -p "$D/history"
printf '#!/bin/sh\ntouch "%s/ran"\ncat\n' "$D" > "$D/mark.sh"; chmod 755 "$D/mark.sh"
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
| 5 | The repository's own settings name the mark script for hooks, the fsmonitor and a filter: `printf '[core]\n\tfsmonitor = %s\n\thooksPath = %s\n[filter "x"]\n\tclean = %s\n\trequired = true\n[commit]\n\tgpgSign = true\n' "$D/mark.sh" "$D/hooks" "$D/mark.sh" >> "$D/history/.git/config"; mkdir -p "$D/hooks"; for h in pre-commit post-commit reference-transaction; do cp "$D/mark.sh" "$D/hooks/$h"; done; mkdir -p "$D/history/.git/info"; printf '*.md filter=x\n' > "$D/history/.git/info/attributes"`; then `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot budget agreed.\n"}'`; `echoctl plugin call acme-history status`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: budget"}'` | `[{"path": "Notes/Acme.md", "code": " M"}, {"path": "Notes/Globex.md", "code": "??"}]`; `{"rev": "<40 hex>"}` |
| 6 | `ls "$D/ran"` | `No such file or directory`: no hook, fsmonitor, filter or signing program ran |
| 7 | `echoctl plugin call acme-history log '{"path":"Notes/Acme.md"}'` | Two commits, newest first: subjects `Acme: budget`, `Acme: first notes`, each with `"echo": "acme-history"`, `"path": "Notes/Acme.md"`, an ISO `date` |
| 8 | `echoctl plugin call acme-history show '{"rev":"HEAD~1","path":"Notes/Acme.md"}'`; `echoctl plugin call acme-history show '{"rev":"HEAD","path":"Notes/None.md"}'`; `echoctl plugin call acme-history show '{"rev":"--output=x","path":"Notes/Acme.md"}'` | `{"text": "# Acme\nPilot planned.\n", "size": 22, "truncated": false}` — the text as committed, no filter; `null`; fails: `“--output=x” isn't a version: HEAD, HEAD~N or a commit's id` |
| 9 | `echoctl plugin call acme-history put '{"path":"a*.md","text":"star\n"}'`; `echoctl plugin call acme-history put '{"path":"ab.md","text":"plain\n"}'`; `echoctl plugin call acme-history commit '{"paths":["a*.md"],"message":"a star"}'`; `echoctl plugin call acme-history status` | The commit's `{"rev": …}`; the status still lists `ab.md` (`??`) and `Notes/Globex.md`: `a*.md` was that one file, not a pattern |
| 10 | `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-history commit '{"paths":["ab.md"],"message":"x"}'`; `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-history log '{"max":1}'` | Fails: `changing files or settings isn't allowed while Claude prepares and asks`; the log answers (one commit, `a star`): reads work while Claude prepares and asks |
| 11 | `ls "$D/ran"` | Still `No such file or directory` |

## Expected Final State
`$D/history` is a repository with three commits by `Echo (acme-history)`; `ab.md` and `Notes/Globex.md` uncommitted; no `$D/ran`.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`
