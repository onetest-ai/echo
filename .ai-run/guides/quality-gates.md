# Quality Gates

Run order is fastest-to-slowest.

## Full runs

| Command | Covers | Source |
|---|---|---|
| `cargo check -p echoctl --no-default-features --features tray,brain` | Windows-native check of code behind tray and brain (on Windows, before a release) | Windows release check |

---

### lint

**Run**: `cargo clippy -- -D warnings`
**Side effects**: writes target/; compiles whisper.cpp (cmake), bundled SQLite and QuickJS
**Pass**: exit code 0: clippy's default lint set reports no warning and no error on the echoctl crate built with its default features (transcribe, tray, brain, js) for the host target
**Fail**: non-zero exit (cargo exits 101): any clippy lint or rustc warning, promoted to an error by -D warnings, or any compile error, including a failed whisper.cpp / bundled SQLite / QuickJS C/C++ build
**Auto-fix**: cargo clippy --fix --allow-dirty -- -D warnings

### type-check

**Run**: `cargo check --target x86_64-pc-windows-msvc --no-default-features`
**Side effects**: writes target/x86_64-pc-windows-msvc/
**Pass**: exit 0
**Fail**: non-zero exit (compile error for the Windows target)

### test

**Run**: `cargo test`
**Side effects**: writes target/; creates and removes temp folders under the OS temp dir
**Pass**: exit 0
**Fail**: non-zero exit (a failing test or build error)

### mod-test

**Run**: `scripts/check-mod.sh`
**Side effects**: npx may download typescript@5.6.3; leaves a temp folder with a tsconfig.json
**Pass**: exit 0 and the last line `tsc: clean`, or `skip tsc: …` when no type declarations exist
**Fail**: non-zero exit from any step
**Skip if**: the tsc step only skips itself when no Claude Code type declarations are found

### functional

**Run**: for a change or fix, the `run: agent` cases of what it touched — the cases of its features in `tests/feature-map/` and of the flows through the changed code — on fresh made-up data (a headless check, a script, or the cases one by one; `run: person` cases by a person). For a merge of a long-lived branch into `main` or a release, when the user decides: every agent scenario in `tests/scenarios/` (`claude --agent test-run-lead`, "run tests/scenarios/S-01… S-03"; once per machine `tests/install.sh`)
**Side effects**: builds a made-up data folder under the OS temp dir, serves the window on a localhost port for the run, writes `tests/reports/` (git-ignored)
**Pass**: no regression (a case that passes on `main` fails here), and `python3 tests/tools/feature-map.py check` reports 0 problems; a failure with an open issue is listed with its number, a wrong case is fixed or listed (a `run: person` case is listed, not run, unless a person ran it)
**Fail**: a regression, or a case of the change itself failing
**When**: mandatory — before a change or fix is called done or its PR opened (minutes, the touched cases only); the full run before merging a long-lived branch into `main` or a release, when the user decides — `.ai-run/guides/testing/testing.md` R4
