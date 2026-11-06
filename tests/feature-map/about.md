# Echo — functional map

What Echo does, as behaviours a user can see or use, one per row. It is the basis for two things:

- **Coverage.** Each case in `tests/cases/` lists the features it checks in its frontmatter:
  `requirements: [F-CALLS-01, F-MIN-03]`. A script reports, per feature, whether it has `run: agent` cases,
  `run: manual` cases, or none.
- **Test data.** Each entity's **state axes** (below) tell the data generator (`tests/fixtures/make-data.sh` and its
  successors) which states, and which combinations of them, must exist in a data set.

Columns: **Priority** — critical (the core promise, data loss or privacy), high, medium, low. **Check** — `agent`
(the served window or the CLI on made-up data can show it; see `app_profile.md` → Out of Scope) or `manual` (a real
call, Teams, Microsoft 365, a signed-in Claude Code, macOS permissions, an install or update, the tray, Windows).
**Axes** — the state axes the behaviour depends on, by name (`entity.axis`, defined in State axes). Axes listed under
State axes but in no row (`meeting.response`, `call.today_minutes`) are Today's: kept for the data generator while
Today is reworked.

Keeping it current: a change in behaviour adds, changes or removes rows here **in the same PR**, with its cases. IDs
are never reused: a removed feature's row goes, its ID stays retired. Rows marked "(?)" were inferred rather than seen
in the code and need a review. Areas with status `rework` (Today, Knowledge) list only their top-level features, with
no axes or combinations, until their rework settles.
