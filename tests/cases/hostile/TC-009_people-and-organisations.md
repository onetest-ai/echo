---
id: TC-009
title: People and organisations show as text on the map, org chart, list and person panel
priority: high
type: functional
module: hostile
requirements: [F-PPL-01, F-PPL-04, F-PPL-05, F-CLIENT-08]
tags: [hostile, people, xss]
run: agent
platform: any
---

# TC-009: People and organisations show as text on the map, org chart, list and person panel

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-41 | person name (markup) in Acme rollout |
| HOSTILE-42 | their title (right-to-left) |
| HOSTILE-43 | "who they are" (Markdown) |
| HOSTILE-44 | group (emoji, Cyrillic) |
| HOSTILE-45 | office, 10 000 characters |
| HOSTILE-46 | a fact (markup) |
| HOSTILE-47 | the fact's quote (Markdown) |
| HOSTILE-29 | person in the hostile thread (markup name) with address hostile-thread@umbrella.example (its own: HOSTILE-41 has hostile@umbrella.example) |
| HOSTILE-30 | their role (right-to-left) |
| HOSTILE-04 | organisation name (markup), not a client; set with `echoctl org add` |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => ({ dialogs: page.__dlg, pageErrors: page.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#project/acme-rollout/people` and wait for the text "Map" | The People tab opens with the buttons Map, Org chart, List (`[data-pp="view"]`) |
| 3 | Click `[data-pp="view"][data-v="list"]` | The List shows people rows; one starts with `HOSTILE-41` and shows `<img src=x onerror=alert(1)>` as text |
| 4 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return t.includes('<img src=x onerror=alert(1)>') && t.includes('HOSTILE-42') && t.includes('HOSTILE-44') }` | Returns `true`: name, title and group are all in the list as text |
| 5 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 6 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-45/.test(e.textContent)).map((e) => e.getBoundingClientRect().right <= m.right + 1) }` | Empty list or only `true`: the 10 000-character office never leaves the main column (it may be hidden in the list and only shown in the panel) |
| 7 | Click the row of `HOSTILE-41` | The person's panel opens (right panel or inline) with the name as text, "who they are" with HOSTILE-43, the group HOSTILE-44, the office HOSTILE-45, a fact with HOSTILE-46 and its quote HOSTILE-47 |
| 8 | Run `browser_evaluate` with `() => { const t = document.body.innerText; return { facts: t.includes('HOSTILE-46') && t.includes('HOSTILE-47'), lit: t.includes('<script>alert(1)</script>'), badLinks: [...document.querySelectorAll('a[href]')].filter((a) => /^\s*javascript:/i.test(a.getAttribute('href'))).length, imgs: document.querySelectorAll('main img[src^="http"]').length } }` | `facts` and `lit` are `true`; `badLinks` 0; `imgs` 0 (the `![img](https://evil.example/i.png)` of the quote loaded nothing) |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Click `[data-pp="view"][data-v="map"]`, then `[data-pp="view"][data-v="org"]` | Both views draw without an error text (they draw the thread's members; HOSTILE-41 is "Seen in calls", not a member, so it isn't in them — its name is checked in the list, step 3) |
| 11 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 12 | Navigate to `{{base_url}}/#project/thread-hostile-21-img-src-x-onerror-alert-1-script-alert-1-s/people`, `[data-pp="view"][data-v="list"]` | The thread's list has a person "Person HOSTILE-29 <img src=x onerror=alert(1)>…" with the role text starting HOSTILE-30 (right-to-left) |
| 13 | Run `browser_evaluate` with `() => { const r = [...document.querySelectorAll('main *')].find((e) => e.children.length === 0 && e.textContent.includes('HOSTILE-30')); return { found: !!r, dir: r ? getComputedStyle(r.closest('main')).direction : null } }` | `found` is `true` and `dir` is `ltr` |
| 14 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 15 | Navigate to `{{base_url}}/#client/umbrella/people` | The client's People tab lists people of the threads, including names that come from calls (HOSTILE-62, 66, 70, 85, 91, 94, 97, 108, 110, 111) and "HOSTILE-109 WWWW…" and "HOSTILE-88 WWWW…"; each as text |
| 16 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return { sw: document.documentElement.scrollWidth <= window.innerWidth, inside: [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-(88\|109)/.test(e.textContent)).every((e) => e.getBoundingClientRect().right <= m.right + 1) } }` | `sw` and `inside` are `true` |
| 17 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 18 | Open a thread's Profile (`{{base_url}}/#project/thread-hostile-21-img-src-x-onerror-alert-1-script-alert-1-s/profile`) and run `browser_evaluate` with `() => [...document.querySelectorAll('option, [data-pj-org]')].some((o) => o.textContent.includes('HOSTILE-04'))` | Returns `true`: the organisation "Org HOSTILE-04 <img …>" is offered as text (an `option` or a chip) with no image made from it |
| 19 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 20 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
People and organisations from the hostile data showed as text on the list, the map, the org chart and the person's panel. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was changed, so no data needs undoing

## Notes
- If HOSTILE-04 is not offered on the Profile of a thread (organisations are only offered after "Client"/"Organisations" is opened), open that chooser without picking anything; if it is nowhere in the window, report the step as failed with where the organisation does show.
- Do not press Yes/No on a suggestion, Same person as, or + Person.
