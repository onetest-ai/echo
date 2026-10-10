# App lifecycle and updates

### TC-LIFE-001 · Starts at login
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Make sure Settings → **Start Echo at login** is checked.
2. Log out and back in (or restart).

**Expected**
- The octopus is in the menu bar without opening anything; no dialogs appear again.

---

### TC-LIFE-002 · Start at login off
**Priority:** P2 · **Platform:** macOS, Windows

**Steps**
1. Uncheck Settings → Start Echo at login; log out and in.

**Expected**
- Echo does not start. Opening it by hand works.

---

### TC-LIFE-003 · Quit
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → **Quit Echo**; check Activity Monitor for `echoctl`.
2. Open Echo again from Applications.

**Expected**
- No `echoctl` process remains after quitting.
- It starts again normally, without repeating the first-run dialogs.

---

### TC-LIFE-004 · Only one Echo at a time
**Priority:** P1 · **Platform:** macOS

**Steps**
1. With Echo running, open it again from Applications (and run `echoctl app start` in a terminal).

**Expected**
- Still exactly one octopus in the menu bar and one `echoctl` app process. Opening it again
  may open the window, but never a second app.

---

### TC-LIFE-005 · Change the data folder
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Settings → **Change data folder…** → pick a new empty folder.

**Expected**
- Echo restarts itself on the new folder; new calls go there; the old calls stay in the old
  folder untouched.

---

### TC-UPD-001 · Update offered and installed
**Priority:** P1 · **Platform:** macOS

**Preconditions:** a release newer than the installed version exists on GitHub.

**Steps**
1. Start Echo (it checks at start, then every 6 hours).
2. Menu → **Update to <version>…** → **Update**.

**Expected**
- An "Update available" notification and the menu item appear.
- Echo downloads the release, quits, replaces itself and starts again on the new version
  (menu → status or `echoctl --version`).
- Data folder, models, permissions and the calendar sign-in survive the update.

---

### TC-UPD-002 · No update during a recording
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Start a recording, then click Update.

**Expected**
- Echo refuses with "Update failed: a call is being recorded — update after it ends"; the
  recording continues untouched.

---

### TC-UPD-003 · Up to date
**Priority:** P2 · **Platform:** macOS

**Expected**
- On the newest version there is no Update item and no notification.

---

### TC-LIFE-006 · Notifications come from Echo
**Priority:** P1 · **Platform:** macOS

**Steps**
1. On a fresh Mac, open Echo for the first time; answer **Allow** when macOS asks to send
   notifications.
2. Trigger a notification (finish a recording, or Menu → Claude Code → Connect Claude Code).
3. Click the notification.

**Expected**
- The banner shows Echo's icon (purple rounded square with the octopus), not a blank
  Script Editor icon; the Dock icon while the window is open is the same.
- Clicking it opens the Echo window.
- With notifications turned off for Echo in System Settings, nothing is shown.
