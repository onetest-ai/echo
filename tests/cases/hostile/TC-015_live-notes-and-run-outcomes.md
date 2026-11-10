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
auto: [tests/auto/specs/S-27.spec.mjs]
---

# TC-015: Live notes, answers and a failed minutes run's message show as text

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
| HOSTILE-100 | live notes "now" (markup) |
| HOSTILE-101 | a live decision (Markdown) |
| HOSTILE-102 | a live action's owner (right-to-left) |
| HOSTILE-103 | a live action, 10 000 characters |
| HOSTILE-104 | a live answer to a question (markup) |
| HOSTILE-114 | message of the failed minutes run (markup) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => { const r = { armed: page.__hostileOn === true, dialogs: [...(page.__dlg || [])], pageErrors: [...(page.__perr || [])] }; if (page.__dlg) { page.__dlg.length = 0; page.__perr.length = 0 } return r }`. Expected: `armed` `true`, `dialogs` and `pageErrors` both `[]`. It empties both lists after reading them, so the next check D sees only what came after
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#calls`, wait for the text "Hostile call HOSTILE-61", click the row `[data-call*="HOSTILE-61"]` | The URL hash starts with `#call/<the build date − 1>_090000-` (the hostile call is yesterday at 09:00); the header shows a title containing "HOSTILE-61" |
| 3 | Click `role=tab[name="Transcript"]`, then open My notes (`[data-panel-flip="notes"]`) unless it is open | The Transcript tab shows the turns alone; My notes (`#r-right`) shows the live notes: "Now" in `section[aria-label="Live notes"]` and ✦ rows among the user's notes, answers under "Asked about this call" |
| 4 | Run `browser_evaluate` with `() => { const t = document.querySelector('#r-right').innerText; return [100, 101, 102, 103, 104].map((n) => t.includes('HOSTILE-' + n)) }` | All five `true` (the answer is under "Asked about this call") |
| 5 | Run `browser_evaluate` with `() => document.querySelector('#r-right').innerText.includes('<img src=x onerror=alert(1)>')` | Returns `true`: markup of the live notes shows as text |
| 6 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 7 | Run `browser_evaluate` with `() => { const m = document.querySelector('#r-right').getBoundingClientRect(); return [...document.querySelectorAll('#r-right *')].filter((e) => e.children.length === 0 && /HOSTILE-103/.test(e.textContent)).map((e) => e.getBoundingClientRect().right <= m.right + 1) }` | Every entry `true`: the 10 000-character action stays in the panel |
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
