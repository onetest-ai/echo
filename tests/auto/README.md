# Autotests

Echo's automated tests: the window driven headless in Chromium on made-up data, the CLI on the same data, and model
runs on the virtual model — no agent in the loop, seconds per test. They automate the **manual scenarios**
(`tests/scenarios/`): one spec a scenario, its stages with fixed checks in order on the scenario's data. Run them
locally, like the functional cases; there is no CI for them.

```bash
cd tests/auto
npm install                                    # once (Playwright 1.63.0)
npx playwright install chromium-headless-shell # once, if Playwright says its browser is missing
npm test                                       # every scenario, then the coverage report
npx playwright test S-24                       # one scenario (no report)
```

`cargo build` first: the tests use `target/debug/echoctl`, and `--ai virtual` needs Claude Code on `PATH`. A spec with a failed test keeps its data folder and prints its path (its runs' `claude.log`, the model's
`model/model-log.jsonl` and `serve.log`); `ECHO_AUTO_KEEP=1` keeps every one. S-14 and S-19 use
the recordings prepared once a build (`tests/fixtures/recordings.py prepare`); when this build (the last commit that
changed the app) has none, whichever of them runs first prepares them (`support/recordings.mjs`) — a few minutes, the
one real capture and transcription. The speech models come only from the folder `ECHOCTL_TEST_MODELS=<folder>` names
(linked, never changed), never the installed Echo's: without it nothing is prepared, and the stages that need a
recording skip, saying so. The data
is built between 06:00 and 22:50 only (`make_data.py`'s rule); outside those hours every spec fails at its setup with
that reason. `ECHO_AUTO_KEEP=1` keeps each spec's data folder (in the system's temporary folder).

## Layout

| Folder | What's in it |
|---|---|
| `specs/` | One file a manual scenario: `S-24.spec.mjs` automates S-24. Each test is a stage, numbered as the scenario numbers it (`13 · …`); a test beside the scenario's stages says so (`after · …`). |
| `pages/` | Page objects, one a view: `AppPage` (opening, the status bar), `TodayPage`, `CallsPage`, `MeetingPage`, `WorkPage`. Every locator lives here, never in a spec. |
| `data/` | The names, titles, ids and expected values of each data set (`small.json`, `standard.json`: what `tests/fixtures/make_data.py` builds, read as `data.set`), and the app's own wording the tests check (`texts.json`, read as `data.text`). Nothing is spelled in a spec: change these with the fixture or the app. |
| `support/` | `aria.mjs` (what an accessibility snapshot reads, in order: for page objects), `recordings.mjs` (the recordings prepared once a build, for the specs that use them), `data.mjs` (the spec's data: built before its first test, stopped after its last; the CLI; the virtual model's script and log) and `covers.mjs` (the links to the manual side). |

## Locators

What a person reads, from the accessibility tree: `getByRole('dialog', { name: 'A task of yours' })`,
`getByLabel('What to do')`, `getByRole('listitem', { name: 'Mail to you' })`, `getByRole('tab', { name: 'Minutes' })`.
No CSS classes or `data-*` attributes. When the window gives an element no name, give it one in the app (a role, an
`aria-label`) — it helps screen readers too — rather than reaching for a selector.

## The map to the manual side

- A spec names the scenario it automates: `scenario(test, 'S-24', 'The calendar, preps and the Inbox on a busy day', () => { … })`.
- Each test names the cases it automates and the features it checks: `covers('today/TC-013', 'F-TODAY-04')`.
- The scenario and each case name the spec back: `auto: [tests/auto/specs/S-24.spec.mjs]` in their frontmatter.

`python3 tests/tools/feature-map.py check` fails when the two sides disagree (a test naming a case that doesn't name it
back, an `auto:` naming a spec that doesn't cover it). So when a case or a scenario changes, its `auto:` says which spec
to update; when a spec changes, its `covers` and `scenario` say which manual cases it stands for.

## The report

`npm test` ends with `feature-map.py auto-report`, which writes `tests/reports/auto-<time>.md` (git-ignored):
- the run: tests passed, failed, flaky, skipped; each failed test with the cases to re-run by hand;
- coverage by area of the functional map: features an autotest reaches (passing / failing), only agent cases, only a
  person, or nothing;
- the map: each scenario with its spec and how many of its cases are automated; each case with its tests and their status;
- what's left: the critical and high features no autotest reaches yet;
- since the last run (`tests/reports/auto-last.json`): tests added and removed, status changes, features newly reached
  or lost, and the automated cases and scenarios changed in git since then (check their tests still match).

## Writing a stage

- **Data**: `const data = useData(test, { set: 'standard' })` once a spec; `data.set` holds its names (`data/<set>.json`).
- **The window**: a page object (`new TodayPage(page, data)`), its methods and locators; fixed checks only — what the
  case's steps check. Judgement (how it looks, others' hostile text, a real model's content) stays with agent runs.
- **The CLI and files**: `data.cli('notes', dir, '--wait')` (exactly the data's environment), `data.home`,
  `data.callDir('initech-planning')`, `data.json(…)`, `data.lastEvent(thread)`.
- **Model runs**: `useData(test, { ai: 'virtual' })`; `data.model.use('guard-probe.json', { OTHER_CALL: … })`,
  `data.model.clear()`, `data.model.turns(rule)`, `data.model.toolResults()`.
- **Time**: `page.installClock()` then `page.minutePasses()` instead of waiting for a minute.
