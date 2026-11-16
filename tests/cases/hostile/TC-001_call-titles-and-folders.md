---
id: TC-001
title: Call titles and folder names show as text in the list, header, palette and thread and client call lists
priority: high
type: functional
module: hostile
requirements: [F-CALLS-01, F-CALLS-03, F-CALLS-05, F-THR-11, F-CLIENT-03, F-WS-03]
tags: [hostile, calls, xss]
run: agent
platform: any
auto: [tests/auto/specs/S-27.spec.mjs]
---

# TC-001: Call titles and folder names show as text in the list, header, palette and thread and client call lists

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything
- **In S-27** the guard (Checks) is a shared asset: the scenario's prelude arms it once and it stays armed for every stage (it survives navigations and reloads: the prelude checks that), so step 1 is skipped there; check D empties what it read, so each stage sees only its own dialogs and page errors. The console part of the last step is read once per group of stages (the scenario says where), not in every stage. On its own, step 1 arms the guard and the last step reads the console

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-61 | call title AND folder name `<the build date − 1>_090000-<img src=x onerror=alert(1)>-HOSTILE-61-שלום-Привіт` (the `data-call` attribute carries the folder) |
| HOSTILE-115 | call `<date>_080000-hostile-title-long`: 10 000-character word after the marker |
| HOSTILE-116 | call `<date>_080000-hostile-title-rtl`: right-to-left, an override character, combining marks |
| HOSTILE-117 | two calls `…-hostile-found-3` and `…-hostile-found-5`, title "Found HOSTILE-117 <img …>" |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => { const r = { armed: page.__hostileOn === true, dialogs: [...(page.__dlg || [])], pageErrors: [...(page.__perr || [])] }; if (page.__dlg) { page.__dlg.length = 0; page.__perr.length = 0 } return r }`. Expected: `armed` `true`, `dialogs` and `pageErrors` both `[]`. It empties both lists after reading them, so the next check D sees only what came after
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calls` and wait for the text "Hostile call HOSTILE-61" | The list shows a row whose `data-call` contains `HOSTILE-61` |
| 3 | Run `browser_evaluate` with `() => [...document.querySelectorAll('[data-call*="HOSTILE-61"]')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the row shows `<img src=x onerror=alert(1)>` as text |
| 4 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 5 | Run `browser_evaluate` with `() => { const r = document.querySelector('[data-call*="hostile-title-long"]'); const l = document.querySelector('[role=listbox][aria-label="Calls"]'); return { rowRight: r.getBoundingClientRect().right, listRight: l.getBoundingClientRect().right, text: r.innerText.slice(0, 60) } }` | `text` contains `HOSTILE-115`; `rowRight` <= `listRight` + 1 (the 10 000-character title stays inside the list column) |
| 6 | Run `browser_evaluate` with `() => { const r = document.querySelector('[data-call*="hostile-title-rtl"]'); return { dir: getComputedStyle(r).direction, fits: r.scrollWidth <= r.clientWidth + 1, text: r.innerText.includes('HOSTILE-116') } }` | `dir` is `ltr`, `fits` is `true`, `text` is `true`: the right-to-left title does not flip or widen its row |
| 7 | Fill `[aria-label="Search calls"]` with `HOSTILE-117` | Exactly two rows remain, both with the text "Found HOSTILE-117" followed by `<img src=x onerror=alert(1)>` |
| 8 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 9 | Clear the search box, then click the row `[data-call*="HOSTILE-61"]` | The URL hash starts with `#call/<the build date − 1>_090000-` (the hostile call is yesterday at 09:00) and its folder contains `HOSTILE-61`; `main` shows a header with the title |
| 10 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the header shows `<img src=x onerror=alert(1)>` as text |
| 11 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 12 | Click `[data-pal-open]`, fill `[aria-label="Action"]` with `HOSTILE-61` | The dialog `.pal` lists the call in its Calls group; its row text contains `<img src=x onerror=alert(1)>` |
| 13 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 14 | Press Escape | The palette closes |
| 15 | Navigate to `{{base_url}}/#project/thread-hostile-21-img-src-x-onerror-alert-1-script-alert-1-s/calls` | The thread's Calls tab lists three calls: the rows for HOSTILE-61, HOSTILE-115 and HOSTILE-116, each with its title as text |
| 16 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 17 | Navigate to `{{base_url}}/#client/umbrella/calls` | The client's Calls tab lists the same three calls (and others); the HOSTILE-61 row shows the markup as text |
| 18 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 19 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The three views and the palette showed the calls' titles as text; none ran code; the long title stayed inside its column. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- Today does not list these calls (they are dated before today: the data places them on the days before the build); Today's meetings are in TC-002.
- The folder name is in `data-call` and in the URL hash; a hash with `<` and `>` must not break the route (the call opens).
