---
id: TC-073
title: Nothing a vault's own git settings, hooks or attributes name runs when a plugin reads or commits it — no hook (a file's or a setting's), fsmonitor, filter or signing program
priority: critical
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git, security]
---

# TC-073: A vault's own git settings run nothing

**Module:** Plugins | **Priority:** Critical | **Type:** Functional

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

- A script that leaves a mark were anything to run it: `printf '#!/bin/sh\ntouch "%s/ran"\ncat\n' "$D" > "$D/mark.sh"; chmod 755 "$D/mark.sh"`

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/history/` (a repository), `$D/hooks/`, the plugin's store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-history --yes`; `ECHOCTL_PICK_FOLDER="$D/history" echoctl plugin call acme-history add`; `echoctl plugin call acme-history init`; `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot planned.\n"}'`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: first notes"}'` | `Installed Acme history 0.1.0`; the vault's path; its `info`; `null`; `{"rev": "<40 hex>"}` |
| 2 | The repository's own settings name the mark script for hooks (a hooks folder and a hook in the settings), the fsmonitor, a filter and signing: `mkdir -p "$D/hooks"; for h in pre-commit post-commit reference-transaction; do cp "$D/mark.sh" "$D/hooks/$h"; done; printf '[core]\n\tfsmonitor = %s\n\thooksPath = %s\n[filter "x"]\n\tclean = %s\n\trequired = true\n[commit]\n\tgpgSign = true\n[gpg]\n\tprogram = %s\n[hook "x"]\n\tevent = pre-commit\n\tevent = post-commit\n\tcommand = %s\n' "$D/mark.sh" "$D/hooks" "$D/mark.sh" "$D/mark.sh" "$D/mark.sh" >> "$D/history/.git/config"; mkdir -p "$D/history/.git/info"; printf '*.md filter=x\n' > "$D/history/.git/info/attributes"` | Nothing printed |
| 3 | `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot signed off.\n"}'`; `echoctl plugin call acme-history status`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: signed off"}'`; `echoctl plugin call acme-history log '{"max":1}'`; `echoctl plugin call acme-history show '{"rev":"HEAD","path":"Notes/Acme.md"}'` | `null`; the list holds `{"path": "Notes/Acme.md", "code": " M"}`; `{"rev": "<40 hex>"}`; one commit `Acme: signed off`; `{"text": "# Acme\nPilot signed off.\n", …}` — the text as on the disk, no filter |
| 4 | `ls "$D/ran"` | `No such file or directory`: no hook, fsmonitor, filter or signing program ran |

## Expected Final State
`$D/history` holds the commit `Acme: signed off` by `Echo (acme-history)`, its settings still naming the mark script; no `$D/ran`.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`
