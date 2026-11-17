---
id: TC-089
title: A vault's repository whose settings include a file a plugin could write — in its vault, there or not — is refused, and a filter the plugin writes into that file never runs; with the include gone, it works again
priority: critical
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git, security]
---

# TC-089: A repository whose settings include a file in the vault is refused

**Module:** Plugins | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- git 2.31 or newer where the system installs it (`/usr/bin/git` with developer tools, or Homebrew's)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault and a made-up plugin **Acme history** (a folder it may change, a host half) in a local marketplace `acme-git` — the block of TC-071 to TC-074:

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

- A script that leaves a mark were anything to run it: `printf '#!/bin/sh\ntouch "%s/ran"\ncat\n' "$D" > "$D/mark.sh"; chmod 755 "$D/mark.sh"`

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/history/` (a repository, `tools/`), the plugin's store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-history --yes`; `ECHOCTL_PICK_FOLDER="$D/history" echoctl plugin call acme-history add`; `echoctl plugin call acme-history init`; `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot planned.\n"}'`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: first notes"}'` | `Installed Acme history 0.1.0`; the vault's path; its `info`; `null`; `{"rev": "<40 hex>"}` |
| 2 | The user's repository settings include a file of the vault that isn't there yet: `printf '[include]\n\tpath = ../tools/git.cfg\n' >> "$D/history/.git/config"`; `echoctl plugin call acme-history info`; `echoctl plugin call acme-history status` | Both fail: `this repository's settings read <the vault's real path>/tools/git.cfg, a file a plugin could change: Echo doesn't use it` (macOS: the path starts `/private/var/…`) |
| 3 | The plugin writes that file with a filter for its notes: `echoctl plugin call acme-history put "{\"path\":\"tools/attrs\",\"text\":\"*.md filter=x\\n\"}"`; `echoctl plugin call acme-history put "{\"path\":\"tools/git.cfg\",\"text\":\"[filter \\\"x\\\"]\\n\\tclean = $D/mark.sh\\n[core]\\n\\tattributesFile = $D/history/tools/attrs\\n\"}"`; `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot signed off.\n"}'`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: signed off"}'` | `null` three times; the commit fails with step 2's message |
| 4 | `ls "$D/ran"` | `No such file or directory`: the filter never ran |
| 5 | The user takes the include out: `git -C "$D/history" config --unset include.path`; `echoctl plugin call acme-history log '{"max":1}'` | One commit, the vault's latest (`Acme: first notes` on its own; never `Acme: signed off` of step 3): the repository works again |

## Expected Final State
`$D/history` is a repository Echo uses, its settings including nothing; `tools/` holds the plugin's two files; no `$D/ran`.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`
