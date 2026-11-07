---
id: TC-014
title: Vocabulary terms and the meetings and people of learned terms show as text
priority: high
type: functional
module: hostile
requirements: [F-VOC-01, F-VOC-02, F-VOC-03]
tags: [hostile, vocabulary, xss]
run: agent
platform: any
---

# TC-014: Vocabulary terms and the meetings and people of learned terms show as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-01 | an "every call" term (emoji, Cyrillic) in `config.json` `vocabulary` |
| HOSTILE-02 | a learned term (markup) |
| HOSTILE-03 | a learned term's meeting subject (right-to-left) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calls` and click the book button (Vocabulary, `[data-panel-show="vocabulary"]`, or the button named "Vocabulary") | The right panel shows the Vocabulary tab with "Every call" and "Learned from calls" |
| 3 | Run `browser_evaluate` with `() => { const t = document.querySelector('[aria-label="Right panel"]').innerText; return t.includes('HOSTILE-01') && t.includes('Привіт') }` | Returns `true`: the "every call" term chip shows its emoji and Cyrillic text |
| 4 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 5 | Run `browser_evaluate` with `() => { const t = document.querySelector('[aria-label="Right panel"]').innerText; return t.includes('HOSTILE-02') && t.includes('<img src=x onerror=alert(1)>') }` | Returns `true`: the learned term shows the markup as text |
| 6 | Run `browser_evaluate` with `() => { const t = document.querySelector('[aria-label="Right panel"]').innerText; return t.includes('HOSTILE-03') }` | Returns `true`: the meeting subject of the learned term (right-to-left) is shown |
| 7 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 8 | Run `browser_evaluate` with `() => { const p = document.querySelector('[aria-label="Right panel"]'); return { fits: p.scrollWidth <= p.clientWidth + 1, dir: getComputedStyle(p).direction } }` | `fits` is `true` and `dir` is `ltr` |
| 9 | Open `{{base_url}}/#settings/transcription` and then `{{base_url}}/#settings/recording`, running check A on each | The sections open; check A holds on both (vocabulary hints may be shown there) |
| 10 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 11 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The vocabulary panel showed terms, meetings and people with hostile words as text; nothing ran. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Close the right panel. Do not press Add, Always or the × of a term: the case only reads
