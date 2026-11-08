# Functional tests

Cases that check Echo the way a user meets it: the window, the CLI, and what only a person can try on a real machine
(permissions, real audio, Teams, Microsoft 365, Claude Code runs, installs and updates). The automated tests live next
to the code (`cargo test`, `scripts/check-mod.sh`).

| Path | What |
|---|---|
| `cases/<area>/TC-NNN_<slug>.md` | One behaviour each (the unit of coverage), in the manual-qa format; `run: agent` (an agent runs it) or `run: person` (a person) |
| `scenarios/S-NN_<slug>.md` | A user's journey that chains cases by path, in stages: what agent runs execute |
| `feature-map/` | The functional map: `areas/<prefix>.json` (features), `axes/<entity>.json` (state axes), combinations, hostile content |
| `tools/feature-map.py check` / `render` | Checks the map and the cases' links; draws the map with coverage to `reports/feature-map.html` |
| `app_profile.md` | Echo for the test agents: rules, starting the app on test data, the data, routes, fragile areas |
| `fixtures/make-data.sh <dir>` | Builds a made-up data folder (Acme, Globex, Initech; calls, threads, tasks, today's agenda) |
| `fixtures/serve.sh up <dir> <port>` / `down <dir>` | Fresh data plus the window served on it, for one scenario |
| `fixtures/recordings.py prepare` / `copy <name>` | The known conversations recorded through TestCall and transcribed **once per build**; copies for each scenario |
| `fixtures/wait.py <seconds> <condition>` | Wait for the run's app state, an event or a file, with a timeout (no fixed `sleep`) |
| `install.sh` | Installs the manual-qa agent team (arozumenko/sdlc-skills) on this machine, pointed at this folder |
| `reports/` | Run reports and screenshots (git-ignored) |

## Running

1. Once per machine: `tests/install.sh`, then restart Claude Code in the repo (it loads the agents and Playwright MCP).
2. `cargo build`; for the recording scenarios (S-14, S-17, S-18, S-19, S-21) also `tests/fixtures/recordings.py prepare`
   once (about 4 minutes, outside any sandbox: it records with TestCall and transcribes; kept until the app changes).
3. Start Claude Code with the lead as the active agent (`claude --agent test-run-lead`) and give it scenarios, e.g.
   "run tests/scenarios". Per scenario it serves the window on fresh data, one runner goes through its stages (the
   browser stays open between scenarios), and the report goes to `tests/reports/RUN-<date>-NNN.md`, counting
   scenarios and their stages, each stage's time and the five slowest; `run: person` cases are listed for a person.

Writing cases: `test-author` (or by hand) following `app_profile.md` → "Writing a case"; then put each agent case
into a scenario (`app_profile.md` → "Scenarios").

## Scenarios

| Scenario | Stages | Journey |
|---|---|---|
| [S-01](scenarios/S-01_review-a-client-call.md) | 7 | Review a client call: find it, read the minutes, check a point, note a follow-up |
| [S-02](scenarios/S-02_plan-the-afternoon.md) | 5 | Plan the afternoon: Today, the calendar, set a meeting to record and take it back |
| [S-03](scenarios/S-03_tidy-the-calls.md) | 2 | Tidy the calls: a call without minutes, delete one, restore it |
| [S-04](scenarios/S-04_the-cli-on-the-data.md) | 2 | The CLI on the same data |
| [S-05](scenarios/S-05_start-and-run-a-thread.md) | 17 | Start and run a thread: create it, brief, rules, calls, rename, stage, close |
| [S-06](scenarios/S-06_shape-the-work.md) | 17 | Shape the work: board, tree, edit, undo, link, merge and reject a proposal |
| [S-07](scenarios/S-07_clients-and-people.md) | 17 | Clients and people: client pages, domains, a thread's people, a person's details |
| [S-08](scenarios/S-08_who-owes-what.md) | 14 | Who owes what: the piles, the filters, a card, closing a task, Claude's dialog, Asks |
| [S-09](scenarios/S-09_a-calls-housekeeping.md) | 11 | A call's housekeeping: list, minutes, kind, thread, tabs and the actions menu |
| [S-10](scenarios/S-10_inbox-and-prep-on-a-quiet-day.md) | 9 | Inbox and prep on a quiet day: nothing waits, meetings still to come, then a day with nothing ("A quiet day") |
| [S-11](scenarios/S-11_set-echo-up-your-way.md) | 36 | Set Echo up your way: appearance, recording, languages, About me, a template, a meeting type, the vocabulary |
| [S-12](scenarios/S-12_skills-and-ai-profiles-and-plugins.md) | 30 | Skills and AI: skills on and off, AI profiles and the default, plugins and marketplaces |
| [S-13](scenarios/S-13_find-your-way-around.md) | 18 | Find your way around: the activity bar, panels, layout, the actions palette, the status bar and shortcuts |
| [S-17](scenarios/S-17_a-recorded-call-end-to-end.md) | 11 | A recorded call end to end with the test call: two tracks, record the next call, notes while recording, no audio kept, the window updating, a pinned language, a very short call, minutes started by themselves at the delay (and later after a note), the minutes in the browser and Finder (`small --ai fake --clock fast`, the prepared recordings; 1 person stage: the tray) |
| [S-18](scenarios/S-18_recording-off-the-happy-path.md) | 23 | Recording off the happy path with the test call: mid-call, cancel, expiry, stop from the terminal and the pill, Ctrl-C, a crash, silence, two voices, opt-in only, only the call app, a drop and rejoin, Echo's "Record?" dialog (`small --ai fake`; 9 person stages: devices, speakers, mute, sleep, an hour-long call, permissions denied, shared screens) |
| [S-19](scenarios/S-19_claude-code-and-the-minutes.md) | 13 | Claude Code and the minutes on the local model: write now and for a short call, your notes in them, a session's claim, the minutes folder, a wrong model, doctor, automatic minutes off, catch-up after a quit with what an automatic run writes (`small --ai local --clock fast`, the prepared recordings; 5 person stages: a signed-in session) |
| [S-21](scenarios/S-21_living-with-echo.md) | 12 | Living with Echo: one instance, quit and back, another data folder, the window following the system's appearance (`small --ai fake`; 8 person stages: login, `app start`, the native window, updates, offline, the network) |
| [S-23](scenarios/S-23_a-busy-day-of-calls-and-minutes-in-every-state.md) | 27 | A busy day of calls and minutes in every state: icons, countdown and Not yet, memo and Teams import, chat, screens, live notes, what moved, failed and waiting runs, the prep tab, delete and restore (`standard` data) |
| [S-24](scenarios/S-24_the-calendar-preps-and-the-inbox-on-a-busy-day.md) | 33 | The calendar, preps and the Inbox on a busy day: block and card states, responses, series, preps with every section, the Inbox's badge, groups, folds, buttons and decisions (`standard` data) |
| [S-25](scenarios/S-25_who-owes-what-on-a-full-board.md) | 21 | Who owes what on a full board: Tasks, piles, filters, badges, the board and its columns (`standard` data) |
| [S-26](scenarios/S-26_ten-threads-in-every-stage.md) | 24 | Ten threads in every stage: membership, people and clients (`standard` data) |
| [S-27](scenarios/S-27_others-words-everywhere.md) | 16 | Others' words everywhere: nothing runs, nothing breaks (the `hostile` data set: markup, script, `javascript:` links, long words, right-to-left, emoji, Markdown in every place) |
| [S-28](scenarios/S-28_echo-from-the-terminal.md) | 11 | Echo from the terminal: help, threads and candidates against the window, `read` and `echo://` links in the window, `m365` without `--yes`, series and settings, clients and organisations, a thread made and changed, the work's edits, one call's commands (`standard` data) |
| [S-29](scenarios/S-29_notes-transcripts-settings-and-today-off-the-happy-path.md) | 10 | Notes, transcripts, settings and Today off the happy path: a Teams import, empty transcripts, to-dos, edits, comments, notes not lost, unreadable and changed files, a broken config, Today's cards and ⌘K (`standard` data) |
| [S-30](scenarios/S-30_minutes-and-the-work-on-a-local-model.md) | 8 | Minutes and the work on a local model: no Claude Code or no plugin, a template by title, write again with another, a memo, a prep, Yolo, a thread's sync that keeps your fields, a call's profile (`standard` data, `--ai local`: real Claude Code runs) |
| [S-31](scenarios/S-31_a-real-minutes-run-on-others-words.md) | 3 | A real minutes run on others' words: only its call's folder changes, nothing runs in the page, orders in the transcript and settings that allow everything change nothing (`hostile` data, `--ai local`) |

## Areas

| Area | Agent | Person | What |
|---|---|---|---|
| `cases/today` | 6 | 0 | Today: Your day, My work |
| `cases/window` | 21 | 1 | Calls, minutes, transcript, notes, delete, calendar views |
| `cases/calendar` | 1 | 10 | Scheduling, Microsoft 365 calendar, meeting chat |
| `cases/cli` | 15 | 1 | `echoctl` commands |
| `cases/claude` | 13 | 4 | Claude Code plugin and minutes runs |
| `cases/privacy` | 3 | 4 | Opt-in, local data, what goes online |
| `cases/transcription` | 5 | 1 | Transcript quality and languages |
| `cases/install` | 0 | 7 | DMG, first run, data folder, models |
| `cases/permissions` | 0 | 7 | macOS permissions and Health |
| `cases/recording` | 14 | 8 | Arming, recording, stopping, interruptions |
| `cases/lifecycle` | 5 | 4 | Login, quit, single instance, updates, notifications |
| `cases/windows` | 0 | 9 | The Windows build |

## Person-run cases

`run: person` cases are run by a person on a real machine: a real Teams call (a second person or account speaking),
Microsoft 365, a signed-in Claude Code, installs and updates. `external_id` keeps each one's id from the older
scenario files. Use made-up meeting content; never paste real transcripts into a report.

### Test machines

- **Fresh Mac:** a Mac where Echo has never been (nothing of Echo's in the account: `tests/tools/clean-mac.sh`
  finds nothing), macOS 14.2 or later, Teams (new) installed and signed in. S-15 and S-16 run there as
  `run: agent` with user steps (`tests/app_profile.md` → Clean-install runs).
- **Dev Mac:** your usual machine, for the regression run.
- **Windows:** Windows 10 2004 or later / Windows 11, Teams installed.

To reset a Mac to "fresh": `tests/tools/clean-mac.sh` shows what Echo left, `--yes --app` removes it (the app,
its config and data folders, the LaunchAgent, the Claude Code plugin, caches); then run the `tccutil reset` lines
(and the `sudo rm` of the CLI link) it prints.

### Run log

Copy this block into `tests/runs/<date>-<version>-<machine>.md` (git-ignored) and fill it in, or add the person-run
results to the lead's report in `tests/reports/`.

```markdown
# Run: <version> on <machine>, <date>
Tester:            macOS / Windows version:        Teams version:

| Case | Result | Notes |
|---|---|---|
| install/TC-001 | pass / fail / blocked / skipped | |
```

Attach `<data folder>/.echo/app.log` for any failure.
