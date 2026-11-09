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
| 1 | Load `{{base_url}}/`; read the notices floating over the window | One: "Whisper is deprecated." with a line on why and what stays on Whisper, a primary button "Switch to Parakeet v3" and "Not now" |
| 2 | Click "Not now" | The notice is gone; nothing in the page moved |
| 3 | Wait 1 s, run `grep -A2 '"announcements_seen"' "$ECHOCTL_HOME/.echo/config.json"; target/debug/echoctl model list \| grep -i "active"` | `"whisper-deprecated"` is listed; the active model is still `large-v3-turbo-q5_0` |
| 4 | Reload `{{base_url}}/` | No notice: answered once, it never comes back |
| 5 | Run `python3 -c "import json,os;p=os.environ['ECHOCTL_HOME']+'/.echo/config.json';c=json.load(open(p));c.pop('announcements_seen',None);json.dump(c,open(p,'w'),indent=2)"`; reload `{{base_url}}/`; click "Switch to Parakeet v3" | The notice is gone |
| 6 | Wait 2 s, run `target/debug/echoctl model list \| grep -i "active"; grep -A2 '"announcements_seen"' "$ECHOCTL_HOME/.echo/config.json"` | The active model is `parakeet-tdt-0.6b-v3` (already downloaded: no download ran); `"whisper-deprecated"` is listed |

## Expected Final State
The announcement was shown until answered, once per answer kept in `config.json`; Switch made Parakeet v3 the active model.

## Teardown
- `target/debug/echoctl model use large-v3-turbo-q5_0`

## Notes
- Without Parakeet in this build (no NeMo-Speech runtime linked), or with a model other than Whisper in use, no notice shows: it doesn't apply (`announce::applies`).
- A new announcement in a later release is an entry in `announce::LIST`; this case is the pattern for its own.
