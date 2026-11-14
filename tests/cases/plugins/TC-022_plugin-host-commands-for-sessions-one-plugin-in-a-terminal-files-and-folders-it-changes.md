---
id: TC-022
title: A plugin's sheet names the commands Claude in Echo may run, the folders it may change and other apps' files it reads; in a terminal only the called plugin loads (ctx.terminal); it writes, moves and deletes only inside the folder it was given, and reads only the files it named
priority: high
type: functional
requirements: [F-PLUG-15, F-PLUG-16, F-PLUG-17, F-PLUG-18]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, permissions, folders, sessions]
---

# TC-022: What a plugin may do beyond its store, line by line

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- Made-up folders and two made-up plugins in a local marketplace `acme-local`: **Acme vault** (asks for all four) and **Acme noisy** (its start-up writes a file into its store):

```bash
mkdir -p "$D/vault/People" "$D/outside" "$D/fakehome/.config/acme"
printf '# Dana' > "$D/vault/People/Dana.md"; printf 'old' > "$D/vault/Old.md"; printf 'no' > "$D/outside/secret.md"
printf '{"vaults":["%s"]}' "$D/vault" > "$D/fakehome/.config/acme/app.json"; ln -s "$D/outside" "$D/vault/out"
M="$D/market"; mkdir -p "$M/plugins/acme-vault" "$M/plugins/acme-noisy"
echo '{"name":"acme-local","plugins":[{"id":"acme-vault","source":"./plugins/acme-vault"},{"id":"acme-noisy","source":"./plugins/acme-noisy"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-vault","name":"Acme vault","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"folders":{"what":"vaults","write":true},"sessions":{"skills":true,"commands":["search"]},"files":{"read":["~/.config/acme/app.json"]}}}' > "$M/plugins/acme-vault/echo-plugin.json"
cat > "$M/plugins/acme-vault/main.js" <<'JS'
export function apply(ctx) {
  const r = () => ctx.fs.folders()[0]
  ctx.command('terminal', () => ctx.terminal)
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('write', (a) => ctx.fs.write(r(), a.path, a.text))
  ctx.command('move', (a) => ctx.fs.move(r(), a.from, a.to))
  ctx.command('remove', (a) => ctx.fs.remove(r(), a.path))
  ctx.command('config', (a) => ctx.system.readFile(a.entry))
}
JS
echo '{"id":"acme-noisy","name":"Acme noisy","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js"}' > "$M/plugins/acme-noisy/echo-plugin.json"
echo "export function apply(ctx) { ctx.store.write('started.txt', String(Date.now())); ctx.command('ping', () => 'pong') }" > "$M/plugins/acme-noisy/main.js"
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugins, the folder grant), `$D/vault`, the plugins' stores

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-vault --yes`; `echoctl plugin install acme-noisy --yes`; `rm -f "$ECHOCTL_HOME/.echo/plugins/acme-noisy/started.txt"` | The first sheet lists `Reads and changes the vaults you pick, on this computer — only those: it may write, move and delete files in them`, a line with `lets Claude in Echo run its commands “search” without asking`, and `Reads these files of other apps on this computer: “~/.config/acme/app.json”`; both install |
| 2 | `echoctl plugin call acme-vault terminal` (if it answers `no command`, wait 2 s and run it again); then `ls "$ECHOCTL_HOME/.echo/plugins/acme-noisy/"` | `true`; the listing has no `started.txt`: calling Acme vault didn't start Acme noisy |
| 3 | `echoctl plugin call acme-noisy ping`; then `ls "$ECHOCTL_HOME/.echo/plugins/acme-noisy/"` | `pong`; `started.txt` is there now (its own call starts it) |
| 4 | `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-vault add` | The vault's path: the folder is given to Acme vault |
| 5 | `echoctl plugin call acme-vault write '{"path":"Notes/New.md","text":"# New"}'`; `… move '{"from":"Old.md","to":"Archive/Old.md"}'`; `… remove '{"path":"Notes/New.md"}'`; then `cd "$D/vault" && find . -type f \| sort` | Each answers `null`; the files are `./Archive/Old.md` and `./People/Dana.md` |
| 6 | `… move '{"from":"Archive/Old.md","to":"People/Dana.md"}'`; `… write '{"path":"out/x.md","text":"x"}'`; `… write '{"path":"../outside/x.md","text":"x"}'`; `… remove '{"path":"People"}'`; then `ls "$D/outside"` | Each fails (`is there already`, `leads outside`, not a path inside, `is a folder with files in it`); `$D/outside` still holds only `secret.md` |
| 6a | `… write '{"path":".obsidian/x.json","text":"x"}'`; `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-vault write '{"path":"Notes/R.md","text":"x"}'`; `ECHOCTL_PICK_FOLDER="$ECHOCTL_CONFIG_DIR" echoctl plugin call acme-vault add` | The first fails (`is hidden`); the second fails (`isn't allowed while Claude prepares and asks`: a plugin command a routine runs changes nothing); the third fails (`holds Echo's own files`) |
| 7 | `HOME="$D/fakehome" echoctl plugin call acme-vault config '{"entry":"~/.config/acme/app.json"}'`; then the same with `"entry":"~/.ssh/id_rsa"` | The first prints the file's text (`{"vaults":[…]}`); the second fails: `didn't name “~/.ssh/id_rsa” in uses.files.read` |

## Expected Final State
Acme vault changed only its vault, read only the one file it named; Acme noisy started only when it was called.

## Teardown
- `echoctl plugin remove acme-vault --delete-data`; `echoctl plugin remove acme-noisy --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the features' text (F-PLUG-15–18), not from the code.
- That a read-only routine may run `plugin call acme-vault search` and nothing else of it is the guard's: `guard::a_read_only_routine_reads_the_data_folder_and_changes_nothing`.
