---
id: TC-200
title: A plugin shows a file of a folder it was given in Finder on the user's click (on test data only in the open log), and nothing without a click or outside the folder
priority: medium
type: functional
requirements: [F-PLUG-115]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, folders, open-log]
---

# TC-200: A plugin shows a file of its folder in Finder on your click, and nothing outside it

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (its `env` sets `ECHOCTL_TEST_OPEN_LOG_ONLY`: what Echo would show in Finder goes to `<dir>/open.log`, nothing opens); the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie); the browser 1201 × 868
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the picker)
- A made-up note and a made-up plugin **Acme reveal** (folders it may read, a host half and a window half) in a local marketplace `acme-local`. Its window half asks to show the note once as it loads (no click) and writes the answer in `document.body.dataset.acmeLoad`; its activity (`#acme-reveal:main`) has the buttons "Show in Finder" (the note) and "Show outside" (`../outside/secret.md`), each writing its answer in `document.body.dataset.acmeShow`:

```bash
mkdir -p "$D/vault/Projects" "$D/outside"
printf '# Acme plan\n' > "$D/vault/Projects/Plan.md"; printf 'x\n' > "$D/outside/secret.md"
M="$D/market-reveal"; mkdir -p "$M/plugins/acme-reveal"
echo '{"name":"acme-local","plugins":[{"id":"acme-reveal","source":"./plugins/acme-reveal"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-reveal","name":"Acme reveal","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js","uses":{"folders":{"what":"notes"}}}' > "$M/plugins/acme-reveal/echo-plugin.json"
cat > "$M/plugins/acme-reveal/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('notes'))
  ctx.command('root', () => ctx.fs.folders()[0] || '')
}
JS
cat > "$M/plugins/acme-reveal/client.js" <<'JS'
export function apply(ctx) {
  let root = ''
  const show = (path, key) => ctx.system.reveal(root, path).then((v) => { document.body.dataset[key] = 'shown ' + v }, (e) => { document.body.dataset[key] = 'refused: ' + e.message })
  ctx.call('root').then((x) => { root = x; show('Projects/Plan.md', 'acmeLoad'); ctx.render() })
  ctx.activities.add({ id: 'main', label: 'Acme reveal', icon: '', render: () => `<h2>Acme reveal</h2><button id="acme-show">Show in Finder</button> <button id="acme-out">Show outside</button>` })
  ctx.listen('click', (e) => {
    if (e.target.id === 'acme-show') show('Projects/Plan.md', 'acmeShow')
    if (e.target.id === 'acme-out') show('../outside/secret.md', 'acmeShow')
  })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant); `<dir>/open.log`

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme reveal (`acme-reveal`) 0.1.0 with `uses.folders` |
| `S` (read in the page, `browser_evaluate`) | `({ load: document.body.dataset.acmeLoad, show: document.body.dataset.acmeShow })` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-reveal --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-reveal add`; `wc -l < "$D/open.log" 2>/dev/null \|\| echo 0` (call it N) | `Installed Acme reveal 0.1.0`; the vault's path (its real path, e.g. under `/private/var/…` on macOS) |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-reveal:main`; nothing touching the page, wait 3 s; read `S`; `wc -l < "$D/open.log" 2>/dev/null \|\| echo 0` | The page shows "Acme reveal" with "Show in Finder" and "Show outside"; `load` is `refused: Finder or Explorer opens only on your click`; the count is still N |
| 3 | Click "Show in Finder"; wait 1 s; read `S`; `tail -n 1 "$D/open.log"` | `show` is `shown true`; the log's last line is `reveal <the vault's path from step 1>/Projects/Plan.md`; no Finder window opened |
| 4 | Click "Show outside"; wait 1 s; read `S`; `wc -l < "$D/open.log"` | `show` starts with `refused:` and says `isn't a path inside`; the count is N + 1 (only step 3's line) |

## Expected Final State
Acme reveal is installed with the vault given to it; `open.log` holds one `reveal` line for the note; nothing in the vault changed.

## Teardown
- `echoctl plugin remove acme-reveal --delete-data`; `echoctl plugin marketplace remove acme-local`

## Notes
- Written from the feature's text (F-PLUG-115), not from the code. Never click "Show in Finder" without the test data's `ECHOCTL_TEST_OPEN_LOG_ONLY`: it would open Finder.
- A link out of the folder, a missing file, a folder it wasn't given, a plugin that didn't ask for folders or is off, and the Windows argument (`explorer.exe /select,"<path>"`, a quote or control character refused) are the Rust tests `window::plugins::a_plugin_shows_in_the_file_manager_only_what_is_in_a_folder_the_user_gave_it_and_test_data_only_logs_it` and `app::a_file_is_shown_in_finder_or_explorer_as_one_argument_never_through_a_shell`; the click guard is `node crates/echoctl/ui/copy-guard.test.mjs`.
- Finder itself showing the file selected (`open -R`) and Explorer (`/select`) are seen only in the native app, by a person, outside test data.
