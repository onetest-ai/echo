# Testing — reference

How Echo's tests are laid out and isolated: files, variables, helpers and tools. It changes with the code and the tests,
in the same PR. The rules are in `.ai-run/guides/testing/testing.md`; how to write and run functional cases is in
`tests/app_profile.md`.

## Where tests live

| Kind | Place |
|---|---|
| Rust tests | a `#[cfg(test)] mod tests` at the end of their module, named as the behaviour (`guard::only_this_calls_files_and_the_previous_minutes`) |
| node tests over a window-script block | `crates/echoctl/ui/<name>.test.mjs`; `scripts/check-mod.sh` is the list of mod and window checks, and a new `ui/*.test.mjs` adds itself there |
| functional cases | `tests/cases/<area>/TC-NNN_<slug>.md`, with `run: agent \| person` and `platform` |
| scenarios | `tests/scenarios/S-NN_<slug>.md` |
| the functional map | `tests/feature-map/areas/<area>.json` (one JSON file per area); a case names its features in `requirements:` |
| Playwright autotests | `tests/auto`; a case lists its spec under `auto:` in its frontmatter; run with `npm test` there |
| run results | `tests/reports/` (git-ignored), never in a case |
| made-up data | `tests/fixtures/make_data.py` (acme.example-style names) |

## The test environment (Rust)

`paths::testenv::folders(name)` returns a guard that, while alive, points `ECHOCTL_HOME`, `ECHOCTL_CONFIG_DIR`, `HOME`
and `CLAUDE_CONFIG_DIR` at the test's own folders (`user/`, `user/.claude` for the last two) and unsets `ZDOTDIR`.

## Helpers

- `paths::testenv::eventually`: wait for a released lock.
- `ai::fake::install(root)`: a scripted fake Claude Code.
- Development-build stand-ins (`#[cfg(debug_assertions)]`): `ECHOCTL_PICK_FOLDER` (instead of a native folder dialog),
  `ECHOCTL_TEST_HOSTS` (a plugin's named host reaching a fake service,
  `jshost::a_plugins_named_host_reaches_the_fake_service_set_for_tests`).

## Opening nothing in tests

With `ECHOCTL_TEST_OPEN_LOG_ONLY` set (the test data's `<dir>/env` sets it), Echo, in any build and on any OS, appends
what the window's `open_url`, a sign-in, `echoctl join` or a Settings pane would open to `ECHOCTL_TEST_OPEN_LOG`
(`<dir>/open.log`) and opens nothing (`app::open_log_only`).

## Served test data

`tests/fixtures/make_data.py` writes `<dir>/env` (it sets `ECHOCTL_HOME`, `ECHOCTL_CONFIG_DIR` and the open-log
variables); every CLI step sources it first (`make_data.py::Data`).

## feature-map.py

- `python3 tests/tools/feature-map.py check`: ids, areas and axes consistent, every case's `requirements:` known,
  scenarios' cases exist; exit 1 on a problem (`feature-map.py::check`).
- `python3 tests/tools/feature-map.py changes [BASE]` (default `main`): the features added, changed and removed (the PR's
  `Map:` line), a warning when app code changed but the map didn't, and the cases, scenarios and map lines still quoting
  a label or message the branch took out of the app, exit 1 when there are any (`feature-map.py::gone_texts`,
  `feature-map.py::quoting`). CLI commands, and texts it doesn't recognise as a label, it doesn't find.
- `python3 tests/tools/feature-map.py auto-report RESULTS`: an autotest run's results against the map, to
  `tests/reports/auto-<time>.md` (`feature-map.py::auto_report`); `feature-map.py::auto_links` reads which scenario and
  cases each spec covers.

## Exemplars

| When you are | Exemplar |
|---|---|
| adding a test that needs Claude Code | `ai::fake::install` |
| adding a window node test | `palette.test.mjs::Palette` |
| writing a functional case | `tests/cases/today/TC-003_task-card-opens-details.md` |
| proving a plan's test design | `workbench.test.mjs` |
