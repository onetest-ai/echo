---
id: TC-015
title: The window follows the system's light and dark appearance
priority: low
type: functional
requirements: [F-SET-04]
module: window
run: agent
platform: any
tags: [appearance]
external_id: TC-WIN-010
---

# TC-015: The window follows the system's light and dark appearance

**Module:** Window | **Priority:** Low | **Type:** Functional

## Preconditions
- The window served on test data (`tests/fixtures/serve.sh up`), the page loaded from `{{base_url}}/`; Settings → General → Appearance on "System" (the default)
- The system's appearance is emulated in the browser (`browser_emulate_media` / `page.emulateMedia({colorScheme})`): never System Settings

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Emulate `colorScheme: light`, open `{{base_url}}/#settings/general` | Appearance's "System" (`[data-theme-set=""]`) is pressed; `document.documentElement.dataset.theme` is unset; the body is light (`rgb(246, 246, 248)`), text dark |
| 2 | Emulate `colorScheme: dark` (no reload) | Within a second the body turns dark (`rgb(20, 20, 24)`), headings light (`rgb(236, 236, 241)`); all text readable (screenshot) |
| 3 | Emulate `colorScheme: light` again | Back to light |

## Expected Final State
With "System", the window follows the system appearance both ways, live.

## Notes
- The native window is a WebKit view that hands the page the system's `prefers-color-scheme` (`app.css`, `@media (prefers-color-scheme: dark)`; `ui/app/*.js` redraws graphs on its change). That the webview passes a real macOS switch through stays a person's look once.
- Run on 2026-11-07 (S-21, served window): PASS.
