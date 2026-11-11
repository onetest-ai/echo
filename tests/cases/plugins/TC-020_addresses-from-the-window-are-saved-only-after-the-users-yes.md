---
id: TC-020
title: Addresses a plugin may reach, sent from the window, are saved only after the user allows them outside the page — a plugin's own window half sending them gets nothing saved; the CLI saves without asking
priority: high
type: functional
requirements: [F-PLUG-14]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, network, settings, client-half, permissions]
---

# TC-020: The window's addresses wait for the user's yes

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic, **the user's answer set to no** (a development build answers the dialog from `ECHOCTL_ALLOW_ADDRESSES`; the dialog itself is TC-021): `ECHOCTL_ALLOW_ADDRESSES=no tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); the browser 1440 × 900
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- A made-up local marketplace `acme-local` with **Acme sneak**: its manifest lists the setting `tts` in `network.hosts_from`; its window half, as soon as it loads, sends the window's own request to save `https://collector.acme.example` in that setting:

```bash
M="$D/market"; mkdir -p "$M/plugins/acme-sneak"
echo '{"name":"acme-local","plugins":[{"id":"acme-sneak","source":"./plugins/acme-sneak"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-sneak","name":"Acme sneak","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js","uses":{"network":{"hosts_from":["tts"]}},"settings":[{"key":"tts","label":"TTS server","type":"string"}]}' > "$M/plugins/acme-sneak/echo-plugin.json"
cat > "$M/plugins/acme-sneak/client.js" <<'JS'
export function apply(ctx) {
  send({ cmd: 'plugin_op', op: 'set', id: 'acme-sneak', key: 'tts', value: 'https://collector.acme.example' })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, the addresses the user lists)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-sneak --yes` | `Installed Acme sneak 0.1.0` |
| 2 | Reload `{{base_url}}/` (the plugin's window half loads and sends its request), navigate to `{{base_url}}/#settings/plugins`, click the row "Acme sneak"; then `echoctl plugin set acme-sneak tts` | The detail shows, in red, `Not saved: you didn't allow Acme sneak to reach https://collector.acme.example`; `value` is `null`: nothing was saved |
| 3 | In the detail's "TTS server" field type `http://192.168.68.*:8808` and leave the field (Tab); then `echoctl plugin set acme-sneak tts` | The detail says `Not saved: you didn't allow Acme sneak to reach http://192.168.68.*:8808`; `value` is still `null` |
| 4 | In the field type `http://example.com` and leave it | It says at once that `“TTS server” isn't an address the plugin may reach` and why (plain http only inside your own network): a wrong list isn't even asked about |
| 5 | `echoctl plugin set acme-sneak tts "http://192.168.68.*:8808"`; then `echoctl plugin set acme-sneak tts` | The first saves without a question (the user at their own terminal); `value` is `http://192.168.68.*:8808` |
| 6 | Reload the window, open Acme sneak's detail again | "TTS server" shows `http://192.168.68.*:8808`; the window half's request was refused again (the same red line, with `https://collector.acme.example`); `echoctl plugin set acme-sneak tts` still gives `http://192.168.68.*:8808` |

## Expected Final State
Only the address the user saved from the CLI is kept; nothing the window sent — the plugin's own request or the user's typing, both answered no — was saved.

## Teardown
- `echoctl plugin remove acme-sneak --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-PLUG-14), not from the code.
- The dialog the user answers is the system's (a release build has no `ECHOCTL_ALLOW_ADDRESSES`): TC-021, run by a person.
