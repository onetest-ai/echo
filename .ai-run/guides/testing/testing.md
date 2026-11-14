# Testing

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: test design in the plan, Rust and node tests, functional cases and the map

## Rules
- R1. A change in behaviour starts from acceptance criteria, each with a test: a Rust test, a node test over a window-script block, or a functional case. The plan's **Tests** part, written before the code, names:
  - the cases in `tests/cases/<area>/` to add, change and remove: they are the acceptance criteria, `run: agent` wherever the window or the CLI can show it;
  - the scenarios in `tests/scenarios/` they go into;
  - the features they check in the functional map (`tests/feature-map/areas/<area>.json`): new behaviour adds its rows;
  - the Rust or node tests that prove it;
  - for each feature that changes, its existing cases checked against the new behaviour: obsolete ones removed, duplicates merged (not the whole area).

  Evidence: `workbench.test.mjs`
- R2. A Rust test that touches the data folder or config dir takes `paths::testenv::folders(name)` first and keeps its guard alive; nothing else sets `ECHOCTL_HOME` / `ECHOCTL_CONFIG_DIR`. Evidence: `paths::testenv::folders`
- R3. Rust tests are a `#[cfg(test)] mod tests` at the end of their module, each named as the behaviour it proves (`only_this_calls_files_and_the_previous_minutes`). Evidence: `guard::only_this_calls_files_and_the_previous_minutes`
- R4. Wait for a released lock with `paths::testenv::eventually`, never a fixed sleep. Evidence: `paths::testenv::eventually`
- R5. A functional case is one behaviour, `tests/cases/<area>/TC-NNN_<slug>.md`, with `run: agent | person` and `platform`; a scenario (`tests/scenarios/S-NN_<slug>.md`) chains cases into a journey. Rules for writing them are in `tests/app_profile.md`. Evidence: `tests/app_profile.md`
- R6. What Echo does is the functional map, `tests/feature-map/` (one JSON file per area); a case names its features in `requirements:`; a change in behaviour updates its area's file in the same PR, and `python3 tests/tools/feature-map.py check` passes. Evidence: `tests/tools/feature-map.py::check`
- R7. Cases change with behaviour: a changed behaviour updates its cases, a removed feature removes them, a bug found by hand gets a case. Evidence: `feature-map.py::check`
- R8. Test data is made up (`tests/fixtures/make_data.py`, acme.example-style names), never real call data, in tests, cases, reports or issues. Evidence: `tests/fixtures/make_data.py`
- R9. `scripts/check-mod.sh` is the list of mod and window checks; a new `ui/*.test.mjs` adds itself there. Evidence: `scripts/check-mod.sh`
- R10. Tests never open the user's browser: with `ECHOCTL_TEST_OPEN_LOG_ONLY` set (the test data's `<dir>/env` sets it) Echo, in any build and on any OS, appends what the window's `open_url`, a sign-in, `echoctl join` or a Settings pane would open to `ECHOCTL_TEST_OPEN_LOG` (`<dir>/open.log`) and opens nothing. A case that clicks an outside link checks `open.log`, and never runs that click without the variable set. Evidence: `app::open_log_only`
- R11. Tests never open a native dialog or reach a real service: a test relying on a development-build stand-in (`ECHOCTL_PICK_FOLDER`, `ECHOCTL_TEST_HOSTS`) is `#[cfg(debug_assertions)]`. Evidence: `jshost::a_plugins_named_host_reaches_the_fake_service_set_for_tests`
- R12. A text the app shows (a label, a message, a CLI command or its list) renamed or removed is searched in all of `tests/`, not only the touched features' cases, and every case and scenario that quotes it is updated in the same PR. CLI commands, and texts `feature-map.py changes` doesn't recognise as a label, are searched by hand. Evidence: `feature-map.py::quoting`
- R13. `feature-map.py changes` lists the lines still quoting a label or message gone from the code (exit 1): each is updated, or the PR says why the app still shows it (a text now built from parts). Evidence: `feature-map.py::gone_texts`
- R14. Run results go to `tests/reports/` (git-ignored), never into a case. Evidence: `feature-map.py::auto_report`
- R15. The Playwright suite in `tests/auto` automates cases:
  - a case gets an autotest only once its manual run passes and its feature is stable;
  - the case lists its spec under `auto:` in its frontmatter;
  - the full suite runs before every release (quality-gates R5);
  - a failing spec is fixed at its root cause: a stale text or clock-dependent data is a bug in the test, not a footnote.

  Evidence: `feature-map.py::auto_links`
- R16. Every CLI step on served test data sources `<dir>/env` first, and a run where `ECHOCTL_HOME` isn't inside the test dir is aborted: the real data folder is never touched. Served windows keep `ECHOCTL_TEST_OPEN_LOG_ONLY` (R10). Evidence: `make_data.py::Data`

### Read first
| When you are | Exemplar |
|---|---|
| adding a test that needs Claude Code | `ai::fake::install` |
| adding a window node test | `palette.test.mjs::Palette` |
| writing a functional case | `tests/cases/today/TC-003_task-card-opens-details.md` |
