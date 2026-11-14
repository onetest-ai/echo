# Quality Gates

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the gates and how to run them, fastest first; functional runs; the release test bar

## Gates

| Gate | Run | Pass |
|---|---|---|
| lint | `cargo clippy -- -D warnings` | exit 0 (default features, host target) |
| type-check | `cargo check --target x86_64-pc-windows-msvc --no-default-features` | exit 0 |
| test | `cargo test` | exit 0 |
| mod-test | `scripts/check-mod.sh` | exit 0; `size-check: ok`; last line `tsc: clean` or `skip tsc: …` |
| functional | `cargo build`, then the `run: agent` cases of what the change touched (R2) on fresh made-up data; `run: person` cases by a person | no regression against `main` (R4); `python3 tests/tools/feature-map.py check` 0 problems |
| autotests | before a release: `npm test` in `tests/auto` (testing R15) | exit 0 |

## Rules
- R1. Every gate but the autotests passes before a change is called done or its PR opened. Evidence: `scripts/check-mod.sh`
- R2. The functional gate runs only the touched cases: the cases of the change's features in the map and of the flows through the changed code, in minutes. A touched case inside a scenario runs with only its scenario's prelude and the stages it depends on, never the whole scenario; no full regression. Evidence: `feature-map.py::check`
- R3. Run `cargo build` right before a functional run: `cargo test` and clippy don't rebuild `target/debug/echoctl`. Evidence: `make_data.py::Data`
- R4. A functional FAIL stops the change only when it is a regression (the case passes on `main`); a failure with an open issue is listed with its number, a wrong case is fixed or listed. The PR lists the cases added, changed, removed and run, with their results. Evidence: `feature-map.py::changes`
- R5. A release passes its test bar on its version-bump commit, each FAIL checked against `main` (a regression, an open issue, or a wrong case):
  - a patch release (X.Y.Z+1): the gates, the full autotests (`tests/auto`, `npm test`) and the touched cases;
  - a minor or major release: also every agent scenario in `tests/scenarios/` (via `claude --agent test-run-lead`) and every `critical` case, agent and person-run.

  Merging a long-lived branch into `main` runs every agent scenario too, only when the user decides. Evidence: `feature-map.py::auto_report`
- R6. On Windows before a release, also run `cargo check -p echoctl --no-default-features --features tray,brain`. Evidence: `crates/echoctl/Cargo.toml::features`
- R7. `cargo clippy --fix --allow-dirty -- -D warnings` may fix lint findings; re-run the gate after.
