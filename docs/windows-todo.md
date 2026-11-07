# Windows: what's missing

The macOS app is the reference. On Windows the app builds, records (WASAPI process loopback on
`ms-teams.exe` + mic), transcribes, shows the tray and the Echo window, and talks to Microsoft
365. What follows is either not built yet or never run on a real Windows machine. Build and
test on Windows itself: `cargo build --release -p echoctl` (with whisper.cpp), then
`scripts\package-windows.ps1`, and the cases in `tests/cases/windows/`.

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
- **Nemotron** (`nemo.rs`, model `nemotron-3.5-0.6b`): NVIDIA's NeMo-Speech CPU runtime in
  `nemo-speech\` next to echoctl.exe (package-windows.ps1 fetches it). Untested on Windows:
  loading it (`LOAD_WITH_ALTERED_SEARCH_PATH`, its app-local MSVC runtime DLLs next to our
  static-CRT exe), its speed on a CPU laptop (on this Mac's CPU: 115 s of speech in ~11 s) and
  whether the live worker keeps up during a call. Its Vulkan/CUDA builds aren't used yet.
- **Names from Teams' speaking marker** (`speaking.rs`, `ax::participants`): on Windows the
  element classes come from UIA's ClassName (Chromium reports the HTML class there; untested
  with WebView2 Teams), and tiles are found by control type MenuItem. Check with
  `echoctl ax-speaking` in a call (talk, pause, talk) that tiles appear and a token flips.
  Dialogs (the share panel) are skipped only on macOS (`UiNode.dialog` from the subrole): find
  what UIA reports for Teams' "Share content" panel and set it in `ax/windows.rs`, or its
  "Browse OneDrive" item counts as a second person and the 1:1 naming is lost. The call timer
  (`ax::call_elapsed`) is read from the same tree: check `ui.jsonl` shows a bare "36:28" text.
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

- **AI profiles and live notes** (`ai.rs`, `livenotes.rs`): the Rust tests have only run on macOS (several
  are Unix-only). Before calling them verified: `cargo test --no-default-features` on
  Windows (`FileLock` share modes, the minutes' single run, settings); a live job on a `local`
  profile through `curl.exe`; stopping a running `claude -p` job (pause, profile change, quit) with
  nothing left behind — `taskkill /T` on a launcher that already exited may miss its `node.exe`;
  a user environment with routing variables in any case (`Anthropic_Base_Url`) and `claude.cmd`:
  the job still uses the profile's account; a `HKCU\SOFTWARE\Policies\ClaudeCode` key refuses
  `local` / `claude` runs; retitling a call while its notes run (`notes-run.lock`, `.meta.lock` open).
  `ai.json` is owner-only through the user profile's ACL (the 0600 mode is Unix only).

- **Signing a Claude profile in** (`signin.rs`, `ai::login_here`): `claude auth login` started directly
  (no login shell) with no console window; whether it opens the browser from there, and the
  per-profile lock (share modes: shared = read + `FILE_SHARE_READ`, exclusive = no sharing), are
  untested. The first shared take of a lock file that doesn't exist yet creates it with write access
  for a moment: a second shared take at that instant sees a sharing violation (a false "signing in",
  retried). Smoke test before calling it verified: sign in from the window, Cancel during it, quit
  Echo during it (no `claude`/`node` left: `claude.cmd` launchers are stopped with `taskkill /T`), and
  `echoctl ai login <id>` + Ctrl-C in a console.

- **Notes, Minutes tab and automatic minutes** (`window.rs`, `tray.rs`, `ui/app.js`, `notes::auto_due`):
  in WebView2, the Minutes tab's sandboxed frame (`/minutes/<call>` at `http://echo.localhost`) shows the
  page with its diagrams and lightbox, a click on a time opens the Transcript there, and `echoctl window
  --selftest` reports `frame_blocked: true`; the Notes editor saves while typing and keeps a tray note
  added meanwhile; closing the window, Quit and an update right after typing save first. **The MSI**:
  run it directly while Echo is open with notes it can't save (a read-only `my-notes.md`), answer No
  to Echo's "Quit Echo anyway?" (No is focused; Yes must go on) — the installation must stop with no
  file replaced; a first install (no Echo yet) must not be affected. **Echo's own update** (tray → Update) passes
  `ECHO_SELF_UPDATE=1`: the MSI goes on whatever that quit answers, and Echo starts again on the new version.
  The popups come up in front (MB_SYSTEMMODAL); a meeting's "Record?" answered Yes records without joining.
  Show in Explorer selects the minutes page of a call whose title has spaces. A call's minutes start by themselves
  15 minutes after it ends.

- **Write minutes** (`notes.rs`): runs `claude` directly (no login shell, `cfg(unix)` only), so the
  user's env is whatever the app inherited. Check: `claude` is found (`claude.exe` / `claude.cmd`
  from npm, `app::find_program`), `--plugin-dir` points at the plugin next to `echoctl.exe`,
  the `//C:/…` permission-rule paths match Claude Code's Windows path format, the run's
  `PATH` puts `echoctl.exe` first, Show log opens `claude.log` in Notepad, and the signed-out
  and bad-model messages appear (`tests/cases/claude/`, once TC-CLA-009 … 013).
- **Plugin script** `plugin/scripts/echoctl.cmd` and the mod (`plugin/hooks/register.tsx`):
  finding the CLI, starting the app, the live pane.
- **Dialogs** (WScript popup, PowerShell InputBox for Minutes model and other text prompts) and
  the Echo window on WebView2 (`http://echo.localhost`).
- **Autostart** (HKCU Run key), **install-cli** (user PATH), single instance (`app.lock`).
- **Microsoft 365** sign-in (loopback redirect), calendar, chat capture.

## Thread folders in the Brain (untested on Windows)
`package-windows.ps1` ships PDFium (`pdfium\pdfium.dll`, chromium/7881, checked by SHA-256). Never run on a
Windows machine: PDF pages through it, LibreOffice found at `C:\Program Files\LibreOffice\program\soffice.exe`
and its `-env:UserInstallation=file:///C:/…` profile, documents opened with `explorer <file>`, the folder picker.
