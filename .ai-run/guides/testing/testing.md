# Testing

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: test design in the plan, Rust and node tests, functional cases and the map

## Rules
- R1. A change in behaviour starts from acceptance criteria, each with a test: a Rust test, a node test over a window-script block, or a functional case. The plan's **Tests** part names the cases to add, change and remove (each existing case of the touched features checked against the new behaviour; obsolete ones removed, duplicates merged), written before the code. Evidence: `workbench.test.mjs`
- R2. A Rust test that touches the data folder or config dir takes `paths::testenv::folders(name)` first and keeps its guard alive; nothing else sets `ECHOCTL_HOME` / `ECHOCTL_CONFIG_DIR`. Evidence: `paths::testenv::folders`
- R3. Rust tests are a `#[cfg(test)] mod tests` at the end of their module, each named as the behaviour it proves (`only_this_calls_files_and_the_previous_minutes`). Evidence: `guard::only_this_calls_files_and_the_previous_minutes`
- R4. Wait for a released lock with `testenv::eventually`, never a fixed sleep. Evidence: `paths::eventually`
- R5. A functional case is one behaviour, `tests/cases/<area>/TC-NNN_<slug>.md`, with `run: agent | person` and `platform`; a scenario (`tests/scenarios/S-NN_<slug>.md`) chains cases into a journey. Rules for writing them are in `tests/app_profile.md`. Evidence: `tests/app_profile.md`
- R6. What Echo does is the functional map, `tests/feature-map/` (one JSON file per area); a case names its features in `requirements:`; a change in behaviour updates its area's file in the same PR, and `python3 tests/tools/feature-map.py check` passes. Evidence: `tests/tools/feature-map.py::check`
- R7. Cases change with behaviour: a changed behaviour updates its cases, a removed feature removes them, a bug found by hand gets a case. Run results go to `tests/reports/` (git-ignored), never into a case. Evidence: `tests/app_profile.md`
- R8. Test data is made up (`tests/fixtures/make_data.py`, acme.example-style names), never real call data, in tests, cases, reports or issues. Evidence: `tests/fixtures/make_data.py`
- R9. `scripts/check-mod.sh` is the list of mod and window checks; a new `ui/*.test.mjs` adds itself there. Evidence: `scripts/check-mod.sh`

### Read first
| When you are | Exemplar |
|---|---|
| adding a test that needs Claude Code | `ai::fake.install` |
| adding a window node test | `palette.test.mjs::Palette` |
| writing a functional case | `tests/cases/today/TC-003_task-card-opens-details.md` |
