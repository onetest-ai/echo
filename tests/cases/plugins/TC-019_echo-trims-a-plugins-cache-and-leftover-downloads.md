---
id: TC-019
title: When a plugin's host half starts, Echo removes what lay in its cache/ over 7 days and downloads left half-written, and keeps everything else of its store
priority: medium
type: functional
requirements: [F-PLUG-13]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, store, cache, host-half]
---

# TC-019: Echo trims a plugin's cache and leftover downloads

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic (the fake Claude Code): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- A made-up local marketplace `acme-local` with **Acme cache** (a host half with one command, `ping`), added and installed:

```bash
M="$D/market-cache"; mkdir -p "$M/plugins/acme-cache"
echo '{"name":"acme-local","plugins":[{"id":"acme-cache","source":"./plugins/acme-cache"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-cache","name":"Acme cache","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js"}' > "$M/plugins/acme-cache/echo-plugin.json"
printf "export function apply(ctx) { ctx.command('ping', () => 'pong') }\n" > "$M/plugins/acme-cache/main.js"
echoctl plugin marketplace add "$M" --name acme-local && echoctl plugin install acme-cache --yes
```

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `<data>/.echo/plugins/acme-cache/` (the files below)

## Test Data

Files in the plugin's store (`S="$ECHOCTL_HOME/.echo/plugins/acme-cache"`), each with the age it is given:

| File | Age | After the host half starts |
|---|---|---|
| `cache/old.mp3` | 8 days | removed |
| `cache/deep/old.png` | 8 days | removed |
| `cache/new.mp3` | 1 day | kept |
| `audio/b.mp3.123-4.part` | 2 hours | removed |
| `audio/c.mp3.123-5.part` | just now | kept (a download may still be writing it) |
| `audio/old.mp3` | 30 days | kept (outside `cache/`: the plugin's own to remove) |
| `notes.txt` | 30 days | kept |
| `book/chapter.part` | 2 hours | kept (a `.part` of the plugin's own, not a download of Echo's) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Make the files: `S="$ECHOCTL_HOME/.echo/plugins/acme-cache"; mkdir -p "$S/cache/deep" "$S/audio"; mkdir -p "$S/book"; for f in cache/old.mp3 cache/deep/old.png cache/new.mp3 audio/b.mp3.123-4.part audio/c.mp3.123-5.part audio/old.mp3 notes.txt book/chapter.part; do echo x > "$S/$f"; done; touch -t $(date -v-8d +%Y%m%d%H%M) "$S/cache/old.mp3" "$S/cache/deep/old.png"; touch -t $(date -v-1d +%Y%m%d%H%M) "$S/cache/new.mp3"; touch -t $(date -v-2H +%Y%m%d%H%M) "$S/audio/b.mp3.123-4.part" "$S/book/chapter.part"; touch -t $(date -v-30d +%Y%m%d%H%M) "$S/audio/old.mp3" "$S/notes.txt"` | No output |
| 2 | `echoctl plugin call acme-cache ping` (it starts the plugin's host half; if it answers `no command “ping”`, wait 2 s and run it again) | `pong` |
| 3 | `cd "$S" && find . -type f \| sort` | Exactly `./audio/c.mp3.123-5.part`, `./audio/old.mp3`, `./book/chapter.part`, `./cache/new.mp3`, `./notes.txt` (and `./settings.json` if it exists): the old cache files and the stale `.part` are gone, the rest kept |

## Expected Final State
The plugin's `cache/` holds only what is under 7 days old; no download of Echo's (`<name>.<pid>-<n>.part`) older than an hour is left; everything outside `cache/` is as it was.

## Teardown
- `echoctl plugin remove acme-cache --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-PLUG-13), not from the code.
- An empty `cache/deep` folder may stay; only files are looked at.
- Echo names its half-written downloads `<file>.<pid>-<n>.part`; the case makes such names by hand.
