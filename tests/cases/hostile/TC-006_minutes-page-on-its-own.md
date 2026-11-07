---
id: TC-006
title: The minutes page opened on its own (the "Open minutes in browser" page) shows hostile minutes as text
priority: high
type: functional
module: hostile
requirements: [F-MIN-01, F-CALLS-17]
tags: [hostile, minutes, xss]
run: agent
platform: any
---

# TC-006: The minutes page opened on its own (the "Open minutes in browser" page) shows hostile minutes as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The call folder, found with `browser_evaluate` on `{{base_url}}/api/calls` (`calls[].name` containing `HOSTILE-61`), is called `<folder>` below; the page is `{{base_url}}/minutes/<folder encoded with encodeURIComponent>`
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-73 | front matter title, also the page `<title>` |
| HOSTILE-74 | heading |
| HOSTILE-75 | Summary line (Markdown link and image) |
| HOSTILE-77 | table cell |
| HOSTILE-78 | table cell |
| HOSTILE-79 | mermaid block |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Run `browser_evaluate` with `async () => { const d = await (await fetch('/api/calls')).json(); const c = d.calls.find((x) => x.name.includes('HOSTILE-61')); return '/minutes/' + encodeURIComponent(c.name) }` | Returns a path starting `/minutes/2026-11-05_090000-%3Cimg%20src%3Dx%20onerror%3Dalert%281%29%3E-HOSTILE-61` |
| 3 | Navigate to `{{base_url}}` + that path | The page loads (HTTP 200) with a heading starting "HOSTILE-74" |
| 4 | Run `browser_evaluate` with `() => ({ title: document.title, h1: document.querySelector('h1').innerText, handlers: document.querySelectorAll('[onerror],[onload],[onclick]').length, imgs: document.querySelectorAll('img').length, badLinks: [...document.querySelectorAll('a[href]')].filter((a) => !/^https:\/\//.test(a.getAttribute('href'))).length, dataScripts: [...document.querySelectorAll('script')].filter((s) => /alert\(/.test(s.textContent)).length, forms: document.querySelectorAll('form,iframe,object,embed').length, sw: document.documentElement.scrollWidth, iw: window.innerWidth })` | `title` contains `<img src=x onerror=alert(1)>` as text; `h1` contains it too; `handlers` 0, `imgs` 0, `badLinks` 0, `dataScripts` 0, `forms` 0; `sw` <= `iw` |
| 5 | Run `browser_evaluate` with `() => document.querySelector('main').innerText` | Contains `javascript:alert(1)`, `<iframe src="javascript:alert(3)"></iframe>` and `<script>alert('minutes')</script>` as text; contains `HOSTILE-77`, `HOSTILE-78` and `HOSTILE-79` |
| 6 | Run `browser_evaluate` with `async () => (await fetch(location.href)).headers.get('content-security-policy')` | The policy starts `default-src 'none'`, contains `form-action 'none'` and `base-uri 'none'`, and its `script-src` lists only `'sha256-…'` hashes (no `'unsafe-inline'` in script-src) |
| 7 | Navigate to `{{base_url}}/#today` and wait for the text "Your day" | Echo's window loads again (the page left and returned) |
| 8 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The standalone minutes page showed the hostile minutes as text under a CSP that allows only its own hashed scripts. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- This is the window server's `/minutes/<folder>` route, the same page the call menu's "Open minutes in browser" writes to `<title>-minutes.html`; the menu item itself opens the system browser and is not pressed.
