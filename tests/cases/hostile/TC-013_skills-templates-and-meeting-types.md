---
id: TC-013
title: Skill, template and meeting type words show as text in Settings → Templates
priority: high
type: functional
module: hostile
requirements: [F-TPL-01, F-TPL-02, F-TPL-03, F-TPL-05, F-SKILL-01, F-SKILL-02]
tags: [hostile, templates, skills, xss]
run: agent
platform: any
---

# TC-013: Skill, template and meeting type words show as text in Settings → Templates

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-14 | skill `hostile-skill` description (markup) |
| HOSTILE-15 | its SKILL.md text (Markdown) |
| HOSTILE-16 | its SKILL.md: a 10 000-character word |
| HOSTILE-17 | template `hostile-template` header (markup) |
| HOSTILE-18 | its section heading (right-to-left) |
| HOSTILE-19 | meeting type `hostile-type` name (emoji, Cyrillic) |
| HOSTILE-20 | its "how to tell it" (markup) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { globalThis.__dlg ??= []; globalThis.__perr ??= []; if (!globalThis.__hostileOn) { globalThis.__hostileOn = true; page.on('dialog', async (d) => { globalThis.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss() }); page.on('pageerror', (e) => globalThis.__perr.push(String(e))) } globalThis.__dlg.length = 0; globalThis.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async () => ({ dialogs: globalThis.__dlg, pageErrors: globalThis.__perr })`. Expected: `dialogs` and `pageErrors` both `[]`
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#settings/templates` and wait for the text "Meeting types" | The tabs Minutes, Meeting types, Skills show with counts |
| 3 | Click `[data-tpl-tab="skills"]` | The list has a skill named `hostile-skill` marked "Yours" with its description starting HOSTILE-14 |
| 4 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the description shows `<img src=x onerror=alert(1)>` as text |
| 5 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 6 | Click the row `hostile-skill` | The skill opens: SKILL.md is shown as written, with the files of its folder |
| 7 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return t.includes('HOSTILE-15') && t.includes('HOSTILE-16') && t.includes('<script>alert(1)</script>') }` | Returns `true`: the SKILL.md text is shown as text |
| 8 | Run `browser_evaluate` with `() => { const m = document.querySelector('main').getBoundingClientRect(); return { sw: document.documentElement.scrollWidth <= window.innerWidth, inside: [...document.querySelectorAll('main *')].filter((e) => e.children.length === 0 && /HOSTILE-16/.test(e.textContent)).every((e) => e.getBoundingClientRect().right <= m.right + 1 \|\| getComputedStyle(e).overflowX !== 'visible' \|\| getComputedStyle(e.parentElement).overflowX !== 'visible') } }` | `sw` is `true` and `inside` is `true`: the long word does not widen the page (it wraps, or sits in a box that scrolls) |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Click `[data-tpl-tab="minutes"]`, then the row of the template `hostile-template` | The editor opens: the header card and the sections; do not type |
| 11 | Run `browser_evaluate` with `() => { const v = [...document.querySelectorAll('main textarea, main input')].map((x) => x.value).join(' '); const t = document.querySelector('main').innerText; return { header: (v + t).includes('HOSTILE-17'), section: (v + t).includes('HOSTILE-18'), lit: (v + t).includes('<img src=x onerror=alert(1)>') } }` | All three `true` |
| 12 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 13 | Click `[data-tpl-tab="types"]` (the tab Meeting types), read the list | A meeting type "HOSTILE-19 🎉🔥…" marked Yours; its "how to tell it" starts HOSTILE-20 |
| 14 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return t.includes('HOSTILE-19') && t.includes('HOSTILE-20') && t.includes('<img src=x onerror=alert(1)>') }` | Returns `true` |
| 15 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 16 | Open a call, for example `{{base_url}}/#calls`, click `[data-call$="acme-weekly-sync"]`, and run `browser_evaluate` with `() => [...document.querySelectorAll('select[aria-label="Kind of meeting"] option')].map((o) => o.textContent.slice(0, 30))` | The kind chooser lists the meeting types; the hostile one appears as an option with its text (not as markup) |
| 17 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 18 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
Skills, templates and meeting types with hostile words showed as text in their lists and editors. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Leave the template editor without typing (navigate to `{{base_url}}/#today`). The editor saves each change at once, so no key may be pressed in its fields

## Notes
- The kind-of-meeting option list may not include user-made types; if the hostile type is absent there, step 17's expected result is only check A (no markup), and the absence is noted.
