---
id: TC-233
title: A plugin keeps a handed file in its own store and counts it (copyIn, usage), reads the manifest's default for an unset setting, and answers its settings page's connection test so the window half gets only ok and a message with no secret in it
priority: medium
type: functional
requirements: [F-PLUG-140, F-PLUG-141, F-PLUG-142]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, store, settings]
---

# TC-233: Small host additions — copyIn, usage, defaults, the connection test

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data served right before the run: `tests/fixtures/serve.sh up <dir> <port>`; every Bash call starts with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D` (abort if `$ECHOCTL_HOME` isn't inside `<dir>`); `E=target/debug/echoctl`, built right before
- A made-up plugin **Acme voice** (no `uses`; a default, a bool default and a secret):

```bash
mkdir -p "$D/voice" "$D/in"
cat > "$D/voice/echo-plugin.json" <<'JSON'
{"id":"acme-voice","name":"Acme voice","version":"0.1.0","main":"main.js","settings":[
 {"key":"voice","label":"Voice","default":"alloy"},
 {"key":"loud","label":"Loud","type":"bool","default":false},
 {"key":"plain","label":"Plain"},
 {"key":"token","label":"Token","type":"secret"}]}
JSON
cat > "$D/voice/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('get', (a) => ctx.settings.get(a.key))
  ctx.command('keep', async (a, { files }) => await ctx.store.copyIn(files[0], a.to, a.opts))
  ctx.command('usage', (a) => ctx.store.usage(a.dir))
  ctx.command('__testSettings', (a) => {
    const key = ctx.settings.get('token')
    return { ok: key === 'sk-made-up-key', message: `server at ${a.section} answered for ${key}`, extra: key }
  })
}
JS
head -c 5000 /dev/zero > "$D/in/pic.png"
$E plugin install "$D/voice" --yes
```

**Changes:** the installed plugin; `$D/voice`, `$D/in`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `for k in voice loud plain nothing; do $E plugin call acme-voice get "{\"key\":\"$k\"}"; done` | `"alloy"`, `false`, `null`, `null` (the manifest's defaults for what nobody set) |
| 2 | `$E plugin set acme-voice voice rachel`; `$E plugin call acme-voice get '{"key":"voice"}'` | `"rachel"` (what the user set wins) |
| 3 | `$E plugin call acme-voice keep '{"to":"images/pic.png"}' --file "$D/in/pic.png"`; `$E plugin call acme-voice usage '{}'`; `$E plugin call acme-voice usage '{"dir":"images"}'` | `{"path": "images/pic.png", "size": 5000}`; then `{"bytes": 5000, "files": 1}` twice |
| 4 | The same `keep` again; then `keep '{"to":"settings.json"}'` and `keep '{"to":"../x.png"}'`, each with `--file "$D/in/pic.png"` | The first fails `“images/pic.png” is there already`; the others fail naming `settings.json` and `isn't a path inside the plugin's folder`; `usage` still 1 file |
| 5 | `$E plugin set acme-voice token sk-made-up-key`; `$E plugin call acme-voice __testSettings '{"section":"tts"}'` (from the terminal: the host half's own answer) | `{"ok": true, "message": "server at tts answered for sk-made-up-key", "extra": "sk-made-up-key"}` |
| 6 | In the served window's page (Playwright, console): `await (await fetch('api/plugin/acme-voice/call/__testSettings', {method: 'POST', body: JSON.stringify({args: {section: 'tts'}})})).json()` | `{value: {ok: true, message: "server at tts answered for ••••"}}` — no `extra`, no `sk-made-up-key` anywhere in the answer |
| 7 | `$E plugin remove acme-voice --delete-data`, then the same fetch as step 6 | `{error: "acme-voice isn't installed and on"}`; no secret |

## Expected Final State
Acme voice is removed with its data; nothing outside `$D` changed.

## Teardown
- Close the Playwright tab; `tests/fixtures/serve.sh down <dir>`
