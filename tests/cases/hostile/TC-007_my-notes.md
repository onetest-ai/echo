---
id: TC-007
title: My notes, the raw my-notes.md and the notes under the minutes lines show as text
priority: high
type: functional
module: hostile
requirements: [F-NOTES-01, F-NOTES-05, F-NOTES-08]
tags: [hostile, notes, xss]
run: agent
platform: any
---

# TC-007: My notes, the raw my-notes.md and the notes under the minutes lines show as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- Close the right panel before step 2 and keep the sidebar folded
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-80 | heading line of `my-notes.md` (markup) |
| HOSTILE-81 | note at 00:05 (Markdown) |
| HOSTILE-82 | quoted words under that note (right-to-left) |
| HOSTILE-83 | question at 00:13 (10 000 characters) |
| HOSTILE-84 | to-do at 00:21 (emoji, Cyrillic) |

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
| 3 | Click the toggle `[aria-label="My notes"]` | It is pressed; the panel opens with a box `[aria-label="New note"]` |
| 4 | Run `browser_evaluate` with `() => document.querySelector('[aria-label="Right panel"]').innerText.match(/HOSTILE-\d+/g)` | Contains HOSTILE-81, HOSTILE-82, HOSTILE-83 and HOSTILE-84 (the heading HOSTILE-80 is in the file's text, step 6) |
| 5 | Run `browser_evaluate` with `() => ({ rtlDir: getComputedStyle(document.querySelector('[aria-label="Right panel"]')).direction, panel: document.querySelector('[aria-label="Right panel"]').getBoundingClientRect().right, scrollW: document.querySelector('[aria-label="Right panel"]').scrollWidth, clientW: document.querySelector('[aria-label="Right panel"]').clientWidth })` | `rtlDir` is `ltr`; `scrollW` <= `clientW` + 1: the 10 000-character question does not widen the panel |
| 6 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 7 | Click `[aria-label="Edit as text"]` | A text area shows the raw file; its value starts with `# HOSTILE-80 <img src=x onerror=alert(1)>` |
| 8 | Run `browser_evaluate` with `() => document.querySelector('[aria-label="Right panel"] textarea').value.includes('<script>alert(1)</script>')` | Returns `true` (the file is shown as typed, in a text area, not parsed) |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Press Escape or click `[aria-label="Edit as text"]` again without changing the text | The timeline comes back; no "Saved" change: the file is untouched |
| 11 | Close the panel (`[aria-label="Close the panel"]`), wait for `iframe#minutes-frame`, then run `browser_run_code_unsafe` with `async (page) => { const f = page.frameLocator('#minutes-frame'); await f.locator('.mynote').first().waitFor(); return { boxes: await f.locator('.mynote').allInnerTexts(), imgs: await f.locator('.mynote img, .mynote script, .mynote a').count() } }` | At least one `.mynote` callout under the Summary line with the time chip 00:05, containing `HOSTILE-81` and the text `[click me](https://evil.example/x)` as text; `imgs` 0 (no element made from the note text) |
| 12 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 13 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
My notes showed the hostile notes in the panel, as raw text and as callouts in the minutes, all as text; the file was not changed. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Make sure My notes is closed (`[aria-label="My notes"]` not pressed); navigate to `{{base_url}}/#today`. The file `my-notes.md` was never edited

## Notes
- Do not type in the New note box: a note typed is saved to the call.
