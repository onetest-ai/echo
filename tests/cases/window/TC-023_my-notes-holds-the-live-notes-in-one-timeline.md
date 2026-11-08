---
id: TC-023
title: My notes holds the call's live notes — Now pinned on top, ✦ items among the user's notes by time — and the Transcript tab is the transcript alone
priority: high
type: functional
requirements: [F-LIVE-03, F-LIVE-06, F-LIVE-02, F-NOTES-01]
module: window
run: agent
platform: any
data: standard
tags: [live-notes, my-notes, transcript]
---

# TC-023: My notes holds the call's live notes — Now pinned on top, ✦ items among the user's notes by time — and the Transcript tab is the transcript alone

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`
- `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)` (today's Acme weekly sync, which kept live notes and has the user's notes)
- Viewport 1201x868. My notes is the right panel `aside#r-right` (outside `main`), opened with `[data-panel-flip="notes"]` (`aria-label="My notes"`); if it is already open, leave it open

## Test Data

| Where | Holds |
|---|---|
| `$A/live-notes.json` | Now "Berlin pilot go-ahead"; decision "The pilot goes ahead in Berlin" (4:10); action item Rita Gomez "Finish the dry run" (0:45); question "Who replaces Yuki?" (2:31); not covered 10:00–11:00 |
| `$A/live-asks.jsonl` | "What did Omar say about the questionnaire?" → "It is on hold until legal reviews the data processing terms."; What did I miss? → "You missed: Rita owns the budget." |
| `$A/live-usage.json` | 6 jobs, 76K in, 2K out, $0.21 |
| `$A/my-notes.md` | heading "During the call"; 00:12 note, 00:31 ? question, 00:45 ! decision, 00:58 to-do, 01:10 done to-do, 01:30 ⚑ flag with a quote, a note without a time |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#call/<A>/transcript` and wait for "Acme weekly sync"; run `browser_evaluate` with `() => { const p = document.querySelector('#pane'); const t = p.querySelector('.transcript'); return { live: !!p.querySelector('[aria-label="Live notes"]'), find: !!p.querySelector('#find'), turns: p.querySelectorAll('.turn').length, wide: t.getBoundingClientRect().width >= p.clientWidth - 70 } }` | `live` `false` (no live column in the tab), `find` `true`, `turns` 17, `wide` `true`: the transcript fills the pane |
| 2 | Open My notes (`[data-panel-flip="notes"]`) and run `browser_evaluate` with `() => { const s = document.querySelector('#r-right section[aria-label="Live notes"]'); const list = document.querySelector('#r-right #nt-list'); return { first: s && s.compareDocumentPosition(list) === Node.DOCUMENT_POSITION_FOLLOWING, cap: s.querySelector('.nl-cap').innerText, now: s.querySelector('.nl-now').innerText, status: s.querySelector('.nl-status').innerText.replace(/\s+/g, ' ') } }` | `first` `true` (the live box is above the list); `cap` "NOW" (or "Now"); `now` "Berlin pilot go-ahead"; `status` is "Claude · work · The call ended · 6 updates · 76K in · 2K out · $0.21 · Not covered: 10:00–11:00" |
| 3 | Run `browser_evaluate` with `() => [...document.querySelectorAll('#r-right #nt-list .nt-entry')].map((e) => [e.classList.contains('nt-livei') ? '✦' : 'mine', (e.querySelector('.at') \|\| {}).textContent \|\| '', (e.querySelector('.nt-text') \|\| {}).textContent \|\| ''])` | In this order: mine 00:12 "Dana says the checklist went out on Monday"; mine 00:31 "Does legal need the old contract too"; mine 00:45 "Pilot starts in Berlin only"; ✦ 00:45 "Finish the dry run"; mine 00:58 "Remind Omar about the access list"; mine 01:10 "Send the agenda"; mine 01:30 "Budget owner is Rita"; mine "" "a note without a time"; ✦ 02:31 "Who replaces Yuki?"; ✦ 04:10 "The pilot goes ahead in Berlin" |
| 4 | Read the three ✦ rows | Each starts with the mark ✦; 00:45 shows the to-do mark ☐ and the owner "Rita Gomez"; 02:31 the question mark ?; 04:10 the decision mark !; the heading "During the call" is above the first row |
| 5 | Read the end of the list | A heading "Asked about this call" with "What did Omar say about the questionnaire?" → "It is on hold until legal reviews the data processing terms." and "What did I miss?" → "You missed: Rita owns the budget." |
| 6 | Run `browser_evaluate` with `() => [...document.querySelectorAll('#r-right .nt-livei')].map((e) => e.querySelectorAll('[data-note-edit],[data-note-del],textarea').length)`; then click the text "Who replaces Yuki?" and read whether `#note-edit` exists | `[0, 0, 0]`: a live row has no ✎ or ×; after the click there is no `#note-edit` (it is read-only) |
| 7 | Hover the row "Dana says the checklist went out on Monday", click its ✎ (`[data-note-edit]`), then press Escape | The editor `#note-edit` opens with that text and closes on Escape, the text unchanged: the user's notes edit as before |
| 8 | Click the ⋯ button at Now (`[data-live-menu]`), read the menu `.nl-menu`, then press Escape | The menu shows "Model for this call" with the select `[data-callprofile]` on "Claude · work", "with your Claude account", and "Ask Claude about this call"; no "Pause live notes" (the call ended); Escape closes it (no `.nl-menu`) |
| 9 | Click the tab "Minutes", then the ✦ row's time chip "02:31" (`#r-right .nt-livei a.at`, the one reading 02:31); wait 1 s and run `browser_evaluate` with `() => { const p = document.querySelector('#pane'); const mid = p.getBoundingClientRect().top + p.clientHeight / 2; const c = [...document.querySelectorAll('.turn')].map((e) => { const r = e.getBoundingClientRect(); return { t: Number(e.dataset.t), d: Math.abs((r.top + r.bottom) / 2 - mid) } }).sort((a, b) => a.d - b.d)[0]; return { hash: location.hash, scrollTop: p.scrollTop, centered: c.t } }` | `hash` ends `/transcript`; `scrollTop` above 0; `centered` between 120 and 151: the transcript scrolled to the moment as a user's chip does (Echo picks the turn whose end is within 2 s before it) |
| 10 | Click the row `[role=option][data-call$="-initech-planning"]` and read My notes | No `section[aria-label="Live notes"]`, no `.nt-livei`, no "Asked about this call"; the list shows "Add notes about this call. Select words in the transcript to comment on what was said." — the panel as without live notes |
| 11 | Click the row `[role=option][data-call$="-quick-check-in"]` (the top bar's "Writing the transcript…" chip carries the same `data-call`) (transcribing; live notes on, none written) and read My notes | No `section[aria-label="Live notes"]` and no ✦ row: with live notes on but empty, the panel is as without them |

## Expected Final State
Nothing changed; Quick check-in is open with My notes beside it.

## Notes
- Read-only: creates or changes nothing (step 7 cancels its edit).
- While a call records with live notes on, the Transcript tab is the live transcript alone at full width, kept at its end (`section#live-transcript`), and Now's menu also has Pause / Resume: that needs a recording with a signed-in model, person-run (F-LIVE-01, F-LIVE-02).
- Step 9's turn is the one every time chip picks (`end >= t - 2`, #101): for 2:31 the 2:16 turn.
