---
id: TC-042
title: Screenshots go under an "Appendix: On screen" heading too, each to the file its bullet names, else to the nearest one not named
priority: medium
type: functional
requirements: [F-MIN-24]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, frames]
---

# TC-042: Screenshots go under an "Appendix: On screen" heading too, each to the file its bullet names, else to the nearest one not named

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (for Acme weekly sync the first match is today's call)

## Test Data

| Acme weekly sync (today) | Value |
|---|---|
| Frames | `01m05s.png` (3 px wide) at 01:05, `05m07s.png` (5 px wide) at 05:07 |
| Its minutes' last section | `## Appendix: On screen` with `- [01:00] The rollout timeline (05m07s.png)` and `- [01:10] The agenda slide` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click its row, then the tab "Minutes" | `iframe#minutes-frame` (accessibility snapshot) has the heading "Appendix: On screen" and two images, one under each of its bullets |
| 2 | Run `browser_evaluate` with `async () => { const d = await (await fetch('/api/calls')).json(); return '/minutes/' + encodeURIComponent(d.calls.map((c) => c.name).filter((n) => n.endsWith('-acme-weekly-sync')).sort().pop()) }` and navigate to `{{base_url}}<that path>` | The minutes page opens on its own |
| 3 | Run `browser_evaluate` with `() => ({ all: document.querySelectorAll('img').length, items: [...document.querySelectorAll('li')].filter((li) => /rollout timeline\|agenda slide/.test(li.textContent)).map((li) => [li.textContent.includes('rollout') ? 'rollout' : 'agenda', (li.querySelector('img') \|\| {}).naturalWidth]) })` | `all` is 2; `items` is `[["rollout", 5], ["agenda", 3]]`: the named `05m07s.png` under "The rollout timeline" although `01m05s.png` is nearer to 01:00, and `01m05s.png`, the nearest not named, under "The agenda slide" |

## Expected Final State
Nothing changed (read only).
