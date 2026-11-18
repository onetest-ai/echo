---
id: TC-190
title: A plugin reads a web page as Markdown — its text, links and pictures, never its scripts — only once the user listed its address; a page that needs JavaScript says so
priority: high
type: functional
requirements: [F-PLUG-110]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, web, cli]
---

# TC-190: A plugin reads a web page as Markdown once its address is listed

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> 48220 --set small`; every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; E=target/debug/echoctl` (built right before); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The made-up site `tests/fixtures/web-site` served on this computer, port 48221 (its pages: `pilot.html` an article with a figure, a tracking pixel, a list, a link to `plan.pdf` and a script link; `app.html` an empty app page built by a script; `docs/` a folder; `img/waves.png`, `plan.pdf`):

```bash
nohup python3 -m http.server 48221 --bind 127.0.0.1 --directory tests/fixtures/web-site > "$(dirname "$ECHOCTL_HOME")/site.out" 2>&1 & echo $! > "$(dirname "$ECHOCTL_HOME")/site.pid"
```
- A copy of the test marketplace, and Web demo installed from it: `C=$(mktemp -d)/m; cp -R tests/fixtures/plugins "$C"; $E plugin marketplace add "$C" --name test-plugins; $E plugin install web-demo --from test-plugins --yes`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, the address the user lists)

## Test Data

| Field | Value |
|---|---|
| Plugin | Web demo (`web-demo`) 0.1.0: a host half whose commands `read`, `fetch` and `markdown` answer `ctx.web.read(url, opts)`, `ctx.web.fetch(url, opts)` and `ctx.web.markdown(html, opts)`; `uses.network.hosts_from: ["site"]`, setting `site` (empty) |
| Site | `http://127.0.0.1:48221/` (`tests/fixtures/web-site`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | The install's sheet (precondition's output) | It lists "Runs its own code in Echo, with your rights" and "Talks to the addresses you list in its setting “site” (wildcards allowed, plain http too; its window part also over WebSocket)" — no line of its own for reading pages |
| 2 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/pilot.html"}'`; then `grep -c pilot.html "$(dirname "$ECHOCTL_HOME")/site.out"` | Exit 1: `Error: web-demo may reach only the hosts its manifest names () and the addresses you list in its settings ()`; `0`: nothing was asked of the site |
| 3 | `$E plugin set web-demo site "http://127.0.0.1:48221"`; then step 2's call again | Exit 0, JSON: `url` `http://127.0.0.1:48221/pilot.html`, `title` `Acme pilot notes` (its heading, not the `<title>` and not what its script would set), `lang` `en`, `source` `html`, `needs_js` false, `truncated` false |
| 4 | Read step 3's `markdown` | It starts "The Acme checkout pilot moves to 18 November."; holds `![Stores by wave](http://127.0.0.1:48221/img/waves.png)` with `Figure 1. The three waves` on the paragraph under it, `## Next steps`, `- Omar Haddad signs off the store list.`, `[pilot plan](http://127.0.0.1:48221/plan.pdf)` and "Run the demo stays plain words." as plain words; no "Home", "People", "Acme intranet", "changed by a script", `javascript`, `alert` or `pixel.acme.example` |
| 5 | Read step 3's `links` and `images` | `links`: one, `{"href": "http://127.0.0.1:48221/plan.pdf", "text": "pilot plan"}`; `images`: two — `img/waves.png` (alt "Stores by wave", caption "Figure 1. The three waves", 64 × 48) and `pilot.html` (alt "Not a picture"); no tracking pixel, none `saved` (nothing but the page was asked: `grep -c waves.png "$(dirname "$ECHOCTL_HOME")/site.out"` is `0`) |
| 6 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/app.html"}'` | `title` `Acme dashboard`, `needs_js` true, `markdown` only "You need to enable JavaScript to run this app." (`source` `noscript`): the page's script was never run |
| 7 | `$E plugin call web-demo markdown '{"html":"<p>Hi <a href=\"/x\">there</a> <a href=\"javascript:alert(1)\">run</a></p><script>alert(2)</script>","opts":{"base":"https://acme.example/"}}'` | `markdown` `Hi [there](https://acme.example/x) run`: the link resolved against `base`, the script link plain words, the script gone; nothing asked of any site |

## Expected Final State
Web demo is installed with `http://127.0.0.1:48221` listed; the made-up site runs.

## Teardown
- Kept for the next stages in S-44. On its own: `kill $(cat "$(dirname "$ECHOCTL_HOME")/site.pid")`; `$E plugin remove web-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from the feature's text (F-PLUG-110), not from the code. The host half runs here in the terminal's own process (`echoctl plugin call`), as in the app.
- What a case can't show is proven by the Rust tests: every kind of hostile page (`web::read::tests::every_trick_of_a_hostile_page_comes_out_as_plain_words`), 10,000-deep nesting, a page past 250,000 elements, broken HTML, JSON-LD, a framework's JSON state and Open Graph alone (`web::read::tests`), a 50 MB answer stopped as it came, a compressed answer refused, a charset (`jshost::web::tests::a_plugin_reads_a_page_it_may_reach_within_its_caps_and_saves_its_pictures`).
