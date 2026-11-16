---
id: TC-015
title: An update that asks for more shows only the new lines on a sheet and waits for a yes — the terminal refuses without --yes, Cancel leaves the old version, the sheet's Update installs it
priority: high
type: functional
requirements: [F-PLUG-06]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, update, permissions]
---

# TC-015: An update that asks for more waits for a yes

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); CLI steps run in Bash after `. <dir>/env`; the browser 1201 × 868
- A made-up local marketplace `acme-local` with **Acme ping 0.1.0**, which talks to one host (no code of its own). The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-ping"
echo '{"name":"acme-local","plugins":[{"id":"acme-ping","source":"./plugins/acme-ping"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-ping","name":"Acme ping","version":"0.1.0","summary":"A made-up plugin for tests","uses":{"network":{"hosts":["api.acme.example"]}}}' > "$M/plugins/acme-ping/echo-plugin.json"
```
- The plugin is installed: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`), then `echoctl plugin install acme-ping --yes` (prints `Installed Acme ping 0.1.0`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | The marketplace publishes 0.2.0, which asks for one more thing: `echo '{"id":"acme-ping","name":"Acme ping","version":"0.2.0","summary":"A made-up plugin for tests","uses":{"network":{"hosts":["api.acme.example"]},"deliver":{"to":["mail"]}}}' > "$M/plugins/acme-ping/echo-plugin.json"`; `echoctl plugin marketplace update acme-local` | The second command ends without an error |
| 2 | `echoctl plugin update acme-ping` (no `--yes`) | It prints `Acme ping 0.2.0 — from acme-local (local-…)`, then `• Talks to api.acme.example over HTTPS` (no "(new)") and `• Sends messages: mail (to you, or to the addresses you set on a routine) (new)`; it then fails with `run again with --yes to agree`; no line `Updated Acme ping to 0.2.0` |
| 3 | `echoctl plugin list` | The installed entry `acme-ping` is still `"version": "0.1.0"`; its `update` has `"version": "0.2.0"` and `new_uses` with the one line `Sends messages: mail (to you, or to the addresses you set on a routine)` |
| 4 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#settings/plugins` (tab "Installed") | The row "Acme ping" shows "0.1.0" and a mark "Update to 0.2.0"; the detail has a button "Update to 0.2.0" |
| 5 | Click the button "Update to 0.2.0" in the detail | A dialog "Acme ping 0.2.0 asks for more" opens, with "from acme-local · commit local-", the warning "Listed by acme-local — Echo hasn’t reviewed it. Install it only if you trust who made it.", the heading "It will be able to" over the one line "Sends messages: mail (to you, or to the addresses you set on a routine)" — and no line "Talks to api.acme.example over HTTPS" (it was agreed before); buttons "Cancel" and "Update to 0.2.0" |
| 6 | Click "Cancel" | The dialog closes; the detail still reads "0.1.0 · from acme-local"; `echoctl plugin list` has `acme-ping` at `"version": "0.1.0"` (nothing was updated without the yes) |
| 7 | Click "Update to 0.2.0" in the detail, then "Update to 0.2.0" in the dialog | The dialog closes; after a moment the detail reads "0.2.0 · from acme-local" and has no button "Update to 0.2.0" |
| 8 | `echoctl plugin list` | `acme-ping` is `"version": "0.2.0"`, `"enabled": true`, its `uses` has both lines (the host line and `Sends messages: mail (to you, or to the addresses you set on a routine)`), and its `update` is `null` |

## Expected Final State
Acme ping is at 0.2.0 with both permission lines agreed; nothing changed until the user said yes on the sheet.

## Teardown
- `echoctl plugin remove acme-ping --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; close the test tab

## Notes
- `plugins::update` refuses a newer version whose new lines were not agreed ("asks for more"); the terminal's way of saying yes is `--yes` (`echoctl plugin update acme-ping --yes` prints `Updated Acme ping to 0.2.0`), the window's is the sheet's "Update to 0.2.0". Step 2 is the terminal's wait, steps 5–7 the window's.
- A made-up plugin never used by another case: it has no `main` or `client`, so nothing runs; its manifest is all Echo reads.
