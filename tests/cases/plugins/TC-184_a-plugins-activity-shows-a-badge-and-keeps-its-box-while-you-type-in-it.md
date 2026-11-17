---
id: TC-184
title: A plugin's activity shows a badge on its icon ('!' with nothing filed, then the count), and while you type in its box Echo leaves the box alone though the plugin asks for a redraw every second; left, it is drawn again
priority: medium
type: functional
requirements: [F-PLUG-105]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, activity]
---

# TC-184: A plugin's activity: its badge, and its box kept while you type

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-180's: the standard set served, `{{base_url}}/` loaded first, CLI steps after `. <dir>/env`
- "call-demo" installed and on (TC-180 step 1). Its activity "Filed calls" (`[data-go="call-demo:filed"]`): its badge the number of calls it filed, '!' while none; its page "N filed · Drawn M times" (M grows: it asks Echo to redraw every second) and a box "A draft"; while that box is focused and not empty, its `editing()` is true

**Changes:** `<data>/.echo/plugins/call-demo/` (the calls it filed)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run this case where no call is filed yet (`echoctl plugin call call-demo count` is `0`), then look at the activity bar | "Filed calls" carries the badge "!" (`aria-label="Needs your attention"`) |
| 2 | File a call (TC-180 steps 2–4) | The badge reads "1" (`aria-label="1 new"`), as Threads' count |
| 3 | Click "Filed calls" | Its page: heading "Filed calls", "1 filed · Drawn M times" (M grows by about one a second) |
| 4 | Type `draft words` in the box "A draft"; read the line, wait 3 seconds, read it again | The same "Drawn M times" both times; the box still holds "draft words" and has the focus |
| 5 | Click the heading "Filed calls" (the box loses the focus); wait 2 seconds | "Drawn M times" grows again |

## Expected Final State
Call demo on; one or more calls filed.

## Teardown
- None beyond the scenario's

## Notes
- The badge's text (counts cut to 1–99 then "99+", '!' kept, anything else none), a `badge()` or `editing()` that throws logged once and ignored, and a box not drawn yet drawn even while `editing()` says true — `node crates/echoctl/ui/call-slots.test.mjs`.
- The page has nothing of the plugin's own in Echo's code: its look is the plugin's HTML with Echo's generic classes (`set-main`, `set-in`).
