# Windows: what's missing

The macOS app is the reference. On Windows the app builds, records (WASAPI process loopback on
`ms-teams.exe` + mic), transcribes, shows the tray and the Echo window, and talks to Microsoft
365. What follows is either not built yet or never run on a real Windows machine. Build and
test on Windows itself: `cargo build --release -p echoctl` (with whisper.cpp), then
`scripts\package-windows.ps1`, and the cases in `tests/10-windows.md`.

Code markers: `git grep "TODO(windows)"`.

## Not built yet

1. **Signed builds** — the MSI and zip are unsigned (SmartScreen says "unknown publisher").
   Updates install without it (the MSI is checked against the release's SHA-256); signing would
   add a publisher check on top (Azure Artifact Signing or an OV certificate + `signtool`).

## Known limits (follow-ups)

- **Teams' UI language** (both platforms): share detection works in any language (the `popout-content-button`
  id), but the presenter's name (`shared_by`, from "Content shared by <name>") and
  detecting that *you* present ("Stop sharing") read English labels. Check a non-English Teams.
- **Frames while you present:** nothing is captured — the call window is a compact view and
  Teams doesn't say which window you share.
- **Transcription during the call** (`live.rs`): a CPU-only build decodes slower than real time
  (11 s test call: CPU 49 s, Vulkan 12 s), so the worker falls behind during long calls. Nothing
  is lost (it reads the WAVs on disk) and it finishes after the call, but it competes with the
  call for the CPU meanwhile (4 threads). Measure on a real call; maybe a lighter model on CPU.
- **Transcription server** (`remote.rs`): a CPU laptop can send its utterances to another
  computer's Echo (Settings → Connect to a transcription server…). Untested on Windows:
  - the Windows Firewall prompt when a Windows Echo serves (the server binds 0.0.0.0:47800);
  - `app::message` (a WScript popup);
  - ring and rustls building with MSVC in the release build.
- **One call-window reader:** frames and `ui.jsonl` each keep their own `ax::CallWindow` (on
  their own threads; UI Automation objects are thread-bound), so the call window's tree is
  read for both. Sharing one needs a single UIA thread serving both.

## Built, needs a live Teams call to confirm

- **Call detection** (`audio/windows.rs`): an active WASAPI capture session owned by the Teams
  process tree ⇒ in a call; its endpoint is the mic track's device. `echoctl procs` shows the
  sessions. Unknown: does Teams keep the session active while muted / on hold?
- **Mute sync** and **Teams UI text** (`ax/windows.rs`, UI Automation): the Teams windows via
  `EnumWindows` (asking UIA for the desktop's children waited ~6 s on a hung window), each
  subtree fetched in one cached call (~80 ms, ~700 nodes). WebView2 builds its accessibility
  tree only after the first request, so the first read is nearly empty. `echoctl ax-mic`,
  `echoctl ax-dump`. Confirm the call window's button label ("Mute (Ctrl+Shift+M)").
- **Screen frames** (`frames.rs`): `PrintWindow(PW_RENDERFULLCONTENT)` of the call window while
  someone else shares (works behind other windows, not minimized). `echoctl frame-grab <file>`.
- **Notifications** (`app::notifications`): WinRT toasts as AppUserModelID `onetest.Echo`,
  registered under `HKCU\Software\Classes\AppUserModelId` with Echo's logo
  (`%APPDATA%\echo\echo.png`); a click opens the window while the app runs (a toast posted by
  a CLI process that has exited does nothing on click).
- **Tray icon**: idle is white when `SystemUsesLightTheme` = 0, re-checked on each tick.

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
