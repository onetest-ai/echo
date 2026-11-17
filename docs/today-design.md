# Today, rebuilt — plan

Status (2026-11-06): designed, not started. Boards: the "Today, on a real day" row of the Echo Workbench canvas
(`TodayDense`, `TodayWidgets`, `TodayViews`, `TodayPluginWidgets`, `TodayLayoutRules`, `TodayResponsive`, `TodayPanels`,
`TodayLive` — the last one is a working prototype of the layout: Play it).

## Goal

Today is the page the window opens on. On a real day (14 meetings, up to 5 at once, 6 unanswered invites) the current
list can't say where to be, and its fixed layout doesn't survive the right or bottom panel opening. The new Today:

- a good default made of **widgets**, which people can rearrange, size and add to (plugins too);
- **temporal** widgets that are there only while they hold something (Needs you, the day's choices, the morning brief),
  and leave when answered or read;
- one **layout model** that re-lays the centre out whenever the menu, the right panel, the bottom panel or the window
  changes size.

## Decisions so far

| Decision | Answer |
|---|---|
| Layout model | Columns, not cells. Columns come from the centre's own width (340 px a column, up to the number chosen). Widgets have a span (1–3) and a height kind: hug, fill (one per column), temporal. |
| One layout or several | One, arranged at the widest; narrower ones are derived: same order, spans clamp to free columns, views step down. |
| Panels | The task opens in the right or bottom panel as the user keeps it (no drawer over Today). Right takes width → fewer columns; below takes height → fills shrink; too short → Today scrolls as a page. |
| Thresholds | Fold at 340 px a column, unfold at 380 (no flicker while a panel is dragged). |
| Your day | One widget, four views: Calendar, Agenda (today's list), Timeline, Strip. Steps down by itself: under 280 px wide → Strip; under 320 px tall → Agenda opened on now. |
| Kept from today's Today | Needs you now, the next-meeting card, My work, the brief card with Got it, "an empty part isn't drawn". |
| Plugins | `contributes.widgets`: data only (`stat`, `stats`, `chart`, `list`, `status`), Echo draws them; `page` for the rest, sandboxed like a panel. `show: when_not_empty` makes one temporal. |
| Choices on overlaps | Echo proposes (you organise / you're required / a promise is due / a client), the user decides; nothing is sent to organisers. |

## How it ships: the current Today, changed in place

No feature branch and no flag. Today already exists and works; this is its next shape, not a second page. Every pull
request changes the one Today a little and leaves it better than before, so each can go to users in the next beta:

- **First, the engine under the page as it is.** The layout engine places today's parts exactly where they are now —
  no visible change except that Today now re-lays itself out when a panel opens or closes (a fix in its own right).
- **Then one visible step per pull request**, small enough to review in one sitting and to revert alone.
- **The only big visual change** (the default layout: Your day as a column, cards beside it) is one pull request of its
  own, late, when every piece it moves already exists.

## Phases

Each phase is a few pull requests, each closing an issue with its acceptance criteria and shipping in the next beta.
A phase ends with the usual checks (clippy, tests, the Windows check, the UI's node tests, `window --selftest`).

0. **`app.js` in parts, before anything else** (a move, no change in behaviour).
   - `ui/app.js` (≈ 8 000 lines, touched by ~130 commits in a month) split along its `// ──` markers into files under
     `ui/app/` by area (core, today, calls, calendar, settings, plugins, projects, tasks, people, brain, graphs,
     terminals, notes, palette, refresh). Still one script: `window.rs` serves them as
     `concat!(include_str!(…), …)` in the old order — same global scope, same CSP, no bundler, no modules.
   - Proof it moved nothing: a test that the parts joined are byte for byte the old `app.js` (kept for that PR only).
   - The node tests read the joined source through one helper (`ui/app-src.mjs`) instead of `app.js`.
   - Merged at a quiet moment (open pull requests in first), since every branch touching `app.js` rebases onto it.
1. **Layout engine under the current page.**
   - A no-DOM block in `app.js` (`// ── today layout`): `todayLayout({width, height, columns, widgets})` →
     a box per widget, its view, and whether the page scrolls. Hysteresis kept in its state.
   - `ui/today-layout.test.mjs`: a table of sizes (1440, 1024, ± menu, ± right panel, ± bottom panel) → placements.
   - Fixture: the dense Wednesday (14 meetings, overlaps, 6 unanswered) as JSON for tests and `--serve`.
   - Today's parts placed by it, as they are today; a ResizeObserver on the centre re-lays them when panels change.
2. **Needs you now and Next on Today.** Needs you from `attention.rs` (last 24 h, where you watch), temporal; the
   next-meeting card; the open task marked on Today.
3. **Your day: the views.** The current list becomes the Agenda view; Calendar (overlaps side by side), Strip and
   Timeline added; the picker in the widget's head; the step-down rules.
4. **The day's choices.** The calendar keeps each meeting's reply status (today the day lists only accepted ones);
   overlaps and unanswered invites as a temporal widget; the proposed route, decided by the user, sent nowhere.
5. **The new default layout.** Columns, spans, fills as on the boards; one pull request.
6. **Customise.** Edit mode: columns 1–3, add / remove / move, S·M·L, presets; saved in `window-layout.json` (or a
   `today-layout.json` beside it, ≤ 16 KB).
7. **Widgets from plugins.** `contributes.widgets` in the manifest, the kinds, `refresh`, `show`; the plugin guides
   updated.

## Acceptance (the whole)

- Opening or closing the menu, the right panel or the bottom panel never reorders widgets; only sizes, places and views
  change.
- At 1512 × 982 with the menu folded: 3 columns; with a task on the right: 2; at 1024 with a task: 1, Your day as Strip.
- On the dense-day fixture every meeting is visible in some view, the overlaps are marked, and nothing is sent anywhere
  without a click.
- A temporal widget with nothing in it takes no space.

## Changes to other documents

- `.ai-run/guides/design/window.md`: Today's section updated with each phase; Workbench layout gains the centre's
  column rule.
- `plugins-design.md` and `.ai-run/guides/plugins/echo-plugins.md`: `contributes.widgets` in phase 8.

## Open questions

- Default panel widths and minimums (right 400? bottom 280?), and what wins when both are open.
- Do panels keep their state per activity (Today vs Calls), as Layout by phase suggests?
