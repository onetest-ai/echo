# Install and first run

### TC-INST-001 · Install from the DMG on a fresh Mac
**Priority:** P0 · **Platform:** macOS

**Preconditions:** fresh Mac (see README). The DMG is downloaded with a browser from the GitHub
release (not copied over AirDrop or USB, so Gatekeeper sees the quarantine flag).

**Steps**
1. Open `Echo-<version>-macos-arm64.dmg`.
2. Drag **Echo** to **Applications**.
3. Open Echo from Applications.

**Expected**
- The DMG mounts with no "damaged" or "unidentified developer" warning.
- At most the standard "downloaded from the internet, open?" prompt, naming Echo.
- No Dock icon stays after the welcome dialogs; the octopus appears in the menu bar.

---

### TC-INST-002 · Choose the data folder
**Priority:** P0 · **Platform:** macOS, Windows

**Preconditions:** TC-INST-001 step 3, first launch.

**Steps**
1. At "Welcome to Echo. Where should call transcripts and notes be kept?", click **Choose folder…**.
2. Pick or create a folder, for example `~/Documents/Echo Calls` (with a space in the name).
3. Open the menu → Settings.

**Expected**
- The folder picker opens and the chosen folder is accepted.
- The Settings submenu shows that folder.
- After the first call, `calls/` and `.echo/` exist inside it, and nothing is written to `~/Echo`.

---

### TC-INST-003 · Use the default data folder
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Fresh machine, first launch: click **Use default**.

**Expected**
- `~/Echo` is used (shown under menu → Settings).

---

### TC-INST-004 · Download the speech model at first run
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. At "Echo transcribes on this computer. Download the speech model now?", click **Download**.
2. Open the menu → **Speech models** while it downloads.
3. Wait for it to finish.

**Expected**
- The suggested model is `large-v3-turbo-q5_0` on Apple Silicon, with its size in MB.
- The Speech models submenu shows the download progress in %.
- A "Speech model ready" notification appears; the model is marked as in use.
- The model file is in `~/Library/Application Support/echo/models/`, not in the data folder.

---

### TC-INST-005 · Skip the model, download it later
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Fresh machine, first launch: click **Later** at the model prompt.
2. Menu → Speech models → pick the recommended model → **Download (… MB)**.
3. After it finishes, **Use this model**.

**Expected**
- Echo runs without a model (recording still works; transcription waits for a model).
- The download completes from the menu and the model can be selected.

---

### TC-INST-006 · Interrupted model download
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Start a model download.
2. At about 30 %, turn Wi-Fi off for a minute, then on.

**Expected**
- A "Model download failed" notification with a readable reason, or the download resumes.
- No half-downloaded file is used as a model; downloading again from the menu works.

---

### TC-INST-007 · Remove a model
**Priority:** P2 · **Platform:** macOS, Windows

**Preconditions:** two models downloaded, one in use.

**Steps**
1. Menu → Speech models → the model not in use → **Remove (… MB)…** → **Remove**.
2. Try the same on the model in use.

**Expected**
- The unused model is deleted and disappears from the downloaded list.
- Removing the model in use (or the language-ID helper model) is refused or asks clearly
  first, and Echo keeps a working model.
