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
| `NN-*.md` | The older manual scenarios, being rewritten into `cases/` (a rewritten file goes) |

## Running

1. Once per machine: `tests/install.sh`, then restart Claude Code in the repo (it loads the agents and Playwright MCP).
2. `cargo build`.
3. Start Claude Code with the lead as the active agent (`claude --agent test-run-lead`) and give it a suite, e.g.
   "run tests/cases/today". It builds the data, serves the window, runs each `run: agent` case, and writes
   `tests/reports/RUN-<date>-NNN.md`; `run: manual` cases are listed for a person.

Writing cases: `test-author` (or by hand) following `app_profile.md` → "Writing a case".

## The older scenarios

Run by hand before a release, on a real machine with a real Teams call.

| File | Area |
|---|---|
| [01-install-first-run.md](01-install-first-run.md) | Download, Gatekeeper, first launch |
| [02-permissions.md](02-permissions.md) | Microphone, System Audio Recording, Accessibility, Screen Recording |
| [03-recording.md](03-recording.md) | Arming, recording, stopping, interruptions |
| [04-transcription.md](04-transcription.md) | Transcript quality, languages, audio removal |
| [09-lifecycle-updates.md](09-lifecycle-updates.md) | Start at login, quit, single instance, updates |
| [10-windows.md](10-windows.md) | Windows build |
| [11-privacy.md](11-privacy.md) | Opt-in, local data, nothing kept that shouldn't be |


### Format

Each case has an ID (`TC-<AREA>-<NNN>`), a priority and the platform it applies to.

- **P0**: blocks the release. Install, permissions, recording, transcript, privacy.
- **P1**: should work for the beta. Window, calendar, chat, plugin.
- **P2**: nice to verify. Edge cases, recovery, cosmetics.

Steps are numbered; **Expected** lists what must be true. A case passes only if every expected
line holds. Use made-up meeting content where you can; never paste real transcripts into a
report.

### Test machines

- **Fresh Mac:** a user account that has never run Echo (no `~/Library/Application Support/echo`,
  no `~/Echo`), macOS 14.2 or later, Teams (new) installed and signed in.
- **Dev Mac:** your usual machine, for the regression run.
- **Windows:** Windows 10 2004 or later / Windows 11, Teams installed.

To reset a Mac to "fresh": quit Echo, then remove `~/Library/Application Support/echo`, the
data folder you chose, `~/Library/LaunchAgents/ai.onetest.echo.plist` if present, and reset
permissions with `tccutil reset All ai.onetest.echo`.

### Run log

Copy this block into `tests/runs/<date>-<version>-<machine>.md` (git-ignored if it contains
anything real) and fill it in.

```markdown
# Run: <version> on <machine>, <date>
Tester:            macOS / Windows version:        Teams version:

| Case | Result | Notes |
|---|---|---|
| TC-INST-001 | pass / fail / blocked / skipped | |
```

Attach `<data folder>/.echo/app.log` for any failure.
