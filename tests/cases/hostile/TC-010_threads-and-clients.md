---
id: TC-010
title: Thread and client names, briefs, About, notes, rules, sources and aliases show as text
priority: high
type: functional
module: hostile
requirements: [F-THR-01, F-THR-08, F-THR-10, F-THR-14, F-THR-17, F-CLIENT-02, F-CLIENT-05]
tags: [hostile, threads, clients, xss]
run: agent
platform: any
---

# TC-010: Thread and client names, briefs, About, notes, rules, sources and aliases show as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-21 | thread name (markup); its id is `thread-hostile-21-img-src-x-onerror-alert-1-script-alert-1-s` |
| HOSTILE-22 | its Notes for Claude (Markdown) |
| HOSTILE-23 | brief paragraph 1 (markup, rendered by the server) |
| HOSTILE-24 | brief paragraph 2, 10 000 characters |
| HOSTILE-25 | brief Markdown (link, image, heading) |
| HOSTILE-26 | calls rule "Subject contains …" (right-to-left) |
| HOSTILE-27 | source: a Teams chat named with markup |
| HOSTILE-28 | Acme's other name (emoji) |
| HOSTILE-05 | Globex About (Markdown) |
| HOSTILE-06 | Globex About (markup) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#projects` and wait for the Threads list | The list shows the client group Umbrella with a thread row `[data-pj="open"][data-id^="thread-hostile-21"]` |
| 3 | Run `browser_evaluate` with `() => [...document.querySelectorAll('[data-pj="open"][data-id^="thread-hostile-21"]')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the row shows `<img src=x onerror=alert(1)>` as text |
| 4 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 5 | Click `[data-pj="open"][data-id^="thread-hostile-21"]`, then `[data-pj="tab"][data-tab-id="brief"]` | The thread opens on Brief; its header shows the name as text |
| 6 | Run `browser_evaluate` with `() => { const d = document.querySelector('.me-doc'); const m = document.querySelector('main').getBoundingClientRect(); return { lit: d.innerText.includes('<img src=x onerror=alert(1)>'), mdLink: d.innerText.includes('click me') \|\| d.innerText.includes('HOSTILE-25'), h: d.querySelectorAll('h1,h2,h3,img,script,iframe').length, badLinks: [...d.querySelectorAll('a[href]')].filter((a) => !/^https:\/\//.test(a.getAttribute('href'))).length, fits: d.scrollWidth <= d.clientWidth + 1 && d.getBoundingClientRect().right <= m.right + 1 } }` | `lit` is `true`; `mdLink` `true`; `h` 0 (the brief's `# Heading` and `![img]` made no heading or image); `badLinks` 0; `fits` `true`: the 10 000-character paragraph is wrapped inside the brief |
| 7 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 8 | Click `[data-pj="tab"][data-tab-id="profile"]` | Profile opens with rows Stage, Client, Calls, Sources, Notes for Claude |
| 9 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return t.includes('HOSTILE-26') && t.includes('HOSTILE-27') && t.includes('HOSTILE-22') }` | Returns `true`: the rule "Subject contains “HOSTILE-26 …”", the source name with HOSTILE-27 and the notes with HOSTILE-22 are all shown |
| 10 | Run `browser_evaluate` with `() => ({ lit: document.querySelector('main').innerText.includes('<script>alert(1)</script>'), sw: document.documentElement.scrollWidth <= window.innerWidth })` | `lit` and `sw` are `true` |
| 11 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 12 | Click the thread's "Thread actions" menu `[aria-label="Thread actions"]`, then Escape | The menu opens and closes; its "Open Umbrella" label is a normal client name |
| 13 | Navigate to `{{base_url}}/#client/globex/overview` | The page shows "About Globex" with the About text |
| 14 | Run `browser_evaluate` with `() => { const d = document.querySelector('.me-doc'); return { text: d.innerText.includes('HOSTILE-05') && d.innerText.includes('HOSTILE-06'), lit: d.innerText.includes('<img src=x onerror=alert(1)>'), bad: d.querySelectorAll('img,script,iframe,h1,h2').length } }` | `text` and `lit` are `true`; `bad` is 0 |
| 15 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 16 | Navigate to `{{base_url}}/#client/acme/profile` | Profile of Acme shows "Other names" as chips |
| 17 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return t.includes('HOSTILE-28') }` | Returns `true`: the emoji alias is a chip with its text |
| 18 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 19 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The thread's name, brief, rule, source and notes, the Globex About and Acme's alias showed as text; no heading, image or script came out of them. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- Briefs and About are rendered by the server's safe writer; the window puts that HTML in `.me-doc` (app.js `p.brief_html`, `cd.brief_html`). A `javascript:` link in them must have become plain text.
- Do not press Edit, Sync now or Delete.
