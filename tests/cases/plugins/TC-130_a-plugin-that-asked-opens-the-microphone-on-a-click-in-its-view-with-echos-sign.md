---
id: TC-130
title: A plugin that asked for the microphone opens it on the user's click in its own view, and Echo's recording sign names it while it is open
priority: high
type: functional
requirements: [F-PLUG-85, F-PLUG-86]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, client-half, microphone, privacy]
---

# TC-130: A plugin that asked opens the microphone on a click in its view, with Echo's sign

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set small`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); the browser 1201 × 868
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- **The browser's microphone is a stand-in, not the computer's.** The Playwright MCP's browser is started once for the whole session, without Chromium's fake-device switches (`--use-fake-device-for-media-stream`, `--use-fake-ui-for-media-stream`), and a case can't start it again with them. So, once, before the first window step, in this tab, with `browser_run_code_unsafe`:

```js
async (page) => {
  await page.addInitScript(() => {
    window.__gum = 0 // how many times the page asked the browser for a device
    MediaDevices.prototype.getUserMedia = async function (c) {
      window.__gum++
      if (!c || !c.audio || c.video) throw new DOMException('Only a stand-in microphone here', 'NotAllowedError')
      const ac = new AudioContext(), d = ac.createMediaStreamDestination(), o = ac.createOscillator()
      o.connect(d); o.start()
      return d.stream // one live audio track, a tone; stop() ends it
    }
  })
  await page.goto(page.url())
}
```

  It stays on this tab until the tab is closed (the scenario closes its tab at its end). What the browser itself grants is Echo's only in the native window: `plugins/TC-131`, by hand.
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
| `S` (the state read in the page, `browser_evaluate`) | `({ mic: document.body.dataset.acmeMic, plain: document.body.dataset.plainMic, pill: document.querySelector('[aria-label="A plugin hears your microphone"]')?.innerText ?? null, track: window.__acmeTrack?.readyState, gum: window.__gum })` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-voice --yes`; `echoctl plugin install acme-plain --yes` | Acme voice's sheet has the line `• Hears your microphone in its own view: only after you click there, with Echo's recording sign on while it listens, and never while Echo records a call or a memo; it stops when you leave the view. What it hears can go to the addresses it may reach (new)`; Acme plain's has no such line; both installed |
| 2 | Navigate to `{{base_url}}/#acme-voice:talk`; wait 6 s; then reload it with nothing touching the page for 5 s (`browser_run_code_unsafe`: `async (page) => { await page.reload(); await page.waitForTimeout(5000) }`; why: `plugins/TC-133`'s Notes); read `S`; look at the top bar | `gum` `0`, no `mic`, no `pill`; the top bar shows Record, no microphone sign |
| 3 | Click "Talk"; wait 2 s; read `S`; look at the top bar | `mic` is `open 1 live`, `track` `live`, `gum` `1`; the top bar's pill (group "A plugin hears your microphone") reads "Acme voice hears your microphone" with a Stop button, in place of Record |

## Expected Final State
Acme voice and Acme plain installed and on; Acme voice holds the microphone, its pill in the top bar.

## Teardown
- Click "Hang up"
- `echoctl plugin remove acme-voice --delete-data`; `echoctl plugin remove acme-plain --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; close the tab; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the features' text (F-PLUG-85, F-PLUG-86), not from the code.
- The ways it stops are `plugins/TC-132`'s; the refusals `plugins/TC-133`'s; the native webview, the tray and the system's own microphone indicator `plugins/TC-131`'s (a person).
