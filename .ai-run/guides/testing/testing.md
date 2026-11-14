# Testing

**Project**: Echo · **Scope**: test design in the plan, Rust and node tests, functional cases and the map

**Reference**: `.ai-run/reference/testing/testing.md` — where tests live, the test environment, helpers, variables and
`feature-map.py` (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Rules
- R1. A change in behaviour starts from acceptance criteria, each with a test: a Rust test, a node test over a window-script block, or a functional case. The plan's **Tests** part, written before the code, names:
  - the cases to add, change and remove: they are the acceptance criteria, `run: agent` wherever the window or the CLI can show it;
  - the scenarios they go into;
  - the features they check in the functional map: new behaviour adds its rows;
  - the Rust or node tests that prove it;
  - for each feature that changes, its existing cases checked against the new behaviour: obsolete ones removed, duplicates merged (not the whole area).

  Evidence: `workbench.test.mjs` (ref: Where tests live)
- R2. A Rust test that touches the data folder or config dir takes the test environment first and keeps it alive, so no test reads the user's Echo or Claude Code setup; nothing else sets those variables. Evidence: `paths::testenv::folders` (ref: The test environment (Rust))
- R3. Rust tests sit at the end of their module, each named as the behaviour it proves. Evidence: `guard::only_this_calls_files_and_the_previous_minutes` (ref: Where tests live)
- R4. Wait for a released lock by polling, never a fixed sleep. Evidence: `paths::testenv::eventually` (ref: Helpers)
- R5. A functional case is one behaviour; a scenario chains cases into a journey. How to write them is in `tests/app_profile.md`. Evidence: `tests/app_profile.md` (ref: Where tests live)
- R6. What Echo does is the functional map; a case names its features; a change in behaviour updates its area's map in the same PR, and the map's check passes. Evidence: `tests/tools/feature-map.py::check` (ref: feature-map.py)
- R7. Cases change with behaviour: a changed behaviour updates its cases, a removed feature removes them, a bug found by hand gets a case. Evidence: `feature-map.py::check`
- R8. Test data is made up (acme.example-style names), never real call data, in tests, cases, reports or issues. Evidence: `tests/fixtures/make_data.py` (ref: Where tests live)
- R9. Every mod and window check is listed in the mod-check script; a new node test adds itself there. Evidence: `scripts/check-mod.sh` (ref: Where tests live)
- R10. Tests never open the user's browser: under the test data's environment Echo logs what it would open and opens nothing. A case that clicks an outside link checks that log, and never runs that click without the environment. Evidence: `app::open_log_only` (ref: Opening nothing in tests)
- R11. Tests never open a native dialog or reach a real service: a test relying on a development-build stand-in is debug-only. Evidence: `jshost::a_plugins_named_host_reaches_the_fake_service_set_for_tests` (ref: Helpers)
- R12. A text the app shows (a label, a message, a CLI command or its list) renamed or removed is searched in all of `tests/`, not only the touched features' cases, and every case and scenario that quotes it is updated in the same PR; what the map tool doesn't find is searched by hand. Evidence: `feature-map.py::quoting` (ref: feature-map.py)
- R13. Every line still quoting a text gone from the code is updated, or the PR says why the app still shows it (a text now built from parts). Evidence: `feature-map.py::gone_texts` (ref: feature-map.py)
- R14. Run results never go into a case or into git. Evidence: `feature-map.py::auto_report` (ref: Where tests live)
- R15. Autotests automate cases:
  - a case gets an autotest only once its manual run passes and its feature is stable;
  - the case names its spec;
  - the full suite runs before every release (quality-gates R5);
  - a failing spec is fixed at its root cause: a stale text or clock-dependent data is a bug in the test, not a footnote.

  Evidence: `feature-map.py::auto_links` (ref: Where tests live)
- R16. Every CLI step on served test data runs with that data's environment, and a run whose data folder isn't inside the test dir is aborted: the real data folder is never touched. Served windows keep opening nothing (R10). Evidence: `make_data.py::Data` (ref: Served test data)
