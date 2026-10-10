# Windows: what's missing

The macOS app is the reference. On Windows the app builds, records (WASAPI process loopback on
`ms-teams.exe` + mic), transcribes, shows the tray and the Echo window, and talks to Microsoft
365. What follows is either not built yet or never run on a real Windows machine. Build and
test on Windows itself: `cargo build --release -p echoctl` (with whisper.cpp), then
`scripts\package-windows.ps1`, and the cases in `tests/10-windows.md`.

Code markers: `git grep "TODO(windows)"`.

## Not built yet

1. **Notifications** — `app::notify` (`app.rs`) does nothing on Windows, so "Transcript ready",
   "Notes ready", "Couldn't write notes", "Update available" never show. Wanted: a Windows
   toast (WinRT `ToastNotificationManager`) from the app with Echo's name and icon, and a click
   that opens the Echo window (`app::OPEN_WINDOW`), like macOS's UserNotifications in
   `app::notifications`. Unpackaged apps need an AppUserModelID (set on a Start-menu shortcut
   or registered under `HKCU\Software\Classes\AppUserModelId\<id>`); the tray's balloon is the
   fallback.
2. **Call detection** — `audio::app_in_call` returns `None` (`audio/mod.rs`), so arming starts
   recording at once and Stop ends it; no `call_started`/`call_ended`, no auto-stop 10 s after
   the call, no "Record?" prompt for unscheduled calls. Wanted: the WASAPI capture sessions
   owned by `ms-teams*.exe` (Teams holds the mic during a call), same 2 s release debounce as
   macOS. Also `audio/mod.rs:446`: the endpoint id of that session for the mic track.
3. **Mute sync** — `watch.rs` (`start_mute_sync`): UI Automation on Teams' microphone button
   ("Unmute" label ⇒ muted) → `audio::mic::MUTED`, events `mic_muted`/`mic_unmuted`.
4. **Teams UI capture** (`ui.jsonl`, participant names) — `watch.rs` (`start_ui_capture`): UI
   Automation text of the Teams window, the same diff format as `ax::UiRecorder`.
5. **Screen frames** (`config.capture_frames`) — `watch.rs` (`start_frame_capture`):
   Windows.Graphics.Capture on the Teams window, same keep rules as `frames.rs`.
6. **Tray icon in dark mode** — idle is drawn black (`tray.rs`, `icon_for`); it should be white
   on a dark taskbar (read `AppsUseLightTheme` / `SystemUsesLightTheme`).
7. **Signed builds and updates** — the zip is unsigned, so `update.rs` opens the release page
   instead of installing. Needs code signing, then a verified swap like macOS's.

## Built but never run on Windows

- **Write notes** (`notes.rs`): runs `claude` directly (no login shell, `cfg(unix)` only), so the
  user's env is whatever the app inherited. Check: `claude` is found (`claude.exe` / `claude.cmd`
  from npm, `app::find_program`), `--plugin-dir` points at the plugin next to `echoctl.exe`,
  the `//C:/…` permission-rule paths match Claude Code's Windows path format, the run's
  `PATH` puts `echoctl.exe` first, Show log opens `claude.log` in Notepad, and the signed-out
  and bad-model messages appear (TC-CLA-009 … 013).
- **Plugin script** `plugin/scripts/echoctl.cmd` and the mod (`plugin/hooks/register.tsx`):
  finding the CLI, starting the app, the live pane.
- **Dialogs** (WScript popup, PowerShell InputBox for Notes model and other text prompts) and
  the Echo window on WebView2 (`http://echo.localhost`).
- **Autostart** (HKCU Run key), **install-cli** (user PATH), single instance (`app.lock`).
- **Microsoft 365** sign-in (loopback redirect), calendar, chat capture.
