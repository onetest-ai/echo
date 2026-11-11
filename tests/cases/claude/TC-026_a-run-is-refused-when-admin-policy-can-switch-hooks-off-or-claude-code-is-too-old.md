---
id: TC-026
title: A minutes run is refused when admin policy can switch hooks off, and when Claude Code is older than Echo needs
priority: high
type: functional
requirements: [F-CC-02]
module: claude
run: agent
platform: any
data: standard
tags: [claude, minutes, guard, hooks, policy, cli]
---

# TC-026: A minutes run is refused when policy can switch hooks off, or Claude Code is too old

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set: `tests/fixtures/serve.sh up <dir> <port> --set standard`. Every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`
- `F="$ECHOCTL_CONFIG_DIR/claude/work/remote-settings.json"` (server-managed settings in the profile's folder: a policy source Echo reads; no machine-wide file is touched); `mkdir -p "$(dirname "$F")"` (the data has no such folder yet)
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-initrode-intro)` (transcribed, no minutes, profile **Claude · work**)
- **A fake Claude Code that tells its version**, in place of the data's fake: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # `--version` says the version in $D/fake-version (default 2.1.200). A minutes run (ECHOCTL_RUN_CALL set) is recorded
  # in $D/started with what it was given on stdin, reports the same version on its init line, and fails after 3 s.
  # Anything else: the data's fake.
  d="$(dirname "$0")/.."
  v=$(cat "$d/fake-version" 2>/dev/null || echo 2.1.200)
  case "$1" in --version) echo "$v (Claude Code)"; exit 0;; esac
  if [ -n "$ECHOCTL_RUN_CALL" ] && [ -z "$ECHOCTL_RUN_MODE" ]; then
    date > "$d/started"; cat >> "$d/started"
    echo "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s-026\",\"claude_code_version\":\"$v\"}"
    echo '{"type":"assistant","message":{"model":"sonnet","content":[{"type":"text","text":"Reading the transcript."}]}}'
    sleep 3
    echo '{"type":"result","is_error":true,"result":"fake 026 stopped here"}'
    exit 1
  fi
  exec "$d/bin/claude.fake" "$@"
  ```
- Echo needs Claude Code 2.1.295 or later for minutes runs (README: "Minutes runs need Claude Code 2.1.295 or later")

**Changes:** `$F`, `$D/fake-version`, `$D/started`, `<call>/notes-run.json` and `claude.log`

## Test Data

| Field | Value |
|-------|-------|
| Policies | `{"disableAllHooks": true}` · `{"allowManagedHooksOnly": true}` |
| Hooks refusal | `the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session (<$F>)` |
| Versions | 2.1.200 and 2.1.294 (too old), 2.1.295 (enough) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echo '{"disableAllHooks": true}' > "$F"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` | `Error: the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session (<$F>) (log: …/claude.log)` and `exit 1` |
| 2 | `python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'], r['message'])" <call>/notes-run.json` | `failed ` + the same sentence without the `(log: …)` part; the fake never started (no "fake 026 stopped here") |
| 3 | `echo '{"allowManagedHooksOnly": true}' > "$F"`, then `target/debug/echoctl notes <call> --wait` | The same `Error: the minutes run needs Echo's guard …` with `(<$F>)` |
| 4 | `echo 'not json' > "$F"`, then `target/debug/echoctl notes <call> --wait` | Refused before the hooks check, by the profile's own policy check: `Error: your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F> can't be read) (log: …)`, then how to write them on Claude Code (as configured) instead (`--profile system`); `notes-run.json` has `failed` (the choice itself: claude/TC-029) |
| 5 | `echo '{"model": "sonnet"}' > "$F"; rm -f "$D/started"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` (the fake's version is 2.1.200) | The policy check passes, then the run is refused for the version, before Claude Code was given anything: `Error: … Claude Code 2.1.200 is too old to write minutes …`, telling to update Claude Code to 2.1.295 or later (how: `claude update`), then `exit 1`; the message does not say "Try again" or point at a button (this is the terminal); `ls "$D/started"` finds nothing (no run was started, no prompt given); no "fake 026 stopped here" |
| 6 | `python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'], r['message'])" <call>/notes-run.json` | `failed ` + the same sentence about 2.1.200 and updating to 2.1.295 or later |
| 7 | `echo 2.1.294 > "$D/fake-version"; rm -f "$D/started"; target/debug/echoctl notes <call> --wait; ls "$D/started"` | `Error: … Claude Code 2.1.294 is too old to write minutes …` with the same advice; `ls` finds nothing |
| 8 | `echo 2.1.295 > "$D/fake-version"; target/debug/echoctl notes <call> --wait; cat "$D/started"` | `Error: fake 026 stopped here (log: …)`: a Claude Code that is new enough is not stopped; `$D/started` now exists (the run reached Claude Code) |

## Expected Final State
A policy that switches hooks off (or can't be read) and a Claude Code older than 2.1.295 each end the minutes run with their own sentence and `failed` in `notes-run.json`, before Claude Code is started or given anything; 2.1.295 is let through.

## Teardown
- `rm -f "$F" "$D/fake-version"; mv "$D/bin/claude.fake" "$D/bin/claude"`; `tests/fixtures/serve.sh down <dir>` and remove `<dir>`.

## Notes
- Step 4: the user's decision (#404) — an unreadable policy file asks instead of only refusing; see claude/TC-029.
- Machine-wide policy (`managed-settings.json` under `/Library/Application Support/ClaudeCode`, macOS managed preferences, Windows policy keys) is read by the same check but needs root: a person creates the file with `sudo` and removes it after, on test data only.
- Plugin runs and thread syncs use the same checks (`claudejob`, `projectsync`); this case checks the minutes run.
