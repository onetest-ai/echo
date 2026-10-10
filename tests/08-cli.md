# Command-line tool

### TC-CLI-001 · Install the command-line tool
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → Settings → **Install command-line tool (Echo)…**; enter your administrator password.
2. Open a new terminal: `which echoctl && echoctl status`.

**Expected**
- A "Command-line tool installed" notification.
- `/usr/local/bin/echoctl` points into `/Applications/Echo.app`.
- `echoctl status` reports the running app and its state.

---

### TC-CLI-002 · Arm and stop from the terminal
**Priority:** P2 · **Platform:** macOS

**Steps**
1. `echoctl arm`, then start a Teams call; after a minute `echoctl stop`.

**Expected**
- The menu-bar icon follows (green, orange, idle) exactly as with the menu.

---

### TC-CLI-003 · Doctor
**Priority:** P2 · **Platform:** macOS

**Steps**
1. `echoctl doctor`.

**Expected**
- It asks the running app (not the terminal) and lists each permission and the model,
  matching the Health menu.

---

### TC-CLI-004 · CLI without the app running
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → **Quit Echo**. Run `echoctl status`, then `echoctl app start`.

**Expected**
- `status` says the app isn't running; `app start` starts it and the icon reappears.
