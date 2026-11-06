---
id: TC-011
title: Inbox items, Found for you and Needs you now show others' words as text
priority: high
type: functional
module: hostile
requirements: [F-INBOX-01, F-INBOX-03, F-INBOX-04, F-INBOX-05, F-THR-18, F-TODAY-04]
tags: [hostile, inbox, found, today, xss]
run: agent
platform: any
---

# TC-011: Inbox items, Found for you and Needs you now show others' words as text

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- Microsoft 365 is not signed in; the Needs you now items come from the data's `.echo/needs.json`
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-53 | Inbox: "Teams transcript: …" item title (markup) |
| HOSTILE-59 | Inbox: a sender waiting to be trusted (markup name) |
| HOSTILE-60 | that sender's reason (right-to-left) |
| HOSTILE-117 | Inbox: "New thread?" item and Found for you: candidate name, rule subject, its calls (markup) |
| HOSTILE-13 | Inbox: a plugin's own item "Review …" (markup) |
| HOSTILE-118 | Today, Needs you now: sender (markup) |
| HOSTILE-119 | where (right-to-left) |
| HOSTILE-120 | line (Markdown) |
| HOSTILE-121 | quote, 10 000 characters |
| HOSTILE-122 | a sender (emoji) |
| HOSTILE-123 | a line (markup) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#today` and wait for the text "Your day" | Today loads |
| 3 | Run `browser_evaluate` with `() => { const t = document.body.innerText; return [118, 119, 120, 121, 122, 123].map((n) => t.includes('HOSTILE-' + n)) }` | Every entry `true`, or `false` only for items folded behind a "Show more" control: open it (button in Needs you now) and run again; all six are `true` then |
| 4 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the markup is shown as text in Needs you now |
| 5 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 6 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-121/.test(e.textContent)).map((e) => e.getBoundingClientRect().right <= m.right + 1) }` | Every entry `true`: the 10 000-character quote stays in the column (the card may show a cut quote) |
| 7 | Click the Inbox button `[data-panel-flip="needs"]` and, if a chip "Open" exists, click it | The right panel shows the Inbox with grouped items |
| 8 | Run `browser_evaluate` with `() => { const t = document.querySelector('[aria-label="Right panel"]').innerText; return [53, 59, 60, 117, 13].map((n) => t.includes('HOSTILE-' + n)) }` | Every entry `true` (items past the first five may be behind a "Show N more" control: open it and run again) |
| 9 | Run `browser_evaluate` with `() => [...document.querySelectorAll('[aria-label="Right panel"]')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: Inbox titles show `<img src=x onerror=alert(1)>` as text |
| 10 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 11 | Run `browser_evaluate` with `() => { const p = document.querySelector('[aria-label="Right panel"]'); return { fits: p.scrollWidth <= p.clientWidth + 1, buttons: [...p.querySelectorAll('button')].filter((b) => /HOSTILE/.test(b.getAttribute('title') \|\| '') \|\| /<img\|<script/.test(b.getAttribute('data-ipc') \|\| '')).length } }` | `fits` is `true`; `buttons` 0 or a number of buttons whose attribute holds the text only as an escaped string: check A (next step) shows no handler |
| 12 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 13 | Click `[data-pj="found"]` after `{{base_url}}/#projects` (do not press Create or Not this) | "Found for you" lists candidate threads; one is named "Found HOSTILE-117 <img …>" with its rule and calls as text |
| 14 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true` |
| 15 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 16 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
Inbox, Found for you and Needs you now showed the hostile items' words as text without running or breaking anything; no decision button was pressed. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Close the right panel; navigate to `{{base_url}}/#today`. Do not press Trust, Ignore, Import, Create or Not this: the case only reads

## Notes
- Inbox actions carry the item's data in attributes (`ipc` with a name containing markup for the Found item): they must be escaped. Check A catches an attribute that broke out of its quotes (an `onerror` or `onclick` on any element).
- Teams threads and mail "Read and reply" (the dialog behind a Needs you now item) need sign-in and are not opened.
