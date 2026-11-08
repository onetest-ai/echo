# Quality Gates

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: the gates and how to run them, fastest first

## Gates

| Gate | Run | Pass |
|---|---|---|
| lint | `cargo clippy -- -D warnings` | exit 0 (default features, host target) |
| type-check | `cargo check --target x86_64-pc-windows-msvc --no-default-features` | exit 0 |
| test | `cargo test` | exit 0 |
| mod-test | `scripts/check-mod.sh` | exit 0; `size-check: ok`; last line `tsc: clean` or `skip tsc: …` |
| functional | the `run: agent` cases of what the change touched (its features in `tests/feature-map/`, the flows through the changed code) on fresh made-up data; `run: person` cases by a person | no regression against `main`; `python3 tests/tools/feature-map.py check` 0 problems; a failure with an open issue listed with its number |

## Rules
- R1. Every gate passes before a change is called done or its PR opened; the functional gate runs only the touched cases (minutes, not scenarios). Evidence: `scripts/check-mod.sh`
- R2. A full regression (every agent scenario in `tests/scenarios/`, via `claude --agent test-run-lead`) runs only when merging a long-lived branch into `main` or for a release, when the user decides. Evidence: `tests/install.sh`
- R3. On Windows before a release, also run `cargo check -p echoctl --no-default-features --features tray,brain`. Evidence: `crates/echoctl/Cargo.toml::features`
- R4. `cargo clippy --fix --allow-dirty -- -D warnings` may fix lint findings; re-run the gate after. Evidence: `cargo clippy --fix`
