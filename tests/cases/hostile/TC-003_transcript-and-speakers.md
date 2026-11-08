---
id: TC-003
title: Transcript turns and speaker names show as text and keep their rows
priority: high
type: functional
module: hostile
requirements: [F-TR-01, F-TR-02, F-TR-03]
tags: [hostile, transcript, xss]
run: agent
platform: any
---

# TC-003: Transcript turns and speaker names show as text and keep their rows

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
| HOSTILE-62 | speaker name (markup) of turn 1 |
| HOSTILE-63 | turn 1 text: `<img …> <script>…</script> <a href="javascript:…">` |
| HOSTILE-64 | speaker name, 10 000 characters |
| HOSTILE-65 | turn 2 text, 10 000 characters |
| HOSTILE-66 | speaker name, right-to-left |
| HOSTILE-67 | turn 3 text, right-to-left with an override character |
| HOSTILE-68 | speaker name, emoji and Cyrillic |
| HOSTILE-69 | turn 4 text, emoji and Cyrillic |
| HOSTILE-70 | speaker name, Markdown |
| HOSTILE-71 | turn 5 text, Markdown |
| HOSTILE-72 | turn 6 text (the user, "Me"), Markdown |

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
| 3 | Click `role=tab[name="Transcript"]` | The URL hash ends with `/transcript`; turns `.turn[data-t]` are listed |
| 4 | Run `browser_evaluate` with `() => [...document.querySelectorAll('.turn')].map((t) => (t.querySelector('.who') \|\| {}).innerText + ' \| ' + t.innerText.slice(0, 40))` | Six turns; the speaker names contain, in order, `HOSTILE-62`, `HOSTILE-64`, `HOSTILE-66`, `HOSTILE-68`, `HOSTILE-70`; texts begin `HOSTILE-63`, `HOSTILE-65`, `HOSTILE-67`, `HOSTILE-69`, `HOSTILE-71`, `HOSTILE-72` |
| 5 | Run `browser_evaluate` with `() => [...document.querySelectorAll('.turn')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: turn 1 (and the speaker name HOSTILE-62) show `<img src=x onerror=alert(1)>` as text |
| 6 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 7 | Run `browser_evaluate` with `() => [...document.querySelectorAll('.turn a[href]')].map((a) => a.getAttribute('href'))` | Returns `[]` or only `https://` links: no link from the data is `javascript:`; the text "[open](javascript:alert(1))" is visible as text in turn 1 |
| 8 | Run `browser_evaluate` with `() => { const c = document.querySelector('.turn').parentElement.getBoundingClientRect(); return [...document.querySelectorAll('.turn')].map((t) => ({ right: t.getBoundingClientRect().right <= c.right + 1, w: t.scrollWidth <= t.clientWidth + 1 })) }` | Every entry has `right: true` and `w: true`: the 10 000-character speaker name and text do not widen any turn |
| 9 | Run `browser_evaluate` with `() => { const t = [...document.querySelectorAll('.turn')].find((x) => x.innerText.includes('HOSTILE-67')); const who = t.querySelector('.who').getBoundingClientRect(); const p = t.querySelector('p').getBoundingClientRect(); return { dir: getComputedStyle(t).direction, whoLeftOfText: who.left <= p.left } }` | `dir` is `ltr` and `whoLeftOfText` is `true`: the right-to-left turn does not flip its row |
| 10 | Fill `[aria-label="Find in transcript"]` with `HOSTILE-63`, wait 1 second | Exactly one `.turn` has the class `hit` (turn 1); its text is still the literal markup |
| 11 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 12 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
The transcript showed six turns with hostile speakers and texts as text, in their normal rows; Find highlighted the turn without turning text into markup. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Clear the Find box; navigate to `{{base_url}}/#today`

## Notes
- Quotes of these turns in What moved / history belong to the work ledger; the data set has none for this call, so those places are not exercised here (see TC-008).
