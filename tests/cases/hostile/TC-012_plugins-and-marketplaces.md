---
id: TC-012
title: Plugin names, summaries, categories and settings show as text in Settings → Plugins
priority: high
type: functional
module: hostile
requirements: [F-PLUG-01, F-PLUG-02, F-PLUG-05, F-PLUG-07]
tags: [hostile, plugins, xss]
run: agent
platform: any
auto: [tests/auto/specs/S-27.spec.mjs]
---

# TC-012: Plugin names, summaries, categories and settings show as text in Settings → Plugins

**Module:** Hostile content | **Priority:** High | **Type:** Functional (security)

## Preconditions
- Data folder built with `tests/fixtures/serve.sh up <dir> <port> --set hostile` (the `hostile` data set), window served at `{{base_url}}`; every `HOSTILE-NN` piece of this case exists in it (`<dir>/manifest.json` → `hostile`)
- Browser at the default viewport 1201x868; the page is the one already open (do not close it or open another tab)
- The pieces are others' words: the browser must show them as text. Nothing here may run, and nothing may be pressed that starts Claude Code or sends anything
- **In S-27** the guard (Checks) is a shared asset: the scenario's prelude arms it once and it stays armed for every stage (it survives navigations and reloads: the prelude checks that), so step 1 is skipped there; check D empties what it read, so each stage sees only its own dialogs and page errors. The console part of the last step is read once per group of stages (the scenario says where), not in every stage. On its own, step 1 arms the guard and the last step reads the console

## Test Data

| Piece | Where it lives |
|---|---|
| HOSTILE-08 | plugin `standup-notes` name (markup), from marketplace acme-tools |
| HOSTILE-09 | its summary (right-to-left) |
| HOSTILE-10 | a settings label (emoji) |
| HOSTILE-11 | a category (markup) |
| HOSTILE-12 | a text setting's value (Markdown), set with `echoctl plugin set standup-notes label` |
| HOSTILE-13 | the plugin's Inbox item (see TC-011) |

## Checks (run exactly as written)

- **Guard** (`browser_run_code_unsafe`): `async (page) => { page.__dlg ??= []; page.__perr ??= []; if (!page.__hostileOn) { page.__hostileOn = true; page.on('dialog', async (d) => { page.__dlg.push(d.type() + ': ' + d.message()); await d.dismiss().catch(() => {}) }); page.on('pageerror', (e) => page.__perr.push(String(e))) } page.__dlg.length = 0; page.__perr.length = 0; return 'armed' }`
- **Check A** (`browser_evaluate`, run after every view below): `() => ({ handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length, inlineScripts: document.querySelectorAll('script:not([src])').length, badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length, brokenImgs: document.querySelectorAll('img[src="x"]').length, scrollWidth: document.documentElement.scrollWidth, innerWidth: window.innerWidth })`. Expected: `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth`
- **Check D** (`browser_run_code_unsafe`): `async (page) => { const r = { armed: page.__hostileOn === true, dialogs: [...(page.__dlg || [])], pageErrors: [...(page.__perr || [])] }; if (page.__dlg) { page.__dlg.length = 0; page.__perr.length = 0 } return r }`. Expected: `armed` `true`, `dialogs` and `pageErrors` both `[]`. It empties both lists after reading them, so the next check D sees only what came after
- The literal text of a piece: the piece's visible text contains the characters, for example `<img src=x onerror=alert(1)>` (a truncated copy with its marker "…" also counts; a broken image or an empty element does not)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/` and wait for the text "Your day". Run `browser_run_code_unsafe` with the guard code (see Checks): it records every dialog and page error from now on | It returns `armed` |
| 2 | Navigate to `{{base_url}}/#settings/plugins` and wait for the tab `role=tab[name="Installed"]` | The plugin tabs show |
| 3 | Click `role=tab[name="Installed"]` | The installed list has a card named "Standup HOSTILE-08 <img src=x onerror=alert(1)>…" with its summary starting HOSTILE-09 |
| 4 | Run `browser_evaluate` with `() => [...document.querySelectorAll('main')].some((e) => e.innerText.includes('<img src=x onerror=alert(1)>'))` | Returns `true`: the plugin's name is shown as text |
| 5 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 6 | Navigate to `{{base_url}}/#settings/plugins/standup-notes` | The plugin's detail opens: where it came from, the enable switch, its settings |
| 7 | Run `browser_evaluate` with `() => { const t = document.querySelector('main').innerText; return t.includes('HOSTILE-10') && t.includes('<img src=x onerror=alert(1)>') }` | Returns `true`: the settings label (HOSTILE-10) and the plugin's name are shown as text (the text value HOSTILE-12 sits in a field, so it is not in `innerText`: step 8 reads it) |
| 8 | Run `browser_evaluate` with `() => { const i = [...document.querySelectorAll('main input[type=text], main input:not([type]), main textarea')].find((x) => x.value.includes('HOSTILE-12')); return i ? { value: i.value.slice(0, 80), attrs: i.outerHTML.slice(0, 160) } : 'no field' }` | `value` starts with `HOSTILE-12` and contains the Markdown as the field's value, `attrs` shows no `onerror`/`onload` attribute (or the value is shown as text: allowed) |
| 9 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 10 | Navigate to `{{base_url}}/#settings/plugins`, click `role=tab[name="Discover"]` | Discover shows category chips and cards, including "Standup HOSTILE-08" |
| 11 | Run `browser_evaluate` with `() => ({ chips: [...document.querySelectorAll('main button, main [role=tab]')].filter((b) => b.innerText.includes('HOSTILE-11')).map((b) => b.innerText.slice(0, 40)), card: document.querySelector('main').innerText.includes('HOSTILE-09') })` | `chips` holds one chip with the text starting "Cat HOSTILE-11" (a category's chip shows its first letter capital) (or the category is only inside the card); `card` is `true` |
| 12 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 13 | Click the chip with HOSTILE-11, if there is one | The list is filtered to the plugin; no dialog; the view stays (the markup in the category did not become an element) |
| 14 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 15 | Click `role=tab[name=/Marketplaces/]` | The marketplaces: "onetest" (built in), "acme-tools" with its plugin count and "broken" with its read error ("it has no echo-marketplace.json…") |
| 16 | Run `browser_evaluate` with `() => document.querySelector('main').innerText.includes('echo-marketplace.json')` | Returns `true`: the error text of the marketplace is shown as text |
| 17 | Run check A (`browser_evaluate`) | `handlers` 0, `inlineScripts` 0, `badUrls` 0, `brokenImgs` 0 and `scrollWidth` <= `innerWidth` |
| 18 | Run `browser_run_code_unsafe` with check D, then `browser_console_messages` (level `error`) | check D returns `dialogs: []` and `pageErrors: []`; the console lists no message except the `favicon.ico` 404, and none that starts with "Refused to" or mentions "Content Security Policy" (a blocked inline handler means markup reached the DOM: that is a FAIL) |

## Expected Final State
Settings → Plugins showed the hostile plugin's name, summary, category and settings as text on Installed, its detail, Discover and Marketplaces. No dialog appeared, check A held on every view, and the console shows only the `favicon.ico` 404.

## Teardown
- Navigate to `{{base_url}}/#today`. Nothing was installed, switched or removed

## Notes
- Marketplace names and titles, and a plugin's Today cards and slot labels, are not hostile in the data set (the plugins use the markers on the plugin's own fields only). Report this in the run's notes; do not treat it as a failure.
- Do not press Install, Update, Remove, Read again, Add or the enable switch.
