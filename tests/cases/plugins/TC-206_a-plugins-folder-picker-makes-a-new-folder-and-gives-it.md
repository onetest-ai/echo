---
id: TC-206
title: In the folder picker a plugin opens, the user makes a new folder and that new folder is the one the plugin is given
priority: medium
type: functional
requirements: [F-PLUG-121]
module: plugins
run: person
platform: any
data: small
tags: [plugins, host-half, folders, native-dialog]
---

# TC-206: A plugin's folder picker makes a new folder and gives it

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set built right before the run: `tests/fixtures/make_data.py <dir> --ai fake`; every Bash call starts with `. <dir>/env`; `echoctl` is a **release** build (`cargo build --release`, `target/release/echoctl`): a debug build with `ECHOCTL_PICK_FOLDER` set answers the picker with no dialog
- A made-up plugin **Acme vaults** that asks for a folder in a local marketplace `acme-local`:

```bash
M="<dir>/market-vaults"; mkdir -p "$M/plugins/acme-vaults"
echo '{"name":"acme-local","plugins":[{"id":"acme-vaults","source":"./plugins/acme-vaults"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-vaults","name":"Acme vaults","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"folders":{"what":"vaults"}}}' > "$M/plugins/acme-vaults/echo-plugin.json"
printf "export function apply(ctx) {\n  ctx.command('add', async () => await ctx.system.pickFolder('a new vault'))\n  ctx.command('roots', () => ctx.fs.folders())\n}\n" > "$M/plugins/acme-vaults/main.js"
echoctl plugin marketplace add "$M" --name acme-local; echoctl plugin install acme-vaults --yes
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant); a new folder `<dir>/Acme vault`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-vaults add` | The system's folder picker opens with the prompt "a new vault"; it has a "New Folder" button (macOS) / "Make New Folder" (Windows) |
| 2 | In the picker go to `<dir>`, press "New Folder", name it `Acme vault`, select it and press "Choose" / "OK" | The command prints the new folder's path, `<dir>/Acme vault` |
| 3 | `echoctl plugin call acme-vaults roots` | The list holds `<dir>/Acme vault` |

## Expected Final State
`<dir>/Acme vault` exists, empty, and is the folder Acme vaults was given.

## Teardown
- `echoctl plugin remove acme-vaults --delete-data`; `echoctl plugin marketplace remove acme-local`; remove `<dir>`

## Notes
- A native dialog: never run by an agent (`.ai-run/guides/testing/testing.md` R11). macOS's `choose folder` panel has "New Folder" (StandardAdditions sets the open panel's `canCreateDirectories`); Windows' folder browser shows "Make New Folder" (`ShowNewFolderButton`, `app::windows_pickers_take_the_prompt_from_the_environment_never_in_the_script`).
