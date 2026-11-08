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

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => ({ dialogs: page.__dlg, pageErrors: page.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Run `browser_evaluate` with `async () => { const d = await (await fetch('/api/calls')).json(); const c = d.calls.find((x) => x.name.includes('HOSTILE-61')); return '/minutes/' + encodeURIComponent(c.name) }` | Returns a path starting `/minutes/<the build date − 1>_090000-%3Cimg%20src%3Dx%20onerror%3Dalert(1)%3E-HOSTILE-61` (the hostile call is yesterday at 09:00; `encodeURIComponent` leaves `(` and `)` as they are) |
| 3 | Navigate to `{{base_url}}` + that path | The page loads (HTTP 200) with a heading starting "HOSTILE-74" |
| 4 | Run `browser_evaluate` with `() => ({ title: document.title, h1: document.querySelector('h1').innerText, handlers: document.querySelectorAll('[onerror],[onload],[onclick]').length, imgs: document.querySelectorAll('img:not(#lightbox img)').length, badLinks: [...document.querySelectorAll('a[href]')].filter((a) => !/^https:\/\//.test(a.getAttribute('href'))).length, dataScripts: [...document.querySelectorAll('script')].filter((s) => /alert\(/.test(s.textContent)).length, forms: document.querySelectorAll('form,iframe,object,embed').length, sw: document.documentElement.scrollWidth, iw: window.innerWidth })` | `title` contains `<img src=x onerror=alert(1)>` as text; `h1` contains it too; `handlers` 0, `imgs` 0 (the page's own empty `#lightbox` image is not counted), `badLinks` 0, `dataScripts` 0, `forms` 0; `sw` <= `iw` |
| 5 | Run `browser_evaluate` with `() => document.querySelector('main').innerText` | Contains `javascript:alert(1)`, `<iframe src="javascript:alert(3)"></iframe>` and `<script>alert('minutes')</script>` as text; contains `HOSTILE-77`, `HOSTILE-78` and `HOSTILE-79` |
| 6 | Run `browser_evaluate` with `() => document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.content` | The policy starts `default-src 'none'`, contains `form-action 'none'` and `base-uri 'none'`, and its `script-src` lists only `'sha256-…'` hashes (no `'unsafe-inline'` in script-src) |
| 7 | Navigate to `{{base_url}}/#today` and wait for the text "Your day" | Echo's window loads again (the page left and returned) |
| 8 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The standalone minutes page showed the hostile minutes as text under a CSP that allows only its own hashed scripts. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- Step 8 fails until the bug is filed and fixed: loading the page logs one "Executing inline event handler violates the following Content Security Policy directive 'script-src …'" (`script-src-attr`, from the bundled mermaid). The mermaid block HOSTILE-79 has a node label `<img src=x onerror=alert(1)> …`; mermaid puts it into the page as HTML for a moment and the CSP blocks its `onerror`. The final DOM holds no handler (step 4 `handlers` 0), and a plain diagram (`graph TD; A[Start] --> B[End]`) logs nothing, so the label's markup is what reaches the DOM.
- Step 6 reads the policy from the page's `<meta http-equiv="Content-Security-Policy">` tag (`minutes.rs`): it is in every copy of the page (the window's frame, the served page, the saved "Open minutes in browser" file), and the browser enforces it. A `fetch` of the page from inside it is blocked by that policy (`default-src 'none'`), and `window --serve` sends no CSP header (it forwards only Content-Type; the native webview gets the header from `page_reply`), so the header cannot be read here.
- Step 4's `sw` <= `iw` fails until the bug is filed and fixed: the 10 000-character word of HOSTILE-76 (a list item) is not wrapped and widens the page (`scrollWidth` 134 036 at 1201 px; `overflow-wrap: normal`).
- This is the window server's `/minutes/<folder>` route, the same page the call menu's "Open minutes in browser" writes to `<title>-minutes.html`; the menu item itself opens the system browser and is not pressed.
