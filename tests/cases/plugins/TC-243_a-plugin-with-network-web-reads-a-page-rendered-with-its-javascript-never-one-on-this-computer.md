---
id: TC-243
title: A plugin with uses.network.web reads a page as a browser shows it (render, its JavaScript run, in a hidden process of Echo); without the capability, or for a page on this computer, the user's network or plain http, render is refused before anything is asked
priority: high
type: functional
requirements: [F-PLUG-155]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, web, render, cli]
---

# TC-243: A plugin reads a rendered page, never one on this computer

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- A Mac or a Windows computer with a logged-in GUI session (the page renders in the system's own webview, hidden: WKWebView or WebView2).
- Data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> 48230 --set small`; then `echo 'export ECHOCTL_TEST_HOSTS=news.acme.example=http://127.0.0.1:48231' >> <dir>/env` (a development build sends the made-up public host `news.acme.example` to the made-up site below; `tests/app_profile.md` § Plugins against fake services); every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; E=target/debug/echoctl` (built right before); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The made-up site `tests/fixtures/web-site` on port 48231 (`rendered.html` writes its text with a script):

```bash
nohup python3 -m http.server 48231 --bind 127.0.0.1 --directory tests/fixtures/web-site > "$(dirname "$ECHOCTL_HOME")/site.out" 2>&1 & echo $! > "$(dirname "$ECHOCTL_HOME")/site.pid"
for i in $(seq 50); do curl -s -o /dev/null http://127.0.0.1:48231/ && break; sleep 0.1; done
```
- Two plugin folders, each `main.js` `export function apply(ctx) { ctx.command('read', (a) => ctx.web.read(a.url, a.opts || {})) }`:
  - `<dir>/renderer`: `echo-plugin.json` `{"id": "renderer", "name": "Acme renderer", "version": "0.1.0", "main": "main.js", "uses": {"network": {"web": true}}}`
  - `<dir>/lister`: `echo-plugin.json` `{"id": "lister", "name": "Acme lister", "version": "0.1.0", "main": "main.js", "uses": {"network": {"hosts": ["news.acme.example"]}}}`

**Changes:** `<config dir>/plugins/` (the installed plugins)

## Test Data

| Field | Value |
|---|---|
| Plugins | Acme renderer (`renderer`, `uses.network.web: true`); Acme lister (`lister`, only `network.hosts: news.acme.example`) |
| Public host | `https://news.acme.example` → the made-up site on `http://127.0.0.1:48231` (development build only) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install <dir>/renderer --yes`; `$E plugin install <dir>/lister --yes` | `Installed Acme renderer 0.1.0`, `Installed Acme lister 0.1.0` |
| 2 | `$E plugin call renderer read '{"url":"https://news.acme.example/rendered.html"}'` | Exit 0: `title` `Acme rollout board`, `needs_js` true, `markdown` without "Built by script", no `rendered`: fetched, the page's script isn't run |
| 3 | `$E plugin call renderer read '{"url":"https://news.acme.example/rendered.html","opts":{"render":true}}'` | Exit 0 within a few seconds, no window, Dock icon or taskbar button shown: `rendered` true, `url` `https://news.acme.example/rendered.html`, `markdown` has "Built by script: the rollout starts with three stores in November", `links` `[{"text":"the plan","href":"https://news.acme.example/plan.pdf"}]` |
| 4 | `wc -l < "$(dirname "$ECHOCTL_HOME")/site.out"`; then `$E plugin call lister read '{"url":"https://news.acme.example/rendered.html","opts":{"render":true}}'`; then `wc -l` again | Exit 1: `Error: lister renders a page only with uses.network.web (reading any page on the public internet), which its sheet doesn't ask for`; the site's log has the same number of lines |
| 5 | One by one, `renderer read` with `"opts":{"render":true}` of `https://127.0.0.1/`, `https://192.168.1.1/`, `https://169.254.169.254/latest/meta-data/`, `https://[::1]/` and `http://news.acme.example/rendered.html`; then `wc -l` of the site's log | Each: exit 1, `Error: Echo renders only a page on the public internet: …` (the first four: `… is on this computer or your own network`; the last: `… only over https`); the site's log is unchanged |
| 6 | `$E plugin call renderer read '{"url":"https://news.acme.example/rendered.html","opts":{"render":"yes"}}'` | Exit 1: `render is true or false` |

## Expected Final State
Both plugins are installed; the made-up site runs; no `echoctl render-page` process is left (`pgrep -f render-page` prints nothing; on Windows no `echoctl.exe` with `render-page` in its command line) and, on Windows, no `echo-render-*` folder in `%TEMP%`.

## Teardown
- `kill $(cat "$(dirname "$ECHOCTL_HOME")/site.pid")`; `$E plugin remove renderer --delete-data`; `$E plugin remove lister --delete-data`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from #835's acceptance, not from the code. The site is on this computer: a development build's `ECHOCTL_TEST_HOSTS` stands for the public host, so nothing is fetched from the network.
- What a served site can't show is proven by the GUI tests `cargo test --test render_page -- --ignored --test-threads=1` (a Mac or Windows with a GUI session). They cover:
  - a page whose script tries to reach this computer and the user's network from every frame and in every way, with a canary that must see nothing;
  - storage that doesn't outlive a render;
  - a page that never stops;
  - a DOM that is too large;
  - only the hosts the gate lets through.

  The gate itself is covered by the `web::gate` unit tests.
- Windows: the same steps and results; the child puts the made-up site behind TLS itself (a development build only), so the page is asked for over https through the gate, as a real one is.
