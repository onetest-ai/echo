---
id: TC-037
title: Without Claude Code, or without Echo's plugin, the Minutes tab says what to do instead of offering Write minutes
priority: high
type: functional
requirements: [F-MIN-16]
module: calls
run: agent
platform: macos
data: standard
ai: local
tags: [calls, minutes, claude-code, second-server]
---

# TC-037: Without Claude Code, or without Echo's plugin, the Minutes tab says what to do instead of offering Write minutes

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- This case starts two more window servers of its own on the same data, each from `target/debug/echoctl window --serve`, and stops each with `kill` of its saved pid (the only commands besides `echoctl` it runs: `mkdir`, `cp`, `nohup`, `kill`, `curl`). `<p2>` is the scenario's port + 1, `<p3>` the port + 2; both free
- The window decides from the app's own check (`/api/state` → `notes`: `claude` = a `claude` program is found on `PATH` or in the usual install folders under `$HOME`; `plugin` = Echo's Claude Code plugin is found next to the program or in Claude Code's plugin records). On this Mac `claude` lives only in `~/.local/bin`: a Mac with it in `/opt/homebrew/bin` or `/usr/local/bin` can't hide it this way (the case is then BLOCKED)
- No model run: nothing here starts Claude Code

## Test Data

| Server | How it is started | `/api/state` → `notes` | Minutes tab of Initech planning |
|---|---|---|---|
| `{{base_url}}` (the scenario's) | `serve.sh` | `claude: true`, `plugin: true` | "Write minutes" (or "Starting the minutes…" while it is due): either way not the texts below |
| `http://127.0.0.1:<p2>` | `HOME=<dir>/nohome PATH=/usr/bin:/bin` | `claude: false`, `plugin: true` | "Install it from claude.com/claude-code, then come back here." |
| `http://127.0.0.1:<p3>` | a copy of the build in `<dir>/app/x/y/echoctl` (no `plugin/` near it; the data's Claude Code plugin record has no install path) | `claude: true`, `plugin: false` | "Connect Claude Code first: menu → Claude Code → Connect Claude Code." |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `. <dir>/env; mkdir -p <dir>/nohome; HOME=<dir>/nohome PATH=/usr/bin:/bin ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve <p2> > <dir>/serve-2.log 2>&1 & echo $! > <dir>/pid-2`, wait 2 seconds, then `curl -s -c <dir>/ck2 http://127.0.0.1:<p2>/ > /dev/null; curl -s -b <dir>/ck2 http://127.0.0.1:<p2>/api/state` | The JSON's `notes` is `{"claude": false, "model": "sonnet", "plugin": true}` |
| 2 | Navigate to `http://127.0.0.1:<p2>/`, wait for "Your day", go to `#calls`, click the row `[data-call$="initech-planning"]`, tab "Minutes" | The box reads "No minutes yet", "The transcript is ready." and "Minutes and action items are written by Claude Code. Install it from claude.com/claude-code, then come back here."; the link goes to `https://claude.com/claude-code`; there is no "Write minutes" button, no Model select and no `/echo:process-call` line |
| 3 | Run `kill $(cat <dir>/pid-2)`; then `mkdir -p <dir>/app/x/y && cp target/debug/echoctl <dir>/app/x/y/echoctl && . <dir>/env && ECHOCTL_SERVE_IPC=1 nohup <dir>/app/x/y/echoctl window --serve <p3> > <dir>/serve-3.log 2>&1 & echo $! > <dir>/pid-3`, wait 3 seconds, and read `/api/state` of `<p3>` as in step 1 | `notes` is `{"claude": true, "model": "sonnet", "plugin": false}` |
| 4 | Navigate to `http://127.0.0.1:<p3>/`, wait for "Your day", open Initech planning's Minutes tab as in step 2 | The box reads "No minutes yet", "The transcript is ready." and "Connect Claude Code first: menu → Claude Code → Connect Claude Code."; no "Write minutes" button, no Model select |
| 5 | Run `kill $(cat <dir>/pid-3)`; navigate back to `{{base_url}}/`, wait for "Your day", open Initech planning's Minutes tab | Neither text of steps 2 and 4 is shown (the scenario's own server finds Claude Code and the plugin) |

## Expected Final State
Both extra servers are stopped; the scenario's server is untouched; no file of the data folder changed.

## Teardown
- `kill $(cat <dir>/pid-2) $(cat <dir>/pid-3) 2> /dev/null`; `rm -rf <dir>/app <dir>/nohome`

## Notes
- Verified 2026-11-07 on served windows: both texts exactly as above.
- The plugin text points at "menu → Claude Code → Connect Claude Code" (the tray menu); in the window the same action is Settings → General "Claude Code plugin" → Connect. Not a failure; worth a look.
- Initech planning is due for automatic minutes on this data, so the scenario's own server may show "Starting the minutes…" (`window --serve` never starts them): step 5 checks only that the two texts are gone.
