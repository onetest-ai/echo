# Autotests

Echo's automated tests: the window driven headless in Chromium on made-up data, the CLI on the same data, and model
runs on the virtual model — no agent in the loop, seconds per test. Run them locally, like the functional cases; there
is no CI for them.

```bash
cd tests/auto
npm install                                   # once (Playwright 1.63.0)
npx playwright install chromium-headless-shell # once, if Playwright says its browser is missing
npm test                                       # every spec, then the coverage report
npx playwright test today.spec.mjs             # one file (no report)
```

`cargo build` first: the tests use `target/debug/echoctl`, and `--ai virtual` needs Claude Code on `PATH`.
`ECHO_AUTO_KEEP=1` keeps each file's data folder after the run (they go into the system's temporary folder).

## The report

`npm test` ends with `feature-map.py auto-report`, which writes `tests/reports/auto-<time>.md` (git-ignored):
- the run: tests passed, failed, flaky, skipped;
- each failed test with the functional cases to re-run by hand (to tell a bug from a broken test);
- coverage by area of the functional map: features an autotest reaches (passing / failing), those only agent cases
  check, only a person, or nothing;
- what each test checked, and the critical and high features no autotest reaches yet;
- what changed since the last run (`tests/reports/auto-last.json`): tests added and removed, status changes, features
  newly reached or lost, and the cases changed in git since then that a test automates (check the test still matches).

## Writing a test

- **One spec file a part of Echo** (`today.spec.mjs`, `minutes.spec.mjs`, …). `useData(test, { set, ai })` builds the
  file's own data with `serve.sh` before its first test and stops it after the last; the tests in a file run in order
  on that data.
- **Link it**: `covers('today/TC-013', 'F-TODAY-04')` as the test's details — the functional cases it automates and the
  features it checks. `feature-map.py check` fails on a case or feature that doesn't exist; the report counts a feature
  reached through either.
- **The window**: `echo.open(page, '#today')` (the cookie first). Fixed checks only — what the case's steps check;
  judgement (how it looks, others' hostile text, a real model's content) stays with agent runs.
- **The CLI and files**: `echo.cli('notes', dir, '--wait')` (exactly the data's environment), `echo.home`,
  `echo.callDir('initech-planning')`, `echo.read(…)`.
- **Model runs**: `useData(test, { ai: 'virtual' })`; `echo.model.write(script)` / `script('guard-probe.json')`,
  `echo.model.clear()`, `echo.model.log()` (the requests: what Echo gave the model, what it was told back).
- **Time**: `page.clock.install()` then `page.clock.fastForward('01:05')` instead of waiting for a minute.
