---
id: TC-002
title: Meeting subjects, organizers, people and the prep show as text in the calendar, Coming up today and Today
priority: high
type: functional
module: hostile
requirements: [F-CAL-04, F-CAL-07, F-PREP-01, F-PREP-02, F-PREP-04, F-TODAY-02]
tags: [hostile, calendar, prep, xss]
run: agent
platform: any
---

# TC-002: Meeting subjects, organizers, people and the prep show as text in the calendar, Coming up today and Today

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The data was built less than an hour before the run, so `ev-hostile` is still to come and `ev-hostile-md` is past (times are relative to the build)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-48 | meeting `ev-hostile` subject "Meeting HOSTILE-48 <img …>" (ends today, later than now) |
| HOSTILE-49 | its organizer (right-to-left, override character) |
| HOSTILE-50 | one of its people (emoji, Cyrillic) |
| HOSTILE-51 | another of its people (10 000-character word) |
| HOSTILE-52 | meeting `ev-hostile-md` (earlier today) subject "HOSTILE-52 # Heading [click me](https://evil.example/x) …" |
| HOSTILE-54 | prep of the meeting "Umbrella security Q&A" (`ev-m11`): a "New since then" title |
| HOSTILE-55 | same prep: the sender (right-to-left) |
| HOSTILE-56 | same prep: the snippet (Markdown) |
| HOSTILE-57 | same prep: the summary ("In short") |
| HOSTILE-58 | same prep: a person (10 000 characters) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calendar/day` and wait for the text "Day" | The Day view of today opens |
| 3 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-(48\|52)/.test(e.textContent)).map((e) => e.textContent.slice(0, 50))` | Both subjects are in the list: one starts "Meeting HOSTILE-48 <img src=x onerror=alert(1)>", the other "HOSTILE-52 # Heading" (blocks may truncate with "…") |
| 4 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 5 | Click the block `role=button[name=/HOSTILE-48/]` | The side card opens: its subject contains `<img src=x onerror=alert(1)>` as text and its organizer line contains "HOSTILE-49" |
| 6 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('HOSTILE-49'))` | Returns `true` |
| 7 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 8 | Click `role=button[name="Week"]` | The Week view shows the same two blocks; the card stays open |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Navigate to `{{base_url}}/#calls`, click the item "Coming up today" (`[data-up-fold]`) to open it | The fold lists the coming meetings including "Meeting HOSTILE-48" with the markup as text |
| 11 | Click that meeting (`[data-up]` containing "HOSTILE-48") | The meeting opens like a call: a header with the subject as text and its people "HOSTILE-49", "HOSTILE-50" and a name starting "HOSTILE-51 WWWW" |
| 12 | Run `browser_evaluate` with `() => { const m = document.querySelector('main'); const b = m.getBoundingClientRect(); return { right: b.right, inside: [...m.querySelectorAll('*')].filter((e) => /HOSTILE-51/.test(e.textContent) && e.children.length === 0).every((e) => e.getBoundingClientRect().right <= b.right + 1) } }` | `inside` is `true`: the 10 000-character name stays inside the main column |
| 13 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 14 | Navigate to `{{base_url}}/#calendar/day`, click the block of "Umbrella security Q&A" and, in its card or in the call view it opens, open the Prep (button "Open prep" or the "Prep" tab) | The prep shows "In short" with HOSTILE-57 as text, and under "New since then" the HOSTILE-54 title with `<img src=x onerror=alert(1)>` as text, the sender HOSTILE-55 and the quoted snippet HOSTILE-56 |
| 15 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 16 | Navigate to `{{base_url}}/#today` and wait for the text "Your day" | Your day lists "HOSTILE-52 # Heading" (past) and "Meeting HOSTILE-48" (coming) with the subjects as text; a link-looking `[click me](https://evil.example/x)` is not a clickable link |
| 17 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 18 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
Calendar blocks and cards, Coming up today, the meeting's header and people, the prep and Your day showed the meeting words as text; no dialog appeared; the long name stayed in its column. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- The prep markers (54-58) belong to the prep of `ev-m11` (Umbrella security Q&A), not to the hostile meeting. How the prep is reached may vary: the case says where to look; if the prep tab or button is missing for that meeting, report the step BLOCKED with the reason.
- HOSTILE-52's `https://evil.example/x` target must not be a link in Your day: `a[href^="https://evil.example"]` count 0 in the Today view.
