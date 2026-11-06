# Functional tests

Cases that check Echo the way a user meets it: the window, the CLI, and what only a person can try on a real machine
(permissions, real audio, Teams, Microsoft 365, Claude Code runs, installs and updates). The automated tests live next
to the code (`cargo test`, `scripts/check-mod.sh`).

| Path | What |
|---|---|
| `cases/<area>/TC-NNN_<slug>.md` | One case each, in the manual-qa format; `run: agent` (an agent runs it) or `run: manual` (a person) |
| `app_profile.md` | Echo for the test agents: rules, starting the app on test data, the data, routes, fragile areas |
| `fixtures/make-data.sh <dir>` | Builds a made-up data folder (Acme, Globex, Initech; calls, threads, tasks, today's agenda) |
| `install.sh` | Installs the manual-qa agent team (arozumenko/sdlc-skills) on this machine, pointed at this folder |
| `reports/` | Run reports and screenshots (git-ignored) |

## Running

1. Once per machine: `tests/install.sh`, then restart Claude Code in the repo (it loads the agents and Playwright MCP).
2. `cargo build`.
3. Start Claude Code with the lead as the active agent (`claude --agent test-run-lead`) and give it a suite, e.g.
   "run tests/cases/today". It builds the data, serves the window, runs each `run: agent` case, and writes
   `tests/reports/RUN-<date>-NNN.md`; `run: manual` cases are listed for a person.

Writing cases: `test-author` (or by hand) following `app_profile.md` → "Writing a case".

## Areas

| Area | Agent | Manual | What |
|---|---|---|---|
| `cases/today` | 3 | 0 | Today: Your day, My work |
| `cases/window` | 10 | 5 | Calls, minutes, transcript, notes, delete, calendar views |
| `cases/calendar` | 1 | 10 | Scheduling, Microsoft 365 calendar, meeting chat |
| `cases/cli` | 2 | 4 | `echoctl` commands |
| `cases/claude` | 0 | 15 | Claude Code plugin and minutes runs |
| `cases/privacy` | 0 | 7 | Opt-in, local data, what goes online |
| `cases/transcription` | 0 | 6 | Transcript quality and languages |
| `cases/install` | 0 | 7 | DMG, first run, data folder, models |
| `cases/permissions` | 0 | 7 | macOS permissions and Health |
| `cases/recording` | 0 | 13 | Arming, recording, stopping, interruptions |
| `cases/lifecycle` | 0 | 9 | Login, quit, single instance, updates, notifications |
| `cases/windows` | 0 | 9 | The Windows build |

## Manual cases

`run: manual` cases are run by a person on a real machine: a real Teams call (a second person or account speaking),
Microsoft 365, a signed-in Claude Code, installs and updates. `external_id` keeps each one's id from the older
scenario files. Use made-up meeting content; never paste real transcripts into a report.

### Test machines

- **Fresh Mac:** a user account that has never run Echo (no `~/Library/Application Support/echo`,
  no `~/Echo`), macOS 14.2 or later, Teams (new) installed and signed in.
- **Dev Mac:** your usual machine, for the regression run.
- **Windows:** Windows 10 2004 or later / Windows 11, Teams installed.

To reset a Mac to "fresh": quit Echo, then remove `~/Library/Application Support/echo`, the
data folder you chose, `~/Library/LaunchAgents/ai.onetest.echo.plist` if present, and reset
permissions with `tccutil reset All ai.onetest.echo`.

### Run log

Copy this block into `tests/runs/<date>-<version>-<machine>.md` (git-ignored) and fill it in, or add the manual
results to the lead's report in `tests/reports/`.

```markdown
# Run: <version> on <machine>, <date>
Tester:            macOS / Windows version:        Teams version:

| Case | Result | Notes |
|---|---|---|
| install/TC-001 | pass / fail / blocked / skipped | |
```

Attach `<data folder>/.echo/app.log` for any failure.
