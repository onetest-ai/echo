---
id: TC-010
title: An Echo macOS runs translocated records the copy in Applications, never its temporary path
priority: high
type: functional
requirements: [F-LIFE-09]
module: lifecycle
run: person
platform: macos
tags: [lifecycle, install, cli]
---

# TC-010: An Echo macOS runs translocated records the copy in Applications, never its temporary path

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- A Mac (or a test user account) with no Echo installed and no `~/Library/Application Support/echo` (the clean-install setup of S-15)
- The release DMG of the build under test, downloaded with quarantine (a browser download, or `xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" <dmg>`)

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Open the DMG; copy the app with `ditto "/Volumes/Echo <version>/Echo.app" /Applications/Echo.app` (not a Finder move, so quarantine stays) | `/Applications/Echo.app` exists |
| 2 | `open /Applications/Echo.app`, accept Gatekeeper, finish the first run; `ps -o command= -p $(pgrep -f 'Echo.app/Contents/MacOS/echoctl' \| head -1)` | The process runs from `…/AppTranslocation/<uuid>/d/Echo.app` (translocated) |
| 3 | `cat ~/Library/Application\ Support/echo/app-path` | `/Applications/Echo.app/Contents/MacOS/echoctl`, not an `AppTranslocation` path |
| 4 | Settings → General → Command line → Install…; `readlink /usr/local/bin/echoctl` | `/Applications/Echo.app/Contents/MacOS/echoctl` |
| 5 | `grep -A3 ProgramArguments ~/Library/LaunchAgents/ai.onetest.echo.plist` | `/usr/bin/open`, `-g`, `/Applications/Echo.app` (the first run turned Start at login on) |

## Expected Final State
Everything that points at Echo points at `/Applications/Echo.app`; nothing at the temporary copy.

## Notes
- Every translocated start shows "Move Echo to Applications" (step 2). With no copy in Applications (Echo opened straight from the DMG), Echo writes no `app-path`, and Install… and Start at login refuse with "Move Echo to Applications first" (#132).
