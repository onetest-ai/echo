# Quality Gates — reference

The gates' commands and what passing looks like, fastest first. They change with the code and the scripts, in the same
PR. The rules are in `.ai-run/guides/quality-gates.md`.

## Gates

| Gate | Run | Pass |
|---|---|---|
| lint | `cargo clippy -- -D warnings` | exit 0 (default features, host target) |
| type-check | `cargo check --target x86_64-pc-windows-msvc --no-default-features` | exit 0 |
| test | `cargo test` | exit 0 |
| mod-test | `scripts/check-mod.sh` | exit 0; `size-check: ok`; last line `tsc: clean` or `skip tsc: …` |
| functional | `cargo build`, then the `run: agent` cases of what the change touched on fresh made-up data; `run: person` cases by a person | no regression against `main`; `python3 tests/tools/feature-map.py check` 0 problems |
| autotests | before a release: `npm test` in `tests/auto` | exit 0 |

Prerequisites for the gates: `.ai-run/reference/development/dev-workflow.md` (Prerequisites).

## Rebuilding

`cargo test` and clippy don't rebuild `target/debug/echoctl`, the binary the functional cases run; `cargo build` does.

## The release test bar's runs

- Agent scenarios: every agent scenario in `tests/scenarios/`, run through `claude --agent test-run-lead`.
- Autotests: `npm test` in `tests/auto`; `feature-map.py auto-report` turns the results into a report.

## Windows before a release

`cargo check -p echoctl --no-default-features --features tray,brain` (`crates/echoctl/Cargo.toml::features`).

## Fixing lint

`cargo clippy --fix --allow-dirty -- -D warnings` may fix lint findings; re-run the gate after.
