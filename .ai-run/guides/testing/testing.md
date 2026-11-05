# Testing

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: Rust test isolation, fakes, node tests, manual release tests

## Tests and checks

### Rules
- R0. Every change in behaviour starts from acceptance criteria and ends with them checked: each criterion has a functional test — a Rust test that proves the behaviour, a node test over an `app.js` block, or a check in the running window (`echoctl window --serve` on a copy of a data folder, never the user's real one); tests are written with the change, not on request, and run with the quality gates before it is called done. Evidence: `crates/echoctl/ui/workbench.test.mjs`
- R1. A test that touches the data folder or config dir takes `paths::testenv::folders(name)` first and keeps the guard alive; it is the only code that sets `ECHOCTL_HOME` / `ECHOCTL_CONFIG_DIR` (the variables are process-wide). Evidence: `crates/echoctl/src/paths.rs::testenv.folders`
- R2. Write Rust tests as a `#[cfg(test)] mod tests` at the end of the module they test, each named as the behaviour it proves in snake_case (`only_this_calls_files_and_the_previous_minutes`). Evidence: `crates/echoctl/src/guard.rs::only_this_calls_files_and_the_previous_minutes`
- R3. Wait for a lock just released with `testenv::eventually`, not a fixed sleep (on macOS a child process holds the descriptor until it execs). Evidence: `crates/echoctl/src/paths.rs::eventually`
- R4. Before a release run the manual cases in `tests/` (P0 at least) and keep the run log; never use real call data in tests or issues, only made-up people and companies. Evidence: `tests/README.md::P0`
- R5. `scripts/check-mod.sh` is the list of mod and window checks (plugin validate and test, every `ui/*.test.mjs` it names, tsc); read the script rather than a copy of its list, and note its tsc step prints `skip tsc` and passes when no Claude Code type declarations are found. Evidence: `scripts/check-mod.sh::node`
- R6. On Windows before a release, run the native check of the code behind `tray` and `brain` that the macOS cross-check skips (the command is in quality-gates.md, Full runs). Evidence: `crates/echoctl/Cargo.toml::features`

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a test that needs Claude Code | `crates/echoctl/src/ai.rs::fake.install` | scripted fake with status, login and `-p` replies | exemplar is a test |
| adding a window node test | `crates/echoctl/ui/palette.test.mjs::Palette` | slices one marked block of `app.js`, plain `node:assert` | exemplar is a test |
