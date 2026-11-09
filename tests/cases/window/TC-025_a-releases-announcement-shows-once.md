---
id: TC-025
title: "With Whisper in use, the window says once that Whisper is deprecated: Not now hides it for good, Switch makes Parakeet v3 the model"
priority: medium
type: functional
requirements: [F-WIN-08, F-SET-09]
module: window
run: agent
platform: macos
tags: [window, announcements, models]
---

# TC-025: A release's announcement shows once

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`); CLI steps run in Bash after `. <dir>/env`
- The speech models and the NeMo-Speech runtime of an installed Echo in the run's config, both `large-v3-turbo-q5_0` and `parakeet-tdt-0.6b-v3` downloaded: `mkdir -p "$ECHOCTL_CONFIG_DIR/models"; for f in "$HOME/Library/Application Support/echo/models/"* <the folder holding parakeet-tdt-0.6b-v3.q8_0.gguf>/*.gguf; do ln -sf "$f" "$ECHOCTL_CONFIG_DIR/models/"; done; ln -sfn /Applications/Echo.app/Contents/Frameworks/nemo-speech "$ECHOCTL_CONFIG_DIR/nemo-speech"`
- `target/debug/echoctl model use large-v3-turbo-q5_0`

**Changes:** `config.json` `announcements_seen`, `model`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Load `{{base_url}}/`; read the strip under the top bar | A strip across the whole window width: "Whisper is deprecated." with a line on why and what stays on Whisper, a primary button "Switch to Parakeet v3" and "Not now" |
| 2 | Click "Not now" | The strip is gone |
| 3 | Wait 1 s, run `grep -A2 '"announcements_seen"' "$ECHOCTL_HOME/.echo/config.json"; target/debug/echoctl model list \| grep -i "active"` | `"whisper-deprecated"` is listed; the active model is still `large-v3-turbo-q5_0` |
| 4 | Reload `{{base_url}}/` | No strip: answered once, it never comes back |
| 5 | **The user does** (Echo's own window, where the tray runs its actions; a served window has no tray): with Whisper in use and Parakeet v3 downloaded, the strip shows; click "Switch to Parakeet v3" | The strip is gone; within a few seconds a notification "Speech model switched"; `echoctl model list` marks `parakeet-tdt-0.6b-v3` active (not downloaded yet: it downloads first, then switches) |

## Expected Final State
The announcement was shown as a strip across the window until answered, the answer kept in `config.json`; Switch made Parakeet v3 the active model.

## Teardown
- `target/debug/echoctl model use large-v3-turbo-q5_0`

## Notes
- Without Parakeet in this build (no NeMo-Speech runtime linked), or with a model other than Whisper in use, no strip shows: it doesn't apply (`announce::applies`).
- A new announcement in a later release is an entry in `announce::LIST`; this case is the pattern for its own.
