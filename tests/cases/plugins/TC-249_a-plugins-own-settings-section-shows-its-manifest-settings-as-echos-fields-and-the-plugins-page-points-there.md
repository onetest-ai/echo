---
id: TC-249
title: A plugin's own Settings section shows the manifest settings it names as Echo's fields (an address with its notes, a secret as set or not), saves them as the Plugins page does and tells the section what changed; the Plugins page shows those settings only as "Set in <section> — Open"; a key it takes itself (draw false) has no field; an unknown key says so; switched off, the Plugins page shows the fields again
priority: high
type: functional
requirements: [F-PLUG-160]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, settings, client-half, secrets]
---

# TC-249: A plugin's own Settings section shows its manifest settings

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Small data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set small`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); CLI steps run in Bash after `. <dir>/env` (abort if `$ECHOCTL_HOME` isn't inside `<dir>`), `E=target/debug/echoctl` built right before the run; the browser 1201 × 868
- A made-up local marketplace `acme-voice` with one plugin, **Acme voice**: a speech server address (`server`, listed by the user), an API key (`api_key`, a secret), a bot token it takes in its own flow (`token`), and a voice name (`voice`) its section doesn't claim. Its section claims `server`, `api_key`, `token` (not drawn) and `nope` (not a setting), counts what it was told changed, and draws a Check line. The only files this case writes, in the run's own scratch folder:

```bash
M="$(dirname "$ECHOCTL_HOME")/market-voice"; mkdir -p "$M/plugins/acme-voice"
echo '{"name":"acme-voice","plugins":[{"id":"acme-voice","source":"./plugins/acme-voice"}]}' > "$M/echo-marketplace.json"
cat > "$M/plugins/acme-voice/echo-plugin.json" <<'J'
{"id":"acme-voice","name":"Acme voice","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js",
 "uses":{"network":{"hosts_from":["server"]}},
 "settings":[{"key":"server","label":"Speech server"},{"key":"api_key","label":"API key","type":"secret"},
  {"key":"token","label":"Bot token","type":"secret"},{"key":"voice","label":"Voice","default":"calm"}]}
J
cat > "$M/plugins/acme-voice/client.js" <<'JS'
export function apply(ctx) {
  let told = []
  ctx.settings.section({ id: 'main', label: 'Acme voice',
    settings: ['server', 'api_key', { key: 'token', draw: false }, 'nope'],
    changed: (key) => { told.push(key); ctx.render() },
    render: () => `<h2>Acme voice</h2><p id="acme-told">Told: ${told.join(',') || 'nothing'}</p>` })
}
JS
$E plugin marketplace add "$M" --name acme-voice
```

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, its settings and secrets)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install acme-voice --yes`; reload `{{base_url}}/`; navigate to `{{base_url}}/#settings/acme-voice:main` | `Installed Acme voice 0.1.0`; under "Its settings", above the heading "Acme voice": the fields "Speech server" (a text box, empty) and "API key" (a password box with the placeholder "not set", no Forget button); no field "Bot token" and none "Voice"; the error line "“nope” isn't one of its settings"; the text "Told: nothing" |
| 2 | Type `http://tts.acme.example:8080` in "Speech server" and leave the box (Tab) | Within 3 s the text reads "Told: server"; the box keeps the address; under it notes (`role="note"`): plain http, and that Echo's window is one page; within 10 s more the window loads again by itself (its network policy now has the address: Echo's own reload) and shows the section with the address and "Told: nothing"; `cat "$ECHOCTL_CONFIG_DIR/plugins/addresses/acme-voice.json"` holds `"server"` with the address |
| 3 | Type `sk-acme-123` in "API key" and leave the box | Within 3 s "Told: api_key"; the box is empty again, its placeholder "•••• set — type to replace", and a button "Forget" beside it; the key's text is nowhere on the page (`document.body.innerHTML` has no `sk-acme-123`) |
| 4 | Navigate to `{{base_url}}/#settings/plugins`, tab "Installed", choose "Acme voice" | Under "Its settings": the field "Voice" (value `calm`); no field "Speech server", "API key" or "Bot token"; one row "Speech server, API key, Bot token" with "Set in Acme voice" and a button "Open" |
| 5 | Click "Open" in that row | The URL is `{{base_url}}/#settings/acme-voice:main`; the section shows its fields as in step 3 |
| 6 | Click "Forget" beside "API key" | Within 3 s "Told: api_key,api_key"; the placeholder is "not set", no Forget button |
| 7 | Navigate to `{{base_url}}/#settings/plugins`, click the switch "Acme voice on" in the detail (off) | Settings has no "From plugins" button "Acme voice"; the detail's "Its settings" shows the fields "Speech server" (with the address), "API key", "Bot token" and "Voice", and no "Set in" row |

## Expected Final State
Acme voice installed and off; its address kept, its API key forgotten.

## Teardown
- `$E plugin remove acme-voice --delete-data`; `$E plugin marketplace remove acme-voice`; `rm -r "$M"`

## Notes
- Besides `echoctl`, this case runs `cat`, `mkdir` and `rm`.
