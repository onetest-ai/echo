---
id: TC-241
title: A plugin with uses.network.web reads a public https page and follows its redirect, its own sheet line saying so; plain http, another port, this computer and the user's network are refused before anything is asked
priority: high
type: functional
requirements: [F-PLUG-152]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, web, cli]
---

# TC-241: A plugin with uses.network.web reads a public https page, never this computer

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> 48230 --set small`; then `echo 'export ECHOCTL_TEST_HOSTS=news.acme.example=http://127.0.0.1:48231' >> <dir>/env` (a development build sends the made-up public host `news.acme.example` to the made-up site below; `tests/app_profile.md` § Plugins against fake services); every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; E=target/debug/echoctl` (built right before); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The made-up site `tests/fixtures/web-site` on port 48231 (its pages: `pilot.html`, the folder `docs/` with its `index.html`):

```bash
nohup python3 -m http.server 48231 --bind 127.0.0.1 --directory tests/fixtures/web-site > "$(dirname "$ECHOCTL_HOME")/site.out" 2>&1 & echo $! > "$(dirname "$ECHOCTL_HOME")/site.pid"
for i in $(seq 50); do curl -s -o /dev/null http://127.0.0.1:48231/ && break; sleep 0.1; done
```
- A plugin folder `<dir>/crawler`: `echo-plugin.json` `{"id": "crawler", "name": "Acme crawler", "version": "0.1.0", "main": "main.js", "uses": {"network": {"web": true}}}` and `main.js` `export function apply(ctx) { ctx.command('read', (a) => ctx.web.read(a.url, {})); ctx.command('http', (a) => ctx.http.fetch(a.url, {})) }`

**Changes:** `<config dir>/plugins/` (the installed plugin)

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme crawler (`crawler`) 0.1.0, `uses.network.web: true`, no hosts, no settings |
| Public host | `https://news.acme.example` → the made-up site on `http://127.0.0.1:48231` (development build only) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install <dir>/crawler --yes` | The sheet it prints lists "Runs its own code in Echo, with your rights (new)" and, as a line of its own, "Reads web pages on the public internet (https) (new)"; `Installed Acme crawler 0.1.0` |
| 2 | `$E plugin call crawler read '{"url":"https://news.acme.example/pilot.html"}'` | Exit 0, JSON: `url` `https://news.acme.example/pilot.html`, `title` `Acme pilot notes`; its `links` against `https://news.acme.example/` (`https://news.acme.example/plan.pdf`) — no dialog, nothing asked of the user |
| 3 | `$E plugin call crawler read '{"url":"https://news.acme.example/docs"}'`; then `grep -c 'GET /docs/ ' "$(dirname "$ECHOCTL_HOME")/site.out"` | Exit 0, `url` `https://news.acme.example/docs/`: the site's 301 to `/docs/` was followed; `1` |
| 4 | `$E plugin call crawler read '{"url":"http://news.acme.example/pilot.html"}'` | Exit 1: `… news.acme.example over plain http: Echo reads a page on the public internet only over https` |
| 5 | `$E plugin call crawler read '{"url":"https://news.acme.example:8443/pilot.html"}'` | Exit 1: `… news.acme.example on port 8443: Echo reads a page on the public internet only on ports 80 and 443` |
| 6 | `wc -l < "$(dirname "$ECHOCTL_HOME")/site.out"`; then, one by one, `read` of `https://127.0.0.1/pilot.html`, `https://192.168.1.1/`, `https://169.254.169.254/latest/meta-data/`, `https://[::1]/` and `https://[fd00::1]/`; then `wc -l` again | Each: exit 1, `Error: crawler reads only pages on the public internet and the addresses it may reach: <host> is on this computer or your own network`; the site's log has the same number of lines: nothing was asked |
| 7 | `$E plugin call crawler http '{"url":"https://news.acme.example/pilot.html"}'` | Exit 1, `may reach only`: `ctx.http.fetch` isn't widened |

## Expected Final State
Acme crawler is installed; the made-up site runs.

## Teardown
- `kill $(cat "$(dirname "$ECHOCTL_HOME")/site.pid")`; `$E plugin remove crawler --delete-data`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from #830's acceptance, not from the code. The site is on this computer: a development build's `ECHOCTL_TEST_HOSTS` stands for the public host, so nothing is fetched from the network.
- A plugin without the capability is unchanged: TC-192 step 2 (a redirect not followed) and step 5 (an address not given).
- What the made-up site can't serve — a redirect from a public page to this computer, the user's network, plain http, another port or `file:`; a sixth redirect; a name whose addresses are private or mixed; an IPv4-mapped private address; the address Echo checked being the one connected to, with no proxy; an update that adds the line asking again — is proven by the Rust tests `jshost::web::tests::a_plugin_with_network_web_reads_any_public_https_page_following_redirects_each_checked_again`, `jshost::web::tests::a_public_page_is_reached_at_the_public_address_echo_checked`, `plugins::capabilities::tests::reading_the_public_web_is_a_line_of_its_own_on_the_sheet` and `plugins::tests::an_update_that_adds_reading_the_public_web_asks_again`.
