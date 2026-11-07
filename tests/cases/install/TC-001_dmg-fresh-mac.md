---
id: TC-001
title: Install from the DMG on a fresh Mac
priority: critical
type: functional
requirements: [F-LIFE-01]
module: install
run: agent
platform: macos
tags: [install, gatekeeper]
external_id: TC-INST-001
user_steps:
  - 'the user drags Echo to Applications in the DMG''s Finder window'
  - 'Gatekeeper''s "downloaded from the internet" prompt: the user clicks Open'
---

# TC-001: Install from the DMG on a fresh Mac

**Module:** Install | **Priority:** Critical | **Type:** Functional

## Preconditions
- A Mac where Echo has never been (app_profile.md → Clean-install runs): `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; otherwise the run stops and asks the user to reset it
- The DMG of the release, with the quarantine flag a browser sets: `gh release download v0.8.2 -R onetest-ai/echo -p '*macos-arm64.dmg' -D "$RUN"` then `xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" "$RUN"/Echo-*.dmg`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `open "$RUN"/Echo-*.dmg` (as a double-click); `spctl -a -vv "/Volumes/Echo <version>/Echo.app"`; `spctl -a -t open --context context:primary-signature -v "$RUN"/Echo-*.dmg` | A Finder window with Echo and Applications, no "damaged" or "unidentified developer" dialog (the user confirms); the app: accepted, Notarized Developer ID; the DMG: accepted (notarized) |
| 2 | **The user does:** drag Echo onto Applications in that window when the run asks (not a copy in Terminal: app_profile.md → Clean-install runs). The run: `hdiutil detach`, `open /Applications/Echo.app`. **The user does:** at "“Echo” is an app downloaded from the Internet…" read it and click Open | The only prompt is that standard one, and it names Echo (the user confirms the text); Echo runs from /Applications/Echo.app, not an AppTranslocation path (its process and `app-path`) |
| 3 | Answer the welcome questions as S-15 says (install/TC-002, TC-004), then check the Dock and the menu bar | No Echo icon stays in the Dock (`lsappinfo info -only ApplicationType Echo` is UIElement, or the user confirms); the octopus is in the menu bar (the user confirms, or System Events lists Echo's menu bar item); `$E status` answers with the running app |

## Expected Final State
A signed, notarised app installs and starts in the menu bar.
