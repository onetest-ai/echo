# Manual test cases

Scenarios a person runs by hand before a release, on a real machine with a real Teams call.
The automated tests live next to the code (`cargo test`, `scripts/check-mod.sh`); these cover
what they can't: permissions, real audio, Microsoft 365, Claude Code, installs and updates.

| File | Area |
|---|---|
| [01-install-first-run.md](01-install-first-run.md) | Download, Gatekeeper, first launch |
| [02-permissions.md](02-permissions.md) | Microphone, System Audio Recording, Accessibility, Screen Recording |
| [03-recording.md](03-recording.md) | Arming, recording, stopping, interruptions |
| [04-transcription.md](04-transcription.md) | Transcript quality, languages, audio removal |
| [05-window.md](05-window.md) | The Echo window: calls, tabs, calendar views |
| [06-calendar-chat.md](06-calendar-chat.md) | Microsoft 365 sign-in, scheduled recordings, meeting chat |
| [07-claude-plugin.md](07-claude-plugin.md) | Claude Code plugin: setup, notes, live pane, Write notes |
| [08-cli.md](08-cli.md) | `echoctl` on the PATH |
| [09-lifecycle-updates.md](09-lifecycle-updates.md) | Start at login, quit, single instance, updates |
| [10-windows.md](10-windows.md) | Windows build |
| [11-privacy.md](11-privacy.md) | Opt-in, local data, nothing kept that shouldn't be |

## Format

Each case has an ID (`TC-<AREA>-<NNN>`), a priority and the platform it applies to.

- **P0**: blocks the release. Install, permissions, recording, transcript, privacy.
- **P1**: should work for the beta. Window, calendar, chat, plugin.
- **P2**: nice to verify. Edge cases, recovery, cosmetics.

Steps are numbered; **Expected** lists what must be true. A case passes only if every expected
line holds. Use made-up meeting content where you can; never paste real transcripts into a
report.

## Test machines

- **Fresh Mac:** a user account that has never run Echo (no `~/Library/Application Support/echo`,
  no `~/Echo`), macOS 14.2 or later, Teams (new) installed and signed in.
- **Dev Mac:** your usual machine, for the regression run.
- **Windows:** Windows 10 2004 or later / Windows 11, Teams installed.

To reset a Mac to "fresh": quit Echo, then remove `~/Library/Application Support/echo`, the
data folder you chose, `~/Library/LaunchAgents/ai.onetest.echo.plist` if present, and reset
permissions with `tccutil reset All ai.onetest.echo`.

## Run log

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
