---
id: TC-130
title: A plugin that asked for the microphone hears it only after the user's click in its own view, with Echo's sign showing, and it stops on Stop, its own close, leaving the view and the plugin switched off
priority: high
type: functional
requirements: [F-PLUG-85, F-PLUG-86, F-PLUG-87]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, client-half, microphone, privacy]
---

# TC-130: A plugin hears the microphone only on a click in its view, with Echo's sign, until it stops

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set small`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); the browser 1201 × 868
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- The browser may use this computer's microphone for the served window: once, before the first window step, with the Playwright MCP's `browser_run_code_unsafe`: `async (page) => page.context().grantPermissions(['microphone'], { origin: '{{base_url}}' })`. The computer has a microphone and the browser is allowed to use it (macOS: System Settings → Privacy & Security → Microphone). The microphone opens for a moment only; nothing is kept
- A made-up local marketplace `acme-local` with two window halves alike: **Acme voice** (`uses.microphone: {}`) and **Acme plain** (asks for nothing). Each, as it loads, asks for the microphone with no click and writes the answer on the page; its activity (`#<id>:talk`) has the buttons "Talk" (`ctx.microphone.open()`) and "Hang up" (`ctx.microphone.close()`), and writes what happened in `document.body.dataset` (`acmeLoad` / `plainLoad`, `acmeMic` / `plainMic`) and the track in `window.__acmeTrack`. The only files this case writes, in the run's scratch folder:

```bash
M="$D/market-mic"; mkdir -p "$M/plugins/acme-voice" "$M/plugins/acme-plain"
echo '{"name":"acme-local","plugins":[{"id":"acme-voice","source":"./plugins/acme-voice"},{"id":"acme-plain","source":"./plugins/acme-plain"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-voice","name":"Acme voice","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js","uses":{"microphone":{}}}' > "$M/plugins/acme-voice/echo-plugin.json"
echo '{"id":"acme-plain","name":"Acme plain","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js","uses":{}}' > "$M/plugins/acme-plain/echo-plugin.json"
cat > "$M/plugins/acme-voice/client.js" <<'EOF'
export function apply(ctx) {
  const k = ctx.id === 'acme-voice' ? 'acme' : 'plain'
  ctx.microphone.open().then(() => { document.body.dataset[k + 'Load'] = 'opened' }, (e) => { document.body.dataset[k + 'Load'] = 'refused: ' + e.message })
  ctx.activities.add({ id: 'talk', label: ctx.id, icon: '', render: () => `<h2>${ctx.id}</h2><button id="${ctx.id}-talk">Talk</button> <button id="${ctx.id}-hang">Hang up</button>` })
  ctx.listen('click', (e) => {
    if (e.target.id === ctx.id + '-talk') ctx.microphone.open().then((s) => { const t = s.getAudioTracks()[0]; window.__acmeTrack = t; document.body.dataset[k + 'Mic'] = 'open ' + s.getAudioTracks().length + ' ' + t.readyState }, (err) => { document.body.dataset[k + 'Mic'] = 'refused: ' + err.message })
    if (e.target.id === ctx.id + '-hang') { ctx.microphone.close(); document.body.dataset[k + 'Mic'] = 'closed' }
  })
}
EOF
cp "$M/plugins/acme-voice/client.js" "$M/plugins/acme-plain/client.js"
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace and the installed plugins)

## Test Data

| Field | Value |
|---|---|
| Plugins | Acme voice (`acme-voice`) 0.1.0 with `uses.microphone: {}`; Acme plain (`acme-plain`) 0.1.0 asking for nothing; both a window half only |
| `S` (the state read in the page) | `({ mic: document.body.dataset.acmeMic, plain: document.body.dataset.plainMic, pill: document.querySelector('[aria-label="A plugin hears your microphone"]')?.innerText ?? null, track: window.__acmeTrack?.readyState })` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-voice --yes`; `echoctl plugin install acme-plain --yes` | Acme voice's sheet has the line `• Hears your microphone in its own view: only after you click there, with Echo's recording sign on while it listens, and never while Echo records a call or a memo; it stops when you leave the view. What it hears can go to the addresses it may reach (new)`; Acme plain's has no such line; both installed |
| 2 | Navigate to `{{base_url}}/#acme-voice:talk` and reload it; wait 3 s; read `document.body.dataset.acmeLoad` and `document.body.dataset.plainLoad`; look at the top bar | Both `refused: the microphone opens only on your click` (asked as they loaded, with no click); the top bar shows "Record next call", no microphone sign |
| 3 | Click "Talk"; wait 2 s; read `S`; look at the top bar | `mic` is `open 1 live`, `track` `live`; the top bar's pill (group "A plugin hears your microphone") reads "Acme voice hears your microphone" with a Stop button, in place of Record |
| 4 | Click the pill's "Stop"; wait 1 s; read `S` | `track` `ended`, `pill` `null`; the top bar shows "Record next call" again |
| 5 | Click "Talk"; wait 2 s; click "Hang up"; wait 1 s; read `S` | After Talk the pill is back; after Hang up `mic` is `closed`, `track` `ended`, `pill` `null` |
| 6 | Click "Talk"; wait 2 s; navigate to `{{base_url}}/#today` (a hash change, no reload); wait 1 s; read `S` | `track` `ended`, `pill` `null`: leaving its view stopped it |
| 7 | Navigate to `{{base_url}}/#acme-plain:talk`; click its "Talk"; wait 2 s; read `S` | `plain` is `refused: Acme plain didn't ask for the microphone when it was installed`; `pill` `null` |
| 8 | Navigate to `{{base_url}}/#acme-voice:talk`; click "Talk"; wait 2 s (the pill shows); then `echoctl plugin disable acme-voice`; wait 6 s; read `S` and `location.hash` | `track` `ended`, `pill` `null`: the plugin switched off took its microphone away; its page is gone (the window went to Calls) |

## Expected Final State
Acme voice installed and off, Acme plain installed and on; no plugin hears the microphone; the top bar shows Record.

## Teardown
- `echoctl plugin remove acme-voice --delete-data`; `echoctl plugin remove acme-plain --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the features' text (F-PLUG-85, F-PLUG-86, F-PLUG-87), not from the code.
- Not here: a refusal while Echo records a call or a memo needs the app recording (the served window has no recorder) — the Rust test `window::plugins` (`a_plugin_gets_the_microphone_only_as_…_never_while_echo_records`) and the node test `plugin-mic.test.mjs` hold it, and `plugins/TC-131` checks it in the native window by hand; "another plugin has the microphone now" can't be reached in the window (leaving one plugin's view stops its microphone first) and is the Rust test's; a click outside the plugin's own view is the node test's.
- The served window's browser grants the microphone through Playwright; what the native webview grants (once for each opening Echo allowed) is `plugins/TC-131`'s, by hand.
