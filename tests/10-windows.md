# Windows

Windows 10 2004+ or Windows 11, Teams (new) installed. Known limits for this beta: the build is
unsigned (updates are checked against the release's SHA-256 instead).

### TC-WINOS-001 · Build
**Priority:** P0 · **Platform:** Windows

**Preconditions:** Rust ≥ 1.90 (MSVC toolchain), Visual Studio Build Tools (C++), CMake (ships
with the Build Tools), LLVM (`LIBCLANG_PATH` set; build-time only, for whisper-rs' bindgen),
Vulkan SDK (`VULKAN_SDK`), Python.

**Steps**
1. `cargo build --release -p echoctl` then `scripts\package-windows.ps1`.

**Expected**
- The build finishes with whisper.cpp compiled in (Vulkan + AVX2 CPU).
- The zip contains `echoctl.exe`, `vulkan-1.dll`, `VULKAN-RUNTIME-LICENSE.txt`, `README.md`,
  `LICENSE.txt`, `THIRD-PARTY-LICENSES.txt` and `plugin\`.
- `dumpbin /dependents echoctl.exe` lists no `MSVCP140` / `VCRUNTIME140` (static C runtime).
- On a PC without a GPU driver (or a VM) Echo starts and transcribes on the CPU.

---

### TC-WINOS-002 · Install and first run
**Priority:** P0 · **Platform:** Windows

**Steps**
1. Unzip to `%LOCALAPPDATA%\Programs\echo`.
2. Run `echoctl.exe app autostart on`, then double-click `echoctl.exe`.
3. Go through the folder and model dialogs.

**Expected**
- SmartScreen may warn ("More info → Run anyway"); after that, no console window stays open.
- The tray icon appears; the data folder and the model download work as on macOS
  (TC-INST-002, TC-INST-004).
- Config lands in `%APPDATA%\echo`.

---

### TC-WINOS-003 · Record a call
**Priority:** P0 · **Platform:** Windows

**Steps**
1. Before the call: `echoctl procs` → `in call: false`. Tray → Record (the icon turns green:
   armed, not recording).
2. Join a Teams test call. `echoctl procs` → an `active` session marked `teams` on your mic,
   `in call: true`. Talk for 2 minutes on both sides.
3. Leave the call.

**Expected**
- Recording starts when the call does (`call_started`, `recording_started` in
  `.echo/events.jsonl`) and stops ~10 s after leaving (`call_ended`) — no Stop needed.
- The mic track follows the mic Teams uses (a headset, not the default input).
- Both sides are captured (only Teams' audio for the remote side: play a YouTube video at
  the same time; it must not appear in the transcript).
- The call is transcribed; the WAV files are deleted afterwards.

---

### TC-WINOS-007 · Mute sync and Teams UI text
**Priority:** P1 · **Platform:** Windows

**Steps**
1. In a Teams call: `echoctl ax-mic --seconds 30`; mute and unmute in Teams.
2. Record a call; mute for ~20 s in the middle while talking.

**Expected**
- `ax-mic` shows the button ("Mute (Ctrl+Shift+M)" / "Unmute …") flipping live / MUTED.
- The call's events show `mic_muted` / `mic_unmuted`; nothing said while muted is in the
  transcript. `ui.jsonl` and `ui-tree-10s.json` exist in the call folder.

---

### TC-WINOS-008 · Notifications and tray look
**Priority:** P1 · **Platform:** Windows

**Steps**
1. `echoctl notify "Test" "Hello"`. With the app running, finish a recording with no Claude
   session open ("Transcript ready") and click the toast.
2. Switch Windows between light and dark mode (Settings → Personalization → Colors).

**Expected**
- Toasts say "Echo" with the octopus logo; clicking one (while the app runs) opens the Echo
  window.
- The idle tray icon is black on a light taskbar and white on a dark one, switching within a
  few seconds.

---

### TC-WINOS-009 · Screen frames
**Priority:** P2 · **Platform:** Windows

**Steps**
1. `echoctl frame-grab %TEMP%\echo-frame.jpg` with Teams open (not minimized), behind other windows.
2. Settings → Save snapshots of shared screens; record a call where someone shares slides.

**Expected**
- The grab shows the Teams window's content (not black), though other windows cover it.
- `frames/` holds one JPEG per slide; `frames.jsonl` indexes them.

---

### TC-WINOS-004 · Window
**Priority:** P1 · **Platform:** Windows

**Steps**
1. Tray → Open Echo….

**Expected**
- The window opens (WebView2) and shows calls, tabs and the calendar as on macOS.

---

### TC-WINOS-005 · Starts at login, quit, PATH
**Priority:** P1 · **Platform:** Windows

**Steps**
1. Sign out and back in. Quit from the tray. Settings → Install command-line tool, then in a
   new terminal run `echoctl status`.

**Expected**
- Echo starts at sign-in; Quit leaves no `echoctl.exe` running; the CLI works from a new
  terminal.

---

### TC-WINOS-006 · Update
**Priority:** P2 · **Platform:** Windows

**Expected**
- With a newer release out, the tray offers it; Update downloads the MSI, checks its SHA-256,
  installs it silently (no admin prompt) and Echo restarts on the new version
  (`%TEMP%\echo-update-<version>\install.log`).
