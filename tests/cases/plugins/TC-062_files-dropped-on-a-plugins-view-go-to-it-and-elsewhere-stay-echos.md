---
id: TC-062
title: Files dropped on the Echo window while a plugin's view that listens is shown go to that plugin as handles; dropped on a thread's logo tile they stay Echo's own
priority: medium
type: functional
requirements: [F-PLUG-46]
module: plugins
run: person
platform: macos
data: standard
tags: [plugins, client-half, files, drop]
---

# TC-062: A drop goes to the plugin whose view is shown, else it is Echo's

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- The person's own Echo dev build (`cargo build`, then `target/debug/echoctl watch` on made-up data — the native window: a drop of files from Finder can't be driven in a served browser)
- TC-061's Acme files, with its window half adding to `apply`: `ctx.onFileDrop(async (files) => { const r = await ctx.call('take', {}, { files }); document.body.dataset.acmeDropped = JSON.stringify(r) })`
- A made-up file `Kickoff.md` in Finder

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open Acme files' activity; drag `Kickoff.md` from Finder onto the page and drop it | `$D/vault/sources/Kickoff.md` is there, the same bytes; `document.body.dataset.acmeDropped` names it and its size, no path |
| 2 | Open a thread's Brief; drop a PNG on its logo tile | The logo is set as before (Echo's own drop); nothing new in `$D/vault/sources/` |

## Expected Final State
Only `Kickoff.md` was added by a drop.

## Teardown
- As TC-061's
