---
id: TC-001
title: With nothing installed Plugins opens on Discover and Installed says so
priority: medium
type: functional
requirements: [F-PLUG-01]
module: plugins
run: agent
platform: any
tags: [plugins]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-001: With nothing installed Plugins opens on Discover and Installed says so

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins` | The heading "Plugins" shows with the tabs "Installed", "Discover" and "Marketplaces" with the count `1`; "Discover" is selected |
| 2 | Click the tab "Installed" | It is selected and the page says "No plugins yet — Discover lists what your marketplaces offer." |
| 3 | Read the tab "Installed" | It has no count |

## Expected Final State
No plugin is installed; the Installed tab says so.
