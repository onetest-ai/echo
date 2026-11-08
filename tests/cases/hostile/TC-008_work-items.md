---
id: TC-008
title: Work items show as text on the tree, the board, Tasks and in Details
priority: high
type: functional
module: hostile
requirements: [F-WORK-04, F-WORK-07, F-WORK-09, F-WORK-14, F-TASK-06, F-TASK-07]
tags: [hostile, work, tasks, xss]
run: agent
platform: any
---

# TC-008: Work items show as text on the tree, the board, Tasks and in Details

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-31 | task T026 title (markup) in Acme rollout |
| HOSTILE-32 | T026 description, Markdown (rendered by the server) |
| HOSTILE-33 | T026 description, markup |
| HOSTILE-34 | T026 link title (right-to-left), URL `https://acme.example/x` |
| HOSTILE-35 | T026 waiting for: who (emoji, Cyrillic) |
| HOSTILE-36 | T026 waiting for: what (10 000 characters) |
| HOSTILE-37 | task T027 title, 10 000 characters |
| HOSTILE-38 | T027 internal owner (markup) |
| HOSTILE-39 | question Q004 title (right-to-left) |
| HOSTILE-40 | Q004 description (markup) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => ({ dialogs: page.__dlg, pageErrors: page.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click `role=button[name="Tree"]` | The tree lists the story rows and tasks; a row with `data-node="T026"` and one with `data-node="T027"` exist |
| 3 | Run `browser_evaluate` with `() => [...document.querySelectorAll('[data-node="T026"]')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: T026's row shows `<img src=x onerror=alert(1)>` as text |
| 4 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 5 | Run `browser_evaluate` with `() => { const r = document.querySelector('[data-node="T027"]'); const m = document.querySelector('main').getBoundingClientRect(); return { right: r.getBoundingClientRect().right <= m.right + 1, text: r.innerText.includes('HOSTILE-37') } }` | `right` and `text` are both `true`: the 10 000-character title is cut or wrapped inside the tree |
| 6 | Run `browser_evaluate` with `() => [...document.querySelectorAll('[data-node]')].filter((e) => e.innerText.includes('HOSTILE-39')).map((e) => e.innerText.slice(0, 60))` | One entry: the question Q004 "Question HOSTILE-39 …" with the right-to-left text in its row |
| 7 | Click `role=button[name="Board"]` | The Board opens; cards `[data-pj="node"]` for T026 and T027 are in their columns (T026 may sit in Waiting) |
| 8 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 9 | Click the card `[data-pj="node"][data-node="T026"]` | The right panel opens on Details: the title contains `<img src=x onerror=alert(1)>` as text |
| 10 | Run `browser_evaluate` with `() => { const p = document.querySelector('[aria-label="Right panel"]'); return { desc: p.innerText.includes('HOSTILE-33'), md: p.innerText.includes('HOSTILE-32'), wait: p.innerText.includes('HOSTILE-35'), link: [...p.querySelectorAll('a')].map((a) => a.getAttribute('href') + ' \| ' + a.innerText.slice(0, 30)), badLinks: [...p.querySelectorAll('a[href]')].filter((a) => !/^(https\|echo):\/\//i.test(a.getAttribute('href'))).length, scrollW: p.scrollWidth <= p.clientWidth + 1 } }` | `desc`, `md`, `wait` are `true`; `link` has an entry with `https://acme.example/x` and the text starting `↗ link HOSTILE-34`; the description's `[click me](https://evil.example/x)` appears as the words "click me" in a link to `https://evil.example/x` or as plain text (either is allowed), never as a heading; `badLinks` 0; `scrollW` is `true` |
| 11 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 12 | Run `browser_evaluate` with `() => document.querySelector('[aria-label="Right panel"]').innerText.includes('<script>alert(1)</script>')` | Returns `true`: the description's script tag is shown as text |
| 13 | Click the card `[data-pj="node"][data-node="T027"]` | Details shows the 10 000-character title; the panel passes check A |
| 14 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 15 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | The piles show; the card of T026 (HOSTILE-31) is in a pile with the markup as text |
| 16 | Run `browser_evaluate` with `() => ({ cards: [...document.querySelectorAll('[data-tk="open"]')].filter((c) => /HOSTILE-3[1-8]/.test(c.innerText)).length, overflow: [...document.querySelectorAll('[data-tk="open"]')].filter((c) => c.scrollWidth > c.clientWidth + 1).length })` | `cards` >= 1 and `overflow` 0: no card is wider than itself |
| 17 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 18 | Click the folded pile `[data-tk="more"][data-pile="others"]` if it shows, then run check A | Cards of other people (T027, owner HOSTILE-38) show the owner as text; check A holds |
| 19 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The tree, board, Tasks and Details showed the hostile work items as text; the long titles stayed inside their cards. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Close the Details panel (`[aria-label="Close the panel"]`); navigate to `{{base_url}}/#today`. Nothing was edited

## Notes
- Descriptions are rendered by the server (`description_html`, safe writers): Markdown links to `https://` may show as links; `javascript:` ones must not.
- Do not press Send to Claude, Edit or Status: the case only reads.
