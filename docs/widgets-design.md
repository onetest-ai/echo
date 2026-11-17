# Widgets — how Echo's parts offer them, and how Today takes them

Status (2026-11-10): agreed with the user in conversation; not started. **The source is the Workbench's row "Widgets —
everything we build"** (boards `WidgetsCalendar`, `WidgetsTasks`, `WidgetsCallsNeeds`, `WidgetsPluginAPI`,
`WidgetsPluginKinds`): every widget we build is on it, nothing is taken from other boards. The row "Today, on a real day" stays: its boards
are recommendations and ideas (the user: "они важны") — the default, edit mode. **Responsive behaviour is agreed
there** (long discussed): `TodayLayoutRules`, `TodayResponsive`, `TodayPanels`, `TodayLive` and `today-design.md` —
columns from the centre's width (fold 340 / unfold 380), right panel takes width, bottom panel takes height (fills
shrink, too short → Today scrolls), views step down, the open task marked on Today. Widgets follow it as is. Layout rules (columns, hug / fill / temporal, panels) stay as in
`today-design.md`. Dashboards beyond Today come later (decided: widgets and Today first).

## Decisions

| Question | Answer |
|---|---|
| What a widget is | A **provider** (a named function of an Echo part or a plugin that returns JSON of a known kind) + **how it is drawn** (Echo draws every kind; a plugin may ship its own page). Data and drawing apart. |
| How many per part | **Several.** A part offers a set; the user picks which go on Today. Calendar, tasks, calls each offer a few (catalog below). |
| Core and plugins | One **catalog**. Core widgets register like plugin ones (`core:…`, `plugin:<id>:…`). Today's `/api/today` is split into providers. |
| On Today | **Instances**: a widget + its params (a thread, a view), with span and order. Today ships a good default; the user adds, removes, moves, sizes. |
| Where Today's layout lives | On the user's computer, in Echo's config folder beside `window-layout.json` (`today-layout.json`, ≤ 16 KB). No servers. |
| Claude | Sees and edits through the CLI: `echoctl widget list / show <id> [--thread …]`, `echoctl today add / remove / move`. The data Claude reads is the data Today shows. |
| Plugins' own pages | **In the first version.** A routine of the Scheduler already shows its result on Today (the `deliver` kind `echo`, a card until read): that becomes a widget of kind `card`, and a plugin's own `page` widget is sandboxed like its panel. |
| Refresh | By events (`ledger.changed`, `calendar.changed`, `call.ended`, `needs.changed`, a delivered card) or, for outside sources, an interval with a floor. The window asks once per poll for all of Today (`/api/today` → the instances' data + a fingerprint), never one request per widget. |
| Models | **No model call inside a provider.** What needs a model (the brief, Needs you now's reading) is a scheduled job; its provider reads the result. (Small Macs: Teams must not lag.) |
| Untrusted text | Rows open `echo://` links (outside links after a confirm); plugin and routine text only through the safe writers (`security/untrusted-text.md`). A plugin's provider runs with the grants it was given at install. |

## A widget's definition

```
{ id: "core:work.mine" | "plugin:jira:burndown",
  part: "Tasks",                       // what the picker groups it under
  label, about,                        // the picker's line
  kind: "stat" | "stats" | "chart" | "list" | "status" | "card" | "page" | core kinds (day, work, needs, next…),
  params: { thread?, client?, view? }, // its schema
  sizes: ["S", "M", "L"],              // spans 1–3
  height: "hug" | "fill" | "temporal",
  show: "always" | "when_not_empty",
  refresh: { on: [events] } | { every: "15m" } }
```

Plugins declare theirs in `echo-plugin.json` → `widgets`; the host half answers
the `op`. A widget that can't fit its size steps down to its compact view (the calendar to a strip).

## How a plugin registers its widgets (boards `WidgetsPluginAPI`, `WidgetsPluginKinds`)

As it adds commands, Needs-you items and delivery ways — declared, then answered from `apply(ctx)`:

- **Manifest** `echo-plugin.json` → top-level `"widgets": [{id, label, about, kind, sizes, height?, show?, refresh?,
  params?}]` (params as settings, plus types `thread` and `client`). Declared, so the catalog and the permission sheet
  know them without running the plugin. Sheet line: "Adds widgets you can put on Today: …"; no grant of its own.
- **Host half**: `ctx.widgets.provide(id, async ({params, size}) => data)` — pulled when shown and on refresh;
  `ctx.widgets.set(id, data)` / `ctx.widgets.clear(id)` — pushed when the plugin learns of a change (as `ctx.inbox`).
  Echo keeps the last answer and draws it at once; a cleared widget leaves Today.
- **Client half**, kind `page`: `ctx.widgets.draw(id, ({data, size, theme}) => html)` — draws into Echo's frame
  (title, ⋯, size, theme); data still from the host half; escaping with `ctx.ui.esc`.
- **Data per kind**: stat {value, unit, label, progress, note} · stats {items ≤ 4} · chart {type, series, unit} ·
  list {rows, more} · status {state, title, note} · card {markdown, actions}; any: `open` (echo:// or https),
  `actions` [{label, command, args}] → the plugin's `ctx.command`, `empty`.
- **Rules**: a provider answers in 5 s (else the last answer, dated); events first, `every` ≥ 60 s, nothing while
  Today is closed except `set`; no `ctx.claude.run` in `provide` (model work is a job whose result is `set`); text is
  data (Echo's escaping, safe Markdown, https after a confirm); a wrong shape shows "Couldn't draw this" with why.
- **Ids** `core:<part>.<widget>`, `plugin:<id>:<widget>`; `echoctl widget show` calls a provider and checks the shape
  (how a plugin author tests one).

## The catalog, first draft (to agree)

**Calendar**
- Your day — calendar (fill): the day's meetings by the hour, overlaps side by side.
- Your day — agenda (fill): "where you'll be" (chosen by the user 2026-11-10 for its call to action): a line a place, a reason on each — overlaps fold into the one you go to ("instead of …", "4 others at once
  skipped"), a free gap carries your work ("your work · 1 h"), "no reply", "maybe · drop in till 17:00", "you're a
  speaker"; bar: solid = going, dashed grey = no reply, dashed violet = maybe, dashed green = your task; time violet =
  next, amber = a choice made; "Show N skipped".
- Your day — strip (hug): the day as one bar; the narrow window's view.
- Next meeting (hug): the next one, with its prep and Join / Record. S/M: title, when, what's expected from you, Show
  prep, Join & record (language beside it). L (the user's variant, 2026-11-10): the prep brought to the front —
  Walk in knowing (≤ 3 points), Who's coming (the invite's answers, a newcomer named), Ask them, Open prep /
  Join & record / Ask Claude about it, "Recording starts with the call".
- The day's choices (temporal): overlaps and unanswered invites, Echo's proposal, the user decides.
- Every meeting in these widgets acts as in Calendar: left click opens its card (right panel on Today), right click
  or ⋯ on the row opens Calendar's event menu (`eventMenu`, plugins' `calendar.event` items included); Record is
  Calendar's split button with the language beside it (`recordButton`: the series' rule, else the default).

**Tasks** (user, 2026-11-10: keep today's list — "очень эффективный и простой"; the rest dropped)
- My work · this week (fill): today's list exactly as it is — late first, then by date; whose it is; mode, ✦, Close.
- Threads at a glance (hug): a bar a thread — done (green), to do (amber), waiting (red) — and the total; settings:
  week / month, threads / clients, only mine.
- Done this week (stat).

**Calls**
- Recorded this week (stats).

**Needs you now** (temporal): as today, with ✦ and Reply.

**Routines** (Scheduler): each routine delivered to Today is a card widget (the morning brief: until read).

**Plugins**: `widgets` in the manifest — stat, stats, chart, list, status, card, page.

## Phases (before and into `today-design.md`'s)

Order set by the user (2026-11-10): first design the widgets of each part (on the canvas), then the catalog to pick
them from (after Apple's widget gallery: by app, a preview at each size), then the rest. The catalog is reused
everywhere widgets go later, not only Today.

0. **Design: each part's widgets**, at each size, light and dark, full and empty/temporal states — boards on the
   Workbench canvas, one per part (Calendar, Tasks, Calls & Needs, Routines & plugins).
0b. **Design: the catalog** (Add widget, board `WidgetsGallery`): edit mode on Today (`TodayWidgets`' idea: presets,
   columns, S·M·L per widget, drag, ×) with the side panel — search, parts, a row per widget (on Today / Add), the
   selected one previewed at S / M / L (Apple-style), Add at a size or drag it onto Today.
1. **Catalog and providers under today's Today.** `widgets::catalog()`, providers for what Today shows now,
   `/api/today` from instances, `echoctl widget list/show`; no visible change.
2. **Instances and `today-layout.json`**; the current page as the default instances; `echoctl today add/remove/move`.
3. **The layout engine** (today-design phase 1) and the visible steps (its phases 2–5), one pull request each.
4. **Add widget**: the picker by part (design on the canvas first), sizes, remove, move (today-design phase 6).
5. **Routine cards as widgets; `widgets` in the manifest** with `page` (today-design phase 7, moved up for the Scheduler).

## Open

- The catalog above: what to drop, what's missing.
- The picker's look (a board on the canvas before code).
- A widget's settings (its params) — in the widget's ⋯ menu, or in the picker only.

## Edit mode: how it behaves (the working prototype on `WidgetsGallery` — Play it; the build matches it)

- **Order is the list's order.** No dense packing: the grid never moves a widget to fill a hole.
- **Moving and adding by drag**, both directions: over a widget, its **top quarter** makes a new row above it, its
  **bottom quarter** a new row below it, its **left / right half** puts the dragged one before / after it in its row;
  a violet bar on that edge says where. The zone at the end makes a new last row. The dragged widget is dimmed.
- **Rows**: the layout is the list in order plus row starts (a widget with `row` begins a new row); taking a widget
  out hands its row start to the next one.
- **From the panel**: drag a row or the preview onto Today, or Add at the size picked under the preview.
- **S·M·L** on a widget: 1, 2 or 3 columns; only the sizes the widget offers. **×** takes it off (its row says Add
  again). **Reset to the default** puts back the default instances.
- **The panel**: search; parts; a row per widget (temporal marked; "on Today" or Add); the selected one previewed at
  S / M / L (sizes it doesn't offer greyed); on Today already → "move it to <size>".

### How we keep the build to it

- The reorder is one pure function in the window's script — `place(layout, id, size, target, side)` — with a node test
  of a table of moves (before / after / above / below / last / from the panel / onto itself / a row start handed on / a size the widget
  lacks), as the layout
  engine's `today-layout.test.mjs`.
- Functional cases first (`tests/cases/today/`), from this section, one behaviour each; then an autotest stage for each
  in a Today scenario (real drags with Playwright's mouse, not a programmatic reorder).
- Before the PR: a screenshot of the built edit mode next to the board, every difference fixed or named.
