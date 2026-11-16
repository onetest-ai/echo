---
id: TC-036
title: A plugin's widget provider is given the size the widget has on Today (S, M or L) and is asked again when it is resized
priority: medium
type: functional
requirements: [F-TODAY-19]
module: today
run: agent
platform: macos
data: standard
tags: [today, widgets, plugins, host-half]
---

# TC-036: A plugin's widget fits the size it was given

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; the browser 1440 × 900
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- A made-up local marketplace `acme-local` with **Acme fit**: one list widget, `fit`, at S, M or L, whose provider names the size it was given:

```bash
M="$D/market-fit"; mkdir -p "$M/plugins/acme-fit"
echo '{"name":"acme-local","plugins":[{"id":"acme-fit","source":"./plugins/acme-fit"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-fit","name":"Acme fit","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","widgets":[{"id":"fit","label":"Fit","kind":"list","sizes":["S","M","L"]}]}' > "$M/plugins/acme-fit/echo-plugin.json"
printf "export function apply(ctx) { ctx.widgets.provide('fit', ({ size }) => ({ items: [{ title: 'Given ' + size }] })) }\n" > "$M/plugins/acme-fit/main.js"
echoctl plugin marketplace add "$M" --name acme-local && echoctl plugin install acme-fit --yes
```

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), Today's layout

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl widget show plugin:acme-fit:fit` | `data.items[0].title` is `Given S`: off Today, the first size it takes |
| 2 | `echoctl today add plugin:acme-fit:fit --size M --above core:needs.now`; open `{{base_url}}/#today` (reload once if Fit hasn't answered yet) | The widget "Fit" shows `Given M` |
| 3 | `echoctl today size plugin:acme-fit:fit L`; reload `{{base_url}}/#today` (once more if it still says M) | Fit, now three columns wide, shows `Given L`: asked again for its new size, well before its refresh |
| 4 | Edit Today, set Fit's size to S, Done; reload once | Fit shows `Given S` |

## Expected Final State
Fit is on Today at S and its last answer was for S.

## Teardown
- `echoctl today reset`; `echoctl plugin remove acme-fit --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-TODAY-19), not from the code.
