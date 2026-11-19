---
id: TC-035
title: The Claude panel pastes and copies Cyrillic as it is
priority: medium
type: functional
requirements: [F-CC-12]
module: claude
run: person
platform: macos
tags: [panel, clipboard]
external_id: TC-CLA-035
---

# TC-035: The Claude panel pastes and copies Cyrillic as it is

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo started from Finder or the Dock (not from a terminal), the Claude panel open

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Copy `Исследование Granola` in another app; click the Claude panel's input and press ⌘V | The input reads `Исследование Granola`, no `?` |
| 2 | Select `Исследование` in the panel's output with the mouse, press ⌘C, paste it into TextEdit | TextEdit shows `Исследование`, not `–ò—Å‚Ä¶` |

## Expected Final State
Text goes in and out of the panel's terminal in any language unchanged.

## Notes
- Stays person-run: it needs the real clipboard and an Echo with no locale (started from Finder). The tools' UTF-8 is checked by `window::tests::the_clipboard_tools_speak_utf8_without_a_locale`. On Windows, `window::tests::windows_copy_puts_the_text_itself_on_the_clipboard` (ignored: it needs a desktop) reads back exactly what a copy put there.
