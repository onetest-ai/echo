---
id: TC-017
title: The top bar holds the sidebar switch, the Record pill, the account chip and the Inbox on one line, overlapping nowhere down to the narrowest window
priority: medium
type: functional
requirements: [F-WS-09]
module: shell
run: agent
platform: any
tags: [shell, top-bar, layout]
---

# TC-017: The top bar holds the sidebar switch, the Record pill, the account chip and the Inbox on one line, overlapping nowhere down to the narrowest window

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)

## Checks (run exactly as written)

- **Check L** (`browser_evaluate`): `() => { const q = (s) => document.querySelector(s); const els = { flip: q('#r-top [data-cmd="bar-flip"]'), pill: q('#r-top .pill-slot'), acct: q('#r-top .acct'), inbox: q('#r-top [data-panel-flip="needs"]') }; const r = Object.fromEntries(Object.entries(els).map(([k, e]) => [k, e && e.getBoundingClientRect()])); const names = Object.keys(r).filter((k) => r[k]); const hit = (a, b) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom; const mid = (x) => x.top + x.height / 2; return { missing: Object.keys(r).filter((k) => !r[k]), overlap: names.some((a, i) => names.slice(i + 1).some((b) => hit(r[a], r[b]))), sameLine: names.every((k) => Math.abs(mid(r[k]) - mid(r.inbox)) <= 2), inOrder: r.flip.right <= r.pill.left && r.pill.right <= r.acct.left && r.acct.right <= r.inbox.left, inside: r.inbox.right <= window.innerWidth, scroll: document.documentElement.scrollWidth <= window.innerWidth } }`. Expected: `missing` `[]`, `overlap` false, `sameLine`, `inOrder`, `inside` and `scroll` true

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | At 1201 x 868, navigate to `{{base_url}}/#today` and wait for "Your day" | From left to right the top bar shows "Expand the sidebar", the Record pill ("Record next call"), the account chip "Claude Code (as configured)" and the button "Inbox" at the right end |
| 2 | Run check L | As expected (see Checks) |
| 3 | Resize the browser to 900 x 700 and run check L | As expected |
| 4 | Resize the browser to 760 x 480 (the narrowest the window allows) and run check L | As expected |
| 5 | Click "Inbox" | The right panel opens on the tab "Inbox" (the click reached the button); click "Inbox" again to close it |

## Expected Final State
The four controls stay on one line in order, apart, inside the window at every width.

## Teardown
- Resize the browser back to 1201 x 868.

## Notes
- On macOS the top strip is the title bar: a press on it drags the window, but not on its buttons, selects and labels (`onTitleBar`), so the chip and the Inbox stay clickable. The served page has no native window to drag, so the drag itself is for a person.
