# Quality Gates

**Project**: Echo · **Scope**: when the gates run, the functional runs, the release test bar

**Reference**: `.ai-run/reference/quality/quality-gates.md` — each gate's command and what passing looks like
(`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Rules
- R1. Every gate but the autotests passes before a change is called done or its PR opened. Evidence: `scripts/check-mod.sh` (ref: Gates)
- R2. The functional gate runs only the touched cases: the cases of the change's features in the map and of the flows through the changed code, in minutes. A touched case inside a scenario runs with only its scenario's prelude and the stages it depends on, never the whole scenario; no full regression. Evidence: `feature-map.py::check`
- R3. Rebuild right before a functional run. Evidence: `make_data.py::Data` (ref: Rebuilding)
- R4. A functional FAIL stops the change only when it is a regression (the case passes on `main`); a failure with an open issue is listed with its number, a wrong case is fixed or listed. The PR lists the cases added, changed, removed and run, with their results. Evidence: `feature-map.py::changes`
- R5. A release passes its test bar on its version-bump commit, each FAIL checked against `main` (a regression, an open issue, or a wrong case):
  - a patch release (X.Y.Z+1): the gates, the full autotests and the touched cases;
  - a minor or major release: also every agent scenario and every `critical` case, agent and person-run.

  Merging a long-lived branch into `main` runs every agent scenario too, only when the user decides. Evidence: `feature-map.py::auto_report` (ref: The release test bar's runs)
- R6. Windows is checked on every PR, not only on Windows: the Windows type-check without default features (`cargo check -p echoctl --target x86_64-pc-windows-msvc --no-default-features`) is one of R1's gates. Before a release, on a Windows computer, also `cargo check -p echoctl --no-default-features --features tray,brain`. Evidence: `crates/echoctl/Cargo.toml::features` (ref: Gates, Windows before a release)
