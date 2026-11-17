---
id: TC-192
title: A plugin is told why a page isn't read — not HTML, moved (no redirect followed), past its size, or an address it may not reach — and nothing more is fetched
priority: medium
type: functional
requirements: [F-PLUG-110]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, web, cli]
---

# TC-192: A plugin is told why a page isn't read

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-190's preconditions, run as written (in S-44: kept from stage 1), and its step 3's address listed: `$E plugin set web-demo site "http://127.0.0.1:48221"`; every Bash call with the prelude `. <dir>/env; E=target/debug/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/plan.pdf"}'` | Exit 1: `Error: this is application/pdf, not a web page: Echo reads only HTML (ctx.web.fetch gets a file)` |
| 2 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/docs"}'`; then `grep -c 'GET /docs/' "$(dirname "$ECHOCTL_HOME")/site.out"` | Exit 1: `Error: the page answered 301 (it moved): Echo doesn't follow a redirect — read the address it moved to`; `0`: the address it moved to was never asked |
| 3 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/pilot.html","opts":{"max":500}}'` | Exit 1: `Error: the answer is over 500 bytes: Echo stopped reading it` |
| 4 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/missing.html"}'` | Exit 1: `Error: the page answered 404` |
| 5 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48222/pilot.html"}'` | Exit 1: `Error: web-demo may reach only the hosts its manifest names () and the addresses you list in its settings (http://127.0.0.1:48221)` |
| 6 | `$E plugin call web-demo read '{"url":"file:///etc/hosts"}'` | Exit 1, `may reach only`: no file of this computer is read |

## Expected Final State
Unchanged: Web demo installed, the made-up site running.

## Teardown
- TC-190's teardown (in S-44: the last stage's, below).

## Notes
- Written from the feature's text (F-PLUG-110). A compressed answer, an answer over 5 MB stopped as it came (a 50 MB one never read whole), and the time limit (`timeout`): the Rust test `jshost::web::tests::a_plugin_reads_a_page_it_may_reach_within_its_caps_and_saves_its_pictures`.
