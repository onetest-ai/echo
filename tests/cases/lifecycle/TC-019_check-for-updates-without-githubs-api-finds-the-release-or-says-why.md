---
id: TC-019
title: Check for updates when GitHub's API refuses — macOS finds the release on github.com without the GitHub CLI; when nothing answers, the notice says why in words
priority: high
type: functional
requirements: [F-LIFE-13]
module: lifecycle
run: person
platform: macos
tags: [update]
---

# TC-019: Check for updates without GitHub's API finds the release, or says why

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- An installed Echo one version older than the newest release (e.g. 0.10.9 when 0.10.10 is out), on a Mac without the GitHub CLI (`command -v gh` finds nothing; `/opt/homebrew/bin/gh` and `/usr/local/bin/gh` absent)
- GitHub's API refuses this network: either its anonymous limit is used up (`curl -s https://api.github.com/rate_limit` shows `"remaining": 0` under `core`), or `api.github.com` is blocked for the test (`sudo sh -c 'echo "0.0.0.0 api.github.com" >> /etc/hosts'`, removed after)

**Changes:** none (step 1 is left at the update's question; `/etc/hosts` put back in Teardown)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → Echo → Check for updates | No "Couldn't check for updates": the update's question names the newest version, as when the API answers (lifecycle/TC-007); answer Later |
| 2 | Disconnect from the internet (Wi-Fi off); Check for updates again | A notice "Couldn't check for updates" whose text says why in words: "Echo can't reach GitHub — check your internet connection"; no "os error", no "No such file or directory" |
| 3 | Reconnect; Check for updates again | The update's question again, naming the same newest version, still with no GitHub CLI and no sign-in; answer Later |

## Expected Final State
An update is found without GitHub's API or the GitHub CLI on macOS; a check that can't reach GitHub says so in plain words.

## Teardown
- Remove the `api.github.com` line from `/etc/hosts` if it was added; Wi-Fi on.

## Notes
- Windows reads the release through the API, or through the GitHub CLI's sign-in when the API refuses: its MSI is installed only with the release's checksum, which those give (F-WIN-05). Without the CLI there, a refused API gives the notice "GitHub limits update checks from your network for now (too many from one address); Echo tries again later. To check now, install the GitHub CLI … and sign in with `gh auth login`."
