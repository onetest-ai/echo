---
id: TC-015
title: Live notes, answers and a failed minutes run's message show as text
priority: high
type: functional
module: hostile
requirements: [F-LIVE-03, F-MIN-09, F-LIVE-05]
tags: [hostile, live, minutes, xss]
run: agent
platform: any
---

# TC-015: Live notes, answers and a failed minutes run's message show as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-100 | live notes "now" (markup) |
| HOSTILE-101 | a live decision (Markdown) |
| HOSTILE-102 | a live action's owner (right-to-left) |
| HOSTILE-103 | a live action, 10 000 characters |
| HOSTILE-104 | a live answer to a question (markup) |
| HOSTILE-114 | message of the failed minutes run (markup) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calls`, wait for the text "Hostile call HOSTILE-61", click the row `[data-call*="HOSTILE-61"]` | The URL hash starts with `#call/2026-11-05_090000-`; the header shows a title containing "HOSTILE-61" |
| 3 | Click `role=tab[name="Transcript"]` | The Transcript tab shows the turns with the live notes beside them ("Live notes" part) |
| 4 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return [100, 101, 102, 103, 104].map((n) => t.includes('HOSTILE-' + n)) }` | All five `true` (an answer may be under "Questions"/"Answers" in the live part) |
| 5 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: markup of the live notes shows as text |
| 6 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 7 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-103/.test(e.textContent)).map((e) => e.getBoundingClientRect().right <= m.right + 1) }` | Every entry `true`: the 10 000-character action stays in the column |
| 8 | Click `role=tab[name="Minutes"]` | The Minutes tab shows the frame; above or beside it the status line of the last run reads "The last update didn't finish: HOSTILE-114 …" or the "Couldn't write the minutes" box with the message |
| 9 | Run `browser_evaluate` with `() => document.querySelector('main').innerText.includes('HOSTILE-114')` | Returns `true` |
| 10 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 11 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main button')].map((b) => b.innerText.trim()).filter(Boolean)` | The buttons are Echo's own ("Try again", "Show log", …); none is named by the data; do not press any |
| 12 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
Live notes, the answer and the failed run's message showed as text. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- Yolo outcomes and `claude` run results come from runs that need a signed-in Claude Code and are not in the data set; the failed minutes run (HOSTILE-114) stands for a run outcome shown in the window.
- The call's action items (`actions.json`: HOSTILE-105, 106, 107) did not show in any view of this call: see the report of the authoring run. If a later app version shows them (What moved, Today, Tasks), add a step.
