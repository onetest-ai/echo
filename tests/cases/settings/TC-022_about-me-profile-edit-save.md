---
id: TC-022
title: About me: the profile is edited and saved with the keyboard
priority: high
type: functional
requirements: [F-SET-14]
module: settings
run: agent
platform: any
tags: [settings, me]
auto: [tests/auto/specs/S-11.spec.mjs]
---

# TC-022: About me: the profile is edited and saved with the keyboard

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/me.md`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/me`; find the card "Your profile" | It shows the heading "About me" with the lines "Name:", "Role:", "Company / team:" and a button "Edit"; the text "Who reads it" lists minutes, live notes and the Brain's reader |
| 2 | Click "Edit" | A text box named "Your profile (Markdown)" (`textarea#me-profile`) opens with the same lines |
| 3 | Replace its text with `I lead delivery at Initech. Write to me in short bullets.` | The box holds that sentence |
| 4 | Press the platform save shortcut (`ControlOrMeta+s`) | The box closes; the card shows `I lead delivery at Initech. Write to me in short bullets.` and the button "Edit" |
| 5 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_HOME/.echo/me.md"` | The file holds `I lead delivery at Initech. Write to me in short bullets.` |

## Expected Final State
The profile text is rendered in the page and written to `me.md`.

## Teardown
- Click "Edit", replace the text with the lines: `# About me`, an empty line, then `Name:`, `Role:`, `Company / team:`, `What I work on now:`, `People I work with often (name — role):`, `Terms and names used in my calls:`, `How I like notes and follow-ups written (tone, length, language):` (one per line), and save with `ControlOrMeta+s`.

## Notes
- Escape in the box discards the edit; this case saves it.
