---
id: TC-024
title: A Claude profile signs in from the window (Sign in…, Cancel) and from `echoctl ai login`, and another account asks "Use <account>"
priority: high
type: functional
requirements: [F-AI-05]
module: claude
run: agent
platform: any
data: standard
tags: [claude, ai-profiles, signin, cli]
---

# TC-024: A Claude profile signs in from the window and the terminal; another account asks "Use <account>"

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is served at `{{base_url}}`. Every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`
- The standard AI profiles (fake Claude Code, never a real account): **Claude · work** (signed in), **Claude · other** (the folder is signed in to `sam.rivera@globex.example`, the profile was set up for `sam@initech.example · initech`), **Claude · out** (signed out, no account recorded)
- Wait 10 seconds after the server starts: the sign-in checks of the profiles settle in the background
- **A fake `claude` that can sign in**, in place of the data's fake (which refuses `auth login`): `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # The data's fake + a sign-in. `auth login` records its arguments in the profile folder (login.args) and does what $D/fake-login says: ok (the folder is signed in to sam@initech.example from then on), anything else: waits, as for a browser that never comes back. Anything else: the data's fake.
  d="$(dirname "$0")/.."
  case "$1 $2" in
  "auth login") echo "$@" > "$CLAUDE_CONFIG_DIR/login.args"
    case "$(cat "$d/fake-login" 2>/dev/null)" in
      ok) touch "$CLAUDE_CONFIG_DIR/signed-in"; echo "Login successful."; exit 0;;
      *) exec sleep 120;;
    esac;;
  "auth status") if [ -f "$CLAUDE_CONFIG_DIR/signed-in" ]; then echo '{"loggedIn": true, "email": "sam@initech.example", "orgName": "Initech", "subscriptionType": "max"}'; exit 0; fi;;
  esac
  exec "$d/bin/claude.fake" "$@"
  ```
- `echo hang > "$D/fake-login"`

**Changes:** `<config dir>/ai.json` (the default profile; the account recorded for `out` and `other`), `<config dir>/claude/out/` (the fake's `signed-in` and `login.args`)

## Test Data

| Field | Value |
|-------|-------|
| Signed-out profile | Claude · out (`out`) |
| Account the fake signs it in to | `sam@initech.example · initech (max)` (email · organisation (plan)) |
| Profile signed in to another account | Claude · other (`other`) now on `sam.rivera@globex.example · globex`, set up for `sam@initech.example · initech` |
| The note | `.pill-note` under the Record pill: it is the default profile's sign-in |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl ai default out` | Prints `new calls use Claude · out` |
| 2 | Load `{{base_url}}/` and wait for the text "isn't signed in."; read `.pill-note` | "Claude · out isn't signed in." and the button "Sign in…" |
| 3 | Click "Sign in…" in the note; read the note; then `cat "$ECHOCTL_CONFIG_DIR/claude/out/login.args"` | The note reads "Sign in to Claude · out in your browser…" with a button "Cancel" and the hint "Browser didn't open? Cancel, then run `echoctl ai login out` in Terminal" with a "Copy" button; `login.args` holds `auth login --claudeai` |
| 4 | Click "Cancel" in the note | Within 15 s the note is "Claude · out isn't signed in." with "Sign in…" again; `target/debug/echoctl ai status out` has `"signed_in": false` and `"account": null` |
| 5 | `echo ok > "$D/fake-login"`, then click "Sign in…" in the note | The note reads "Signed in as sam@initech.example · initech (max)." |
| 6 | `target/debug/echoctl ai status out` | `"signed_in": true`, `"account": "sam@initech.example · initech (max)"`, `"expected_account": "sam@initech.example · initech"` (the first sign-in recorded the account) |
| 7 | `rm "$ECHOCTL_CONFIG_DIR/claude/out/signed-in"`, then `target/debug/echoctl ai status out \| grep signed_in` | `"signed_in": false` |
| 8 | `target/debug/echoctl ai login out; echo "exit $?"`, then `cat "$ECHOCTL_CONFIG_DIR/claude/out/login.args"` | Prints "Login successful." then `Claude · out is signed in as sam@initech.example · initech (max)` and `exit 0`; `login.args` is now `auth login --claudeai --email sam@initech.example` (the recorded account's email pre-filled) |
| 9 | `target/debug/echoctl ai default other`, reload `{{base_url}}/` and wait for the text "not sam@initech.example"; read `.pill-note` | "Claude · other is signed in as sam.rivera@globex.example · globex, not sam@initech.example · initech." with the buttons "Use sam.rivera@globex.example · globex" and "Sign in again…" |
| 10 | `target/debug/echoctl ai status other \| grep problem` | `"problem"` is a text starting "Claude · other is signed in as sam.rivera@globex.example · globex, not sam@initech.example · initech — Use sam.rivera@globex.example · globex in the Echo window, or sign in again" |
| 11 | Click "Use sam.rivera@globex.example · globex" | Within 20 s `.pill-note` is gone from the page |
| 12 | `target/debug/echoctl ai status other \| grep -E 'problem\|expected_account'` | `"expected_account": "sam.rivera@globex.example · globex"` and `"problem": null` |

## Expected Final State
`out` signed in from the window and again from the terminal, with its account recorded; the Cancel left it signed out; `other` accepted the account it is signed in to.

## Teardown
- `tests/fixtures/serve.sh down <dir>`, then remove `<dir>` (this case changes the data's AI profiles; a re-run builds fresh data). Close the browser tab on `{{base_url}}/`.

## Notes
- The tray's "Sign in…" (the same `signin::start`) and a real browser sign-in are not checked here: the tray is person-run and a real account is never used in agent runs.
- On a Mac the app's own notifications ("Signed in · Claude · out", "Account accepted") may show as real banners (`app::notify` runs `osascript` on a dev build); they are not asserted.
- Step 3's hint and the missing "(as <email>)": `out` has no email or recorded account at that point. After step 6 it would read "… in your browser (as sam@initech.example)…".
