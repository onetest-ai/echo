---
id: TC-074
title: Under the read-only marker of before, a plugin's git commit goes through and its git reads answer: no run prepares and asks any more
priority: high
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git, routines]
---

# TC-074: The read-only marker of before limits a plugin's commit no more

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
| 1 | `echoctl plugin install acme-history --yes`; `ECHOCTL_PICK_FOLDER="$D/history" echoctl plugin call acme-history add`; `echoctl plugin call acme-history init`; `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot planned.\n"}'`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: first notes"}'` | `Installed Acme history 0.1.0`; the vault's path; its `info`; `null`; `{"rev": "<40 hex>"}` |
| 2 | `echoctl plugin call acme-history put '{"path":"Notes/Routine.md","text":"# Routine\n"}'`; `echoctl plugin call acme-history log '{"max":1}'` | `null`; one commit — note its `subject` and `rev` |
| 3 | `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-history commit '{"paths":["Notes/Routine.md"],"message":"Routine notes"}'` | `{"rev": "<40 hex>"}`: the marker limits nothing |
| 4 | `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-history log '{"max":1}'`; `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-history status` | One commit, subject `Routine notes` (step 3's); the status lists nothing: reads answer too |

## Expected Final State
`Notes/Routine.md` is in the vault, committed by step 3.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`
