# Echo — functional map

What Echo does, as behaviours a user can see or use: one feature per entry, grouped by area. It is the basis for two
things:

- **Coverage.** Each case in `tests/cases/` names the features it checks in its frontmatter, e.g.
  `requirements: [F-CALLS-01, F-MIN-03]`. This page shows, per feature, whether it has agent cases, manual cases, or
  none.
- **Test data.** Each entity's **state axes** tell the data generator (`tests/fixtures/make-data.sh` and its
  successors) which states, and which combinations of them, must exist in a data set.

Each feature has a **priority** (critical: the core promise, data loss or privacy; high; medium; low), a **check**
(`agent`: the served window or the CLI on made-up data can show it; `manual`: it needs a real call, Teams,
Microsoft 365, a signed-in Claude Code, macOS permissions, an install or update, the tray, or Windows) and the
**axes** its behaviour depends on (`entity.axis`). Axes that no feature names yet (`meeting.response`,
`call.today_minutes`) belong to Today and are kept for the data generator while Today is reworked.

The map lives in `tests/feature-map/`: one file per area in `areas/` (its features), one per entity in `axes/` (its
state axes), plus `combinations.json` and `hostile.json`, so features of different parts of the app are changed
without merge conflicts.

**Keeping it current:** a change in behaviour adds, changes or removes its features in its area's file **in the same
PR**, with its cases; `tests/tools/feature-map.py check` must pass. IDs are never reused: a removed feature goes, its
ID stays retired. Features marked "needs review" were inferred rather than seen in the code. Areas being reworked
(Today, Knowledge) list only their top-level features, with no axes or combinations, until the rework settles.
