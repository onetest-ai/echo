# Claude Code integration — design

The Claude Code plugin (`plugin/`) is Echo's client inside Claude Code: skills that drive the `echoctl` CLI, and a
status-line mod. Echo never depends on Claude Code; the plugin reaches the app only through the CLI and the files in
the data folder. This doc also covers how Echo installs and checks Claude Code, and the `echoctl m365` commands its
sessions use for Microsoft 365.

Related: rules for the plugin in `.ai-run/guides/plugins/claude-code-plugin.md`; the user's install steps in
`docs/install.md`. Echo's own plugins (a different thing): `.ai-run/guides/design/plugins.md`. AI profiles, minutes
runs and the run guard belong to the AI design doc.

## Modules

| File | Role |
|---|---|
| `.claude-plugin/marketplace.json` | Marketplace `onetest` with one plugin, `echo` (source `./plugin`) |
| `plugin/.claude-plugin/plugin.json` | The plugin's manifest (`echo`, its version follows Echo's) |
| `plugin/scripts/echoctl`, `echoctl.cmd` | Launchers: find the CLI (the plugin ships no binary) |
| `plugin/skills/*/` | `manage`, `brain`, `microsoft-365`, `process-call`, `record`, `doctor`, `project-sync` (app-only) |
| `plugin/hooks/register.tsx`, `view.ts`, `plugin/types/index.d.ts` | The status-line mod, its pure helpers, its state contract (`Band`) |
| `scripts/build-plugin.sh` | Puts a dev build in `plugin/bin` (by rename) |
| `scripts/check-mod.sh` | Plugin validate + `claude plugin test` + the window's node tests + tsc |
| `crates/echoctl/src/app.rs` | `connect_claude`, `install_cli`, `add_to_user_path`, `stale_plugins` / `update_plugins`, `APP_PATH_FILE` |
| `crates/echoctl/src/claude_setup.rs` | Is Claude Code really set up: `check`, `status`, `add_to_path` |
| `crates/echoctl/src/m365.rs` | `echoctl m365`: mail, Teams, files, people on Echo's own Microsoft 365 sign-in |
| `crates/echoctl/src/notes.rs::plugin_dir` | The plugin shipped with the app, passed as `--plugin-dir` |
| `crates/echoctl/src/view.rs::howto_block` | `<echo_howto>` given to the window's Claude session, pointing at the `manage` skill |

## How it works

### Division of labour
- Anything deterministic (matching, filtering, template choice, preflight) is a CLI command (`up`, `agenda sync`,
  `context`). Skills keep only judgment (notes, titles, vocabulary picks, follow-ups). The app does everything else.

### Finding the CLI (`plugin/scripts/echoctl`)
1. `$ECHOCTL_BIN`
2. `plugin/bin/echoctl[.exe]` (dev build)
3. `<config dir>/app-path` — written at each app start, so any install folder works
4. `echoctl` on PATH
5. `/Applications/Echo.app`, `~/Applications/Echo.app`, `%LOCALAPPDATA%\Programs\echo\echoctl.exe`

On Windows Claude Code's Bash is Git Bash, so skills run the **sh** launcher there too (MINGW paths via `cygpath`).

### Skills
| Skill | For |
|---|---|
| `manage` | Clients, organisations, threads and rules, the work ledger, people, calls, mail through `deliver`, the Brain — any session |
| `brain` | Search scoped by client / thread, numbers, topics and terms, senders, insights |
| `microsoft-365` | `echoctl m365` (below) |
| `process-call` | A call's minutes: notes → actions → vocabulary → follow-ups. One file: a minutes run's guard reads only the call's folder |
| `record` | Arm / stop from chat |
| `doctor` | `echoctl up` and fix what's missing (data folder, permissions, model, languages); first run itself is the app's |
| `project-sync` | A thread's sync; run by the app only |

Descriptions follow Anthropic's skill guidance: each starts "Use when…", triggers don't overlap, SKILL.md stays short
with commands one level deep (`reference.md`, `calls-and-settings.md`).

### Connecting (`app::connect_claude`; tray / Settings → Connect, `echoctl claude connect [--scope]`)
1. Refuse a `claude` that doesn't start.
2. `claude plugin marketplace add onetest-ai/echo` if not known (`ECHOCTL_MARKETPLACE` overrides the source).
3. `claude plugin install echo@onetest --scope project` — project = the **data folder** by default, where Echo's own
   sessions run; `--scope user` for every session.
4. Check Claude Code's own list and the files; else an error naming the command to run.

"Claude Code ✓" counts a user-scope install or one whose folder is the data folder (after `home set`, Connect is
offered again). A folder session on a `claude` / `local` profile reads only its own settings, so Echo passes the
bundled plugin with `--plugin-dir` (`notes::plugin_dir`: `Echo.app/Contents/Resources/plugin`; dev: repo `plugin/`).
Older Echo plugin installs (`app::stale_plugins`, from `installed_plugins.json`) show in Health and Needs you with
Update it (`app::update_plugins`: the marketplace, then each install in its scope).

### Is Claude Code really set up (`claude_setup.rs`)
- `check()`: it starts (`claude --version`, 15 s, its own folder first on PATH — an npm install is a node script); a
  terminal finds it (the login shell's `command -v claude` between markers; Windows: the PATH); Claude Code lists
  Echo's plugin where Echo's sessions run (in `CLAUDE_CONFIG_DIR` when set).
- `status()` keeps the answer 10 min, re-checks at once after Check again / Connect / Add to PATH, recounts in the
  background. Feeds `/api/settings` `claude_code`, Health's rows, the Settings badge, General's plugin row.
- **Add to PATH** (`add_to_path`, `echoctl claude path`): one marked line, folder single-quoted, in `$ZDOTDIR/.zshrc`
  or bash's first existing `.bash_profile` / `.bash_login` / `.profile`. Windows: the user PATH as unexpanded
  `REG_EXPAND_SZ`, change broadcast (`app::add_to_user_path`). The check reads the stored PATH, not Echo's own.

### Echo's CLI on PATH (`app::install_cli`, `app install-cli`)
macOS: `/usr/local/bin/echoctl` → the app's binary (admin prompt via osascript). Windows: the app folder appended to
the user PATH (`app::add_to_user_path`).

### The status-line mod (`plugin/hooks/register.tsx`)
- Only a status line above the prompt; Echo's window shows calls, live transcripts, live notes and minutes.
- At session start it asks `echoctl app` for the data folder and starts the app if set up but not running.
- Shows **Echo** (hotkey e → `echoctl app open [--call <recording>]`) + "● Recording · title · N min" (· muted),
  "◌ Armed", or "Echo · next: …".
- Every 1 s tails `.echo/events.jsonl` from the end (never replays): muted / unmuted for the line; toasts for a failed
  transcription, an error, an expired arm.
- Asks for no minutes and never calls a model (`pending --for` / `notes --auto` stay for older mods, and refuse).
  Inside a run Echo started (`ECHOCTL_HEADLESS`) it does nothing.

### Microsoft 365 for sessions (`m365.rs`, `echoctl m365`)
- Echo's own Graph sign-in, so sessions don't get lost between msgraph skills and connectors. The registration's
  token carries every scope the tenant consented, so `graph::request` reaches mail, chats, channels, files, people.
- `search` = Graph's Search API, one request per kind (message, chatMessage, driveItem), Microsoft's ranking, KQL
  passed through. Plus `mail read|send|reply|draft`, `teams chats|read|send|list|channel|post`, `people`,
  `drive list|download|upload`, `get` (any GET).
- `teams send --to <address>` makes their 1:1 chat with `POST /chats` + `users('<address>')` (no User.Read.All).
- The skill: search → open the best hits → search again in other words → answer with links.

### Packaging (where the plugin ships)
- `scripts/package-macos.sh`: Developer ID, hardened runtime, notarized app, signed dmg; the plugin is bundled under
  `Contents/Resources/plugin`. `LOCAL=1`: ad-hoc signature, no notarization or dmg, `disable-library-validation`
  (an ad-hoc signature has no Team ID, and the hardened runtime would refuse the NeMo-Speech libraries).
- `scripts/package-windows.ps1`: per-user MSI (`packaging/windows/echo.wxs`, WiX 5) → `%LOCALAPPDATA%\Programs\echo`
  + a zip; `-Phase stage | package` leaves room for a signing step. Built only by
  `.github/workflows/windows-release.yml` on a published release, from scratch.

## Decisions

| Decision | Why | Where |
|---|---|---|
| Plugin ships no binary; launchers find the app's own CLI | One CLI per install, any install folder | `plugin/scripts/echoctl`, `app::APP_PATH_FILE` |
| Dev build placed by rename | macOS SIGKILLs a binary rewritten in place (exit 137) | `scripts/build-plugin.sh` |
| Default install scope = the data folder's project | That is where Echo's own sessions run | `app::connect_claude` |
| Bundled plugin passed with `--plugin-dir` | `claude` / `local` profiles read only their own settings | `notes::plugin_dir` |
| "Found" isn't "working": check start, terminal and plugin list | A `claude` that doesn't start can't install anything | `claude_setup::check` |
| Status line only, no model, no minutes from the mod | The window owns live views; the app owns minutes | `plugin/hooks/register.tsx` |
| Events tailed from the end | A new session must not replay old toasts | `register.tsx` (`eventsOffset`) |
| Nothing leaves without `--yes`; headless runs can't send | Without it the CLI returns `{sent: false, would}` for the user to see | `m365.rs` |
| A send whose recipient and words the user gave is their yes; anything Claude chose waits | A model never picks what goes out on its own | `plugin/skills/microsoft-365/SKILL.md` |
| `process-call` stays one file | The minutes run's guard reads only the call's folder | `plugin/skills/process-call/SKILL.md` |
| WiX 5, not later | Later versions carry a maintenance-fee EULA | `scripts/package-windows.ps1` |

## Files and state

| Path | What | Who writes |
|---|---|---|
| `<config dir>/app-path` | The running app's executable path | the app, at each start |
| `plugin/bin/echoctl[.exe]` | Dev build (not shipped) | `scripts/build-plugin.sh` |
| shell rc file (one marked line) / HKCU user PATH | Claude Code's or Echo's folder on PATH | `claude_setup::add_to_path`, `app::add_to_user_path` |

## Known gaps
- `.claude-plugin/marketplace.json` still describes the plugin as having a "live call pane", which the mod dropped.
- The Windows builds are unsigned (SmartScreen warns: More info → Run anyway) until a certificate is bought.
