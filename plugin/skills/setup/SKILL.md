---
name: setup
description: Set up Echo — install the echoctl app (menu-bar/tray recorder), choose the data folder, languages and transcription model, start it at login, and get the OS permissions it needs (microphone, system audio, accessibility on macOS; microphone privacy on Windows). Use on first run, when the app is missing or not running, when a recording comes out silent, or when the user asks why capture isn't working.
---

# Echo setup

Echo is two parts: the **app** (menu-bar icon on macOS, tray icon on Windows) records and
transcribes on its own, with or without Claude Code; this plugin is its Claude Code client
(live pane, notes, follow-ups). CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below:
`echoctl`) — the installed app's own executable.

1. **App installed?** Run `echoctl app`. Exit code 127 means it isn't installed: tell the user
   to install it — macOS: open `Echo-<version>.dmg` and drag Echo to Applications;
   Windows: unzip to `%LOCALAPPDATA%\Programs\echo`. Stop until they confirm, then retry.
2. **Data folder.** If `home_source` is `default`, nothing was chosen yet. Ask where calls and
   notes should live (offer `~/Echo`, the default; any folder works). Then
   `echoctl home set <path>` (or `echoctl home choose` for a native picker). Opening the app
   for the first time asks the same question in a dialog.
3. **Start at login** (recommended): `echoctl app autostart on`. Then `echoctl app start`.
   Optional: `echoctl app install-cli` puts `echoctl` on the PATH for the user's own terminal
   (macOS asks for an administrator password; the plugin itself doesn't need it).
4. **Permissions** — they belong to the app (Echo), not to the terminal. Run
   `echoctl doctor --json`; with the app running, the app checks in its own process
   (`checked_by: "app"`).
   - If `os_supported` is false, tell the user the requirement (`os_requirement`) and stop.
   - For each check not `granted`, explain `why` in one sentence. If any required check is
     `not_determined`, run `echoctl doctor --request` and tell the user to click **Allow** on
     each dialog (same as the tray's Health → Ask for permissions). If `denied`, run
     `echoctl doctor --open <id>` and walk them through `how`.
   - Accessibility is optional (mute sync, speaker names); after enabling it the app must be
     restarted: `echoctl app quit && echoctl app start`.
   - Repeat `echoctl doctor --json` until `ready`. On macOS finish with `echoctl doctor
     --probe` while some audio plays: the only reliable proof System Audio Recording works.
5. **Languages.** Ask which languages the calls are in and save: `echoctl config --languages
   en,uk,ru`. Detection is restricted to this list (stops Ukrainian/Russian mix-ups).
6. **Transcription model** (skip if `echoctl model list --json` shows an `active` model).
   Present the `recommended` one (with `reason`) plus one more accurate and one smaller, each
   with `size_mb` and `summary`. **Don't download before the user confirms** (0.3–1 GB). Then
   `echoctl model download <name>` (resumable; it becomes active). Models are shared by all
   data folders. Switch with `echoctl model use <name>`, free space with `echoctl model
   remove <name>` (not the active one) — or the menu-bar icon's **Speech models** menu.
7. **Calendar** (optional, recommended): `echoctl calendar login` opens the browser for a
   read-only Microsoft 365 sign-in (or menu → Calendar → Connect Microsoft 365). It powers
   Upcoming, Join & record, attendee names and recaps, and fills in the user's name/email. The
   default app registration is CodeMie (Contoso); in another org their IT registers a public
   client (redirect `http://localhost`, delegated `Calendars.Read`) and you set
   `echoctl config --graph-client-id <id>`. Check with `echoctl calendar events`.
   Otherwise `echoctl config --me-name "<name>" --me-email <email>` (colleagues vs external).
8. **Where notes are written.** By default any Claude Code session with this plugin writes the
   notes for new calls (one session per call). If the user wants that only in a dedicated
   folder (so coding sessions stay clean): `echoctl config --notes-in <folder>` (`any` to undo;
   also in the menu: Claude Code ▸ Write call notes in).

Installing this plugin elsewhere: `echoctl claude connect --scope user|project|local
[--project <repo>]` (adds the `echoctl` marketplace and installs the plugin; the menu's
**Claude Code ▸ Connect Claude Code** does the same at user scope).

Never record during setup. Never toggle permissions on the user's behalf — only open the page
and explain.
