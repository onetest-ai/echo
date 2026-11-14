---
id: TC-032
title: On test data a plugin's script keeps Echo's open guard — a program it starts that opens a page with `open` (a sign-in's browser) only adds a line to open.log; nothing else of Echo's reaches it
priority: high
type: functional
requirements: [F-PLUG-25]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, scripts, test-safety]
---

# TC-032: A plugin's script on test data opens nothing

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data: `python3 tests/fixtures/make_data.py <dir> --set standard` (an empty `<dir>`); CLI steps in Bash after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`; `echoctl` is `target/debug/echoctl`, built right before the run. Confirm `echo "$ECHOCTL_TEST_OPEN_LOG_ONLY"` prints `1` and `ECHOCTL_HOME` is inside `<dir>` before going on. No window step
- A made-up plugin **Acme sign-in** in a local marketplace. Its script refuses to go on without the guard, or when `open` isn't the test data's own, so a regression fails the case instead of opening a browser:

```bash
M="$D/market"; K="$M/plugins/acme-signin"
mkdir -p "$K/skills/signin/scripts"
echo '{"name":"acme-local","plugins":[{"id":"acme-signin","source":"./plugins/acme-signin"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-signin","name":"Acme sign-in","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","skills":["skills/signin"],"uses":{"scripts":[{"skill":"signin","script":"scripts/signin.sh"}]}}' > "$K/echo-plugin.json"
printf -- '---\nname: signin\ndescription: Signs in to Acme.\n---\n\nRun scripts/signin.sh.\n' > "$K/skills/signin/SKILL.md"
printf '#!/bin/sh\necho "guard=${ECHOCTL_TEST_OPEN_LOG_ONLY:-none} home=${ECHOCTL_HOME:-none}"\n[ -n "$ECHOCTL_TEST_OPEN_LOG_ONLY" ] || { echo "no guard: not opening"; exit 3; }\no=$(command -v open); grep -q ECHOCTL_TEST_OPEN_LOG_ONLY "$o" 2>/dev/null || { echo "not the test data open: $o"; exit 4; }\nopen https://signin.acme.example/device\n' > "$K/skills/signin/scripts/signin.sh"
echo "export function apply(ctx) { ctx.command('signin', () => ctx.scripts.run('signin', 'scripts/signin.sh', [])) }" > "$K/main.js"
echoctl plugin marketplace add "$M" --name acme-local
```

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-signin --yes` | "Installed Acme sign-in 0.1.0" |
| 2 | `L=$(wc -l < "$D/open.log" 2>/dev/null \|\| echo 0)`; `echoctl plugin call acme-signin signin` | `"status": 0`; `stdout` starts `guard=1 home=none` (the guard reaches the script, Echo's data folder doesn't) |
| 3 | `tail -n +$((L+1)) "$D/open.log"` | One new line: `https://signin.acme.example/device`; no browser window opened |

## Expected Final State
Acme sign-in installed; open.log has the sign-in's line; nothing opened.

## Teardown
- `echoctl plugin remove acme-signin --delete-data`; `echoctl plugin marketplace remove acme-local`; `rm -r "$M"`

## Notes
- Covered: a program that opens a page through `open` on PATH. One that opens a browser another way (Python's `webbrowser`, `/usr/bin/open` by its full path, Windows' `start`) bypasses the test data's fake.
- The variable list a script keeps is the Rust test `plugins::scripts::a_script_gets_test_datas_open_guard_but_nothing_else_of_echos_or_claudes`.
