---
id: TC-005
title: The minutes frame shows hostile minutes as text, with only https links and no script of the data
priority: high
type: functional
module: hostile
requirements: [F-MIN-01, F-MIN-03, F-NOTES-08]
tags: [hostile, minutes, xss]
run: agent
platform: any
---

# TC-005: The minutes frame shows hostile minutes as text, with only https links and no script of the data

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- Close the right panel and keep the sidebar folded (`app_profile.md` → Fragile areas): the minutes frame is 1 px wide otherwise
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-73 | front matter `title:` of `notes.md` (markup) |
| HOSTILE-74 | the `#` heading (markup) |
| HOSTILE-75 | a Summary line with Markdown link, image and autolink, and a time chip [00:05] |
| HOSTILE-76 | a Summary line with a 10 000-character word |
| HOSTILE-77 | table cell (right-to-left) |
| HOSTILE-78 | table cell (emoji, Cyrillic) |
| HOSTILE-79 | a ```mermaid block whose label holds markup |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => ({ dialogs: page.__dlg, pageErrors: page.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calls`, wait for the text "Hostile call HOSTILE-61", click the row `[data-call*="HOSTILE-61"]` | The URL hash starts with `#call/2026-11-05_090000-`; the header shows a title containing "HOSTILE-61" |
| 3 | Wait for `iframe#minutes-frame`; run `browser_run_code_unsafe` with `async (page) => { const f = page.frameLocator('#minutes-frame'); await f.locator('h1').first().waitFor(); return { h1: await f.locator('h1').first().innerText(), meta: await f.locator('.meta').innerText() } }` | `h1` starts with `HOSTILE-74` and contains `<img src=x onerror=alert(1)>` as text; `meta` (Title) contains `HOSTILE-73` and `<script>alert(1)</script>` as text |
| 4 | Run `browser_run_code_unsafe` with `async (page) => { const f = page.frameLocator('#minutes-frame'); return await f.locator('body').evaluate(() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick]').length, imgs: document.querySelectorAll('img').length, dataScripts: [...document.querySelectorAll('script')].filter((s) => /alert\(/.test(s.textContent)).length, badLinks: [...document.querySelectorAll('a[href]')].filter((a) => !/^https:\/\//.test(a.getAttribute('href'))).length, httpsLinks: [...document.querySelectorAll('a[href]')].map((a) => a.getAttribute('href') + ' rel=' + a.rel), iframes: document.querySelectorAll('iframe,object,embed,form').length })) }` | `handlers` 0, `imgs` 0, `dataScripts` 0, `badLinks` 0, `iframes` 0; `httpsLinks` holds only `https://evil.example…` entries and each has `noopener` in its rel |
| 5 | Run `browser_run_code_unsafe` with `async (page) => { const f = page.frameLocator('#minutes-frame'); return { text: await f.locator('main').innerText() } }` | `text` contains `javascript:alert(1)` and `<iframe src="javascript:alert(3)"></iframe>` and `<script>alert('minutes')</script>` as text, and the words "a link" and "another" without a link around them |
| 6 | Run `browser_run_code_unsafe` with `async (page) => { const f = page.frameLocator('#minutes-frame'); return { cells: await f.locator('td').allInnerTexts(), diagram: await f.locator('figure.diagram').innerText().catch(() => 'none') } }` | `cells` contain `HOSTILE-77` and `HOSTILE-78`; `diagram` contains `HOSTILE-79` (as the label text, or as source text if mermaid shows its syntax-error box); no dialog |
| 7 | Run `browser_run_code_unsafe` with `async (page) => { const f = page.frameLocator('#minutes-frame'); const m = await f.locator('main').boundingBox(); const w = await f.locator('body').evaluate(() => ({ sw: document.documentElement.scrollWidth, iw: window.innerWidth })); const li = await f.locator('li', { hasText: 'HOSTILE-76' }).first().boundingBox(); return { w, liRight: li.x + li.width, mainRight: m.x + m.width } }` | `w.sw` <= `w.iw` and `liRight` <= `mainRight` + 1: the 10 000-character line wraps inside the frame |
| 8 | Click the chip `iframe#minutes-frame >> internal:control=enter-frame >> .ts >> nth=0` | The Transcript tab opens (hash ends `/transcript`) and one turn gets the class `hit`; no dialog |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Go back to the Minutes tab (`role=tab[name="Minutes"]`) | The frame shows the minutes again; the main page still passes check A |
| 11 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The minutes frame displayed the hostile minutes as text; links were https only; the markup and the script of the data were inert. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- Evaluate in the cross-origin frame works through `frameLocator` in `browser_run_code_unsafe`; `browser_evaluate` cannot read it.
- The frame's own scripts (mermaid, the click handler) are hashed in its CSP; only scripts whose text contains `alert(` would be the data's.
