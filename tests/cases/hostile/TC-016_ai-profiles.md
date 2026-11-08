---
id: TC-016
title: An AI profile named with markup shows as text in Settings, the account chip and the model pickers
priority: high
type: functional
module: hostile
requirements: [F-AI-01, F-AI-02]
tags: [hostile, ai, xss]
run: agent
platform: any
---

# TC-016: An AI profile named with markup shows as text in Settings, the account chip and the model pickers

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-07 | AI profile `ollama` name: "Qwen HOSTILE-07 <img …>" (a model on a host), in `ai.json` |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => ({ dialogs: page.__dlg, pageErrors: page.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#settings/ai` and wait for the heading "AI & minutes" (not for a profile's name: the account chip's hidden options in the top bar hold the names too) | AI & minutes lists the profiles, among them one named "Qwen HOSTILE-07" with the kind "Model · this computer" (the data adds it without `--url`) |
| 3 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the profile name shows `<img src=x onerror=alert(1)>` as text |
| 4 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 5 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-07/.test(e.textContent)).every((e) => e.getBoundingClientRect().right <= m.right + 1) }` | Returns `true` |
| 6 | Navigate to `{{base_url}}/#calls` and run `browser_evaluate` with `() => [...document.querySelectorAll('#r-top select[aria-label="AI profile"] option')].map((o) => o.textContent.slice(0, 60))` | The account chip's list holds the profile names; one begins "Qwen HOSTILE-07 <img src=x onerror=alert(1)>" as text (do not choose it, do not press Record) |
| 7 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 8 | Open the hostile call (`[data-call*="HOSTILE-61"]`), and run `browser_evaluate` with `() => [...document.querySelectorAll('select[data-callprofile] option, select[data-minprofile] option')].map((o) => o.textContent.slice(0, 40))` | The model pickers (if shown on this call) list the same names, the hostile one as text; an empty list is allowed when the call has no picker |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Click `[data-pal-open]`, fill `[aria-label="Action"]` with `HOSTILE-07` | The palette lists nothing for the profile or lists it as text; no element is made from the markup |
| 11 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 12 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The profile name with markup showed as text in Settings, the account chip's picker and the call's pickers. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Press Escape to close the palette. Do not press Make default, Sign in or Remove
