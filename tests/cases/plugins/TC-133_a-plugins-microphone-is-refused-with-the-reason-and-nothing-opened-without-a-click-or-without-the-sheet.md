---
id: TC-133
title: A plugin's microphone is refused with the reason, and no device asked for, without the user's click and for a plugin whose sheet didn't ask for it
priority: high
type: functional
requirements: [F-PLUG-87]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, client-half, microphone, privacy]
---

# TC-133: A plugin's microphone is refused without a click or without the sheet

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-130`'s preconditions (the served data, the Bash prelude with `D`, the browser's stand-in microphone on this tab, the marketplace `acme-local`), then its step 1 (Acme voice and Acme plain installed, both on)
- `S` as in `plugins/TC-130`'s Test Data

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#acme-voice:talk`; wait 6 s; then reload it with nothing touching the page for 5 s, in one `browser_run_code_unsafe`: `async (page) => { await page.reload(); await page.waitForTimeout(5000) }`; then read `document.body.dataset.acmeLoad`, `document.body.dataset.plainLoad` and `S`; look at the top bar | Both `refused: the microphone opens only on your click` (each asked as it loaded, with no click); `gum` `0`: the page never asked the browser for a device; `pill` `null`, the top bar shows Record |
| 2 | Navigate to `{{base_url}}/#acme-plain:talk`; click its "Talk"; wait 2 s; read `S` | `plain` is `refused: Acme plain didn't ask for the microphone when it was installed`; `gum` `0`; `pill` `null` |

## Expected Final State
Acme voice and Acme plain installed and on; no plugin hears the microphone.

## Teardown
- `plugins/TC-130`'s teardown, without its "Hang up"

## Notes
- Written from the feature's text (F-PLUG-87), not from the code.
- **Why step 1 reloads that way:** Chromium counts Playwright's own evaluations (`browser_evaluate`, `browser_snapshot`, a locator's wait, `page.evaluate`) as the user's activation for about 5 s, across a reload of the same page. A read or a snapshot just before or while the plugins load lets their no-click ask through (`opened`), which is the tool, not Echo. So nothing touches the page from 6 s before the reload to 5 s after it; a real click (`browser_click`) is what the other steps need anyway.
- Not here: a refusal while Echo records a call or a memo needs the app recording (the served window has no recorder) — the Rust test `window::plugins` (`a_plugin_gets_the_microphone_only_as_…_never_while_echo_records`) and the node test `plugin-mic.test.mjs` hold it, and `plugins/TC-131` checks it in the native window by hand; "another plugin has the microphone now" can't be reached in the window (leaving one plugin's view stops its microphone first) and is the Rust test's; a click outside the plugin's own view is the node test's.
