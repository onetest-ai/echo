---
id: TC-132
title: A plugin's microphone stops, and Echo's sign goes, on the pill's Stop, the plugin's own close, its tracks ending, leaving its view and the plugin switched off
priority: high
type: functional
requirements: [F-PLUG-86]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, client-half, microphone, privacy]
---

# TC-132: A plugin's microphone stops on Stop, its close, its tracks ending, leaving its view, switched off

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-130`'s preconditions (the served data, the Bash prelude with `D`, the browser's stand-in microphone on this tab, the marketplace `acme-local`), then its step 1 (Acme voice and Acme plain installed)
- `S` as in `plugins/TC-130`'s Test Data

**Changes:** `<config dir>/plugins/` (Acme voice switched off)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#acme-voice:talk`; click "Talk"; wait 2 s (the pill "Acme voice hears your microphone" shows); click the pill's "Stop"; wait 1 s; read `S` | `track` `ended`, `pill` `null`; the top bar shows Record again |
| 2 | Click "Talk"; wait 2 s (the pill is back); click "Hang up"; wait 1 s; read `S` | `mic` is `closed`, `track` `ended`, `pill` `null` |
| 3 | Click "Talk"; wait 2 s (the pill is back); `browser_evaluate` `window.__acmeTrack.stop()`; wait 4 s; read `S` | `track` `ended`, `pill` `null`: every track ended, so the microphone closed |
| 4 | Click "Talk"; wait 2 s (the pill is back); navigate to `{{base_url}}/#today` (a hash change, no reload); wait 1 s; read `S` | `track` `ended`, `pill` `null`: leaving its view stopped it |
| 5 | Navigate to `{{base_url}}/#acme-voice:talk`; click "Talk"; wait 2 s (the pill is back); then `echoctl plugin disable acme-voice`; wait 6 s; read `S` and `location.hash` | `track` `ended`, `pill` `null`: the plugin switched off took its microphone away; its page is gone (`location.hash` is a Calls address) |

## Expected Final State
Acme voice installed and off, Acme plain installed and on; no plugin hears the microphone; the top bar shows Record.

## Teardown
- `plugins/TC-130`'s teardown, without its "Hang up"

## Notes
- Written from the feature's text (F-PLUG-86), not from the code.
- The page loading again closing the microphone and a plugin updated or removed are the node tests' (`plugin-mic.test.mjs`, `plugin-client-ctx.test.mjs`); after any of these the native webview asking Echo again is `plugins/TC-131`'s step 3.
