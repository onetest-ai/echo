---
id: TC-004
title: Meeting chat messages, senders and file names show as text
priority: high
type: functional
module: hostile
requirements: [F-CALLS-14]
tags: [hostile, chat, xss]
run: agent
platform: any
---

# TC-004: Meeting chat messages, senders and file names show as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything
- **In S-27** the guard (Checks) is a shared asset: the scenario's prelude arms it once and it stays armed for every stage (it survives navigations and reloads: the prelude checks that), so step 1 is skipped there; check D empties what it read, so each stage sees only its own dialogs and page errors. The console part of the last step is read once per group of stages (the scenario says where), not in every stage. On its own, step 1 arms the guard and the last step reads the console
- **In S-27** the hostile call is already open, left by the stage before (`hostile/TC-005` step 2 opens it, and the stages after it stay on it), so step 2 is skipped there. On its own, step 2 opens it

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-85 | sender 1 (markup) |
| HOSTILE-86 | message 1 (markup) |
| HOSTILE-87 | file name 1 (markup) |
| HOSTILE-88 | sender 2, 300 characters |
| HOSTILE-89 | message 2, 10 000 characters |
| HOSTILE-90 | file name 2, 300 characters |
| HOSTILE-91 | sender 3 (right-to-left) |
| HOSTILE-92 | message 3 |
| HOSTILE-93 | file name 3 |
| HOSTILE-94 | sender 4 (emoji, Cyrillic) |
| HOSTILE-95 | message 4 |
| HOSTILE-96 | file name 4 |
| HOSTILE-97 | sender 5 (Markdown) |
| HOSTILE-98 | message 5 |
| HOSTILE-99 | file name 5 |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => { const r = { armed: page.__hostileOn === true, dialogs: [...(page.__dlg || [])], pageErrors: [...(page.__perr || [])] }; if (page.__dlg) { page.__dlg.length = 0; page.__perr.length = 0 } return r }`. Expected: `armed` `true`, `dialogs` and `pageErrors` both `[]`. It empties both lists after reading them, so the next check D sees only what came after
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calls`, wait for the text "Hostile call HOSTILE-61", click the row `[data-call*="HOSTILE-61"]` | The URL hash starts with `#call/<the build date − 1>_090000-`; the header shows a title containing "HOSTILE-61" |
| 3 | Click `role=tab[name=/Chat/]` | The URL hash ends with `/chat`; the tab label shows the count 5 |
| 4 | Run `browser_evaluate` with `() => document.querySelector('main').innerText.match(/HOSTILE-\d+/g)` | The list contains each of HOSTILE-85 to HOSTILE-99 at least once (senders, messages and file names; a file name may be shown cut with "…") |
| 5 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: message 1 or its sender shows `<img src=x onerror=alert(1)>` as text |
| 6 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 7 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main a[href]')].map((a) => a.getAttribute('href'))` | No `href` of a chat message, sender or file starts with `javascript:`; the text of the Markdown messages (HOSTILE-97, 98, 99) shows `[click me](https://evil.example/x)` as text, not a heading, a link or an image |
| 8 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return [...document.querySelectorAll('main *')].filter((e) => /HOSTILE-(88\|89\|90)/.test(e.textContent) && e.children.length === 0).map((e) => e.getBoundingClientRect().right <= m.right + 1) }` | Every entry is `true`: the long sender, message and file name stay inside the main column |
| 9 | Run `browser_evaluate` with `() => ({ imgs: document.querySelectorAll('main img').length, bigHeadings: document.querySelectorAll('main h1').length })` | `imgs` 0 (the `![img](…)` of HOSTILE-97 to 99 loaded nothing); the Markdown `# Heading` made no extra `h1` beyond the call's own title (`bigHeadings` <= 1) |
| 10 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The Chat tab listed five hostile messages with their senders and files as text; the long ones stayed in the column; no image or link came from the data. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing
