---
id: TC-031
title: Calendar & chat → Colleagues' photos, not connected, says photos come with Microsoft 365, and the window gets no photos
priority: medium
type: functional
requirements: [F-SET-22, F-PPL-08]
module: settings
run: agent
platform: any
data: standard
tags: [settings, microsoft-365, photos]
---

# TC-031: Colleagues' photos before a sign-in: the row says what it needs, nothing is asked

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie). The test data has no Microsoft 365 sign-in
- CLI steps run in Bash after `. <dir>/env`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/calendar` and read the row "Colleagues' photos" in the card "Microsoft 365" | It says "In place of initials, once Microsoft 365 is connected." and has no switch and no button |
| 2 | Run `browser_evaluate` with `async () => (await fetch('/api/photos?n=' + encodeURIComponent(JSON.stringify(['Dana Lee', 'Omar Haddad'])))).json()` | `{}`: no photo for anyone |
| 3 | Run `ls "$ECHOCTL_CONFIG_DIR/photos" 2>/dev/null \| wc -l` | `0`: nothing was fetched or kept |
| 4 | Navigate to `{{base_url}}/#calls` and run `browser_evaluate` with `() => document.querySelectorAll('.ph').length` | `0`: every face shows its initials |

## Expected Final State
Nothing changed; no photo asked.

## Notes
- The switch is off by default (`config.json` `people_photos`), and without a sign-in that may read photos nothing is asked even when it is on (Rust: `photos::tests::nothing_is_shown_or_asked_with_the_switch_off_or_without_a_sign_in`). Allow…, the switch and real photos need a Microsoft 365 account: `people/TC-015` (person).
