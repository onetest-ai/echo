# Windows

Windows 10 2004+ or Windows 11, Teams (new) installed. Known limits for this beta: Echo can't
detect calls, so **Record** starts recording immediately and **Stop** ends it; the build is
unsigned; updates open the release page.

### TC-WINOS-001 · Build
**Priority:** P0 · **Platform:** Windows

**Preconditions:** Rust (MSVC toolchain), Visual Studio Build Tools (C++), CMake,
LLVM (`LIBCLANG_PATH` set), Python.

**Steps**
1. `cargo build --release -p echoctl` then `scripts\package-windows.ps1`.

**Expected**
- The build finishes with whisper.cpp compiled in.
- The zip contains `echoctl.exe`, `README.md`, `LICENSE.md` and `THIRD-PARTY-LICENSES.txt`.

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
1. Join a Teams test call, then tray → Record. Talk for 2 minutes on both sides.
2. Tray → Stop.

**Expected**
- Both sides are captured (only Teams' audio for the remote side: play a YouTube video at
  the same time; it must not appear in the transcript).
- After Stop the call is transcribed; the WAV files are deleted afterwards.

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
- With a newer release out, the tray offers it and opens the release page (no automatic
  install on Windows).
