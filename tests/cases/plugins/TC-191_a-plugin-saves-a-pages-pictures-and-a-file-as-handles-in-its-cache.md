---
id: TC-191
title: A plugin saves a page's pictures as file handles in its store's cache, each picture saved or told why not, and fetches one file as a handle
priority: medium
type: functional
requirements: [F-PLUG-111]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, web, files, cli]
---

# TC-191: A plugin saves a page's pictures and a file as handles

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-190's preconditions, run as written (in S-44: kept from stage 1), and its step 3's address listed: `$E plugin set web-demo site "http://127.0.0.1:48221"`; every Bash call with the prelude `. <dir>/env; E=target/debug/echoctl`

**Changes:** Web demo's store (`$ECHOCTL_HOME/.echo/plugins/web-demo/cache/web/`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call web-demo read '{"url":"http://127.0.0.1:48221/pilot.html","opts":{"images":"save"}}'`; read its `images` | Two: `img/waves.png` with `saved` `{"file": <32 hex>, "name": "01.png", "size": 6302, "type": "image/png"}` and its caption kept; `pilot.html` with no `saved` and `error` `this is text/html, not a picture Echo saves (PNG, JPEG, GIF, WebP, AVIF, SVG)` |
| 2 | `find "$ECHOCTL_HOME/.echo/plugins/web-demo/cache/web" -type f` | One file, `…/cache/web/<16 hex>/01.png` (6302 bytes: `wc -c`): nothing half-written, the refused one not kept |
| 3 | Step 1's `markdown` | The same as TC-190's step 4: the picture still `![Stores by wave](http://127.0.0.1:48221/img/waves.png)` (the plugin maps `src` to the saved name itself) |
| 4 | `$E plugin call web-demo fetch '{"url":"http://127.0.0.1:48221/plan.pdf"}'` | `{"file": <32 hex>, "name": "plan.pdf", "size": 34, "type": "application/pdf"}` |
| 5 | `$E plugin call web-demo fetch '{"url":"http://127.0.0.1:48222/plan.pdf"}'` | Exit 1: `web-demo may reach only … (http://127.0.0.1:48221)`: a file only where the plugin may reach |

## Expected Final State
Web demo's cache holds the saved picture and the fetched file.

## Teardown
- Kept for the next stage in S-44. On its own: TC-190's teardown.

## Notes
- Written from the feature's text (F-PLUG-111). A picture of a public page from its own public host, one on this computer refused for such a page, the 50-picture and 10 MB caps: the Rust test `jshost::web::tests::a_plugin_reads_a_page_it_may_reach_within_its_caps_and_saves_its_pictures` (a development build's fake hosts stand for public ones); which addresses count as public: `jshost::web::tests::only_public_addresses_are_public`.
