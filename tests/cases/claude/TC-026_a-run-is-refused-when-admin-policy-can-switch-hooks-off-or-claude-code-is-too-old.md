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
- `F="$ECHOCTL_CONFIG_DIR/claude/work/remote-settings.json"` (server-managed settings in the profile's folder: a policy source Echo reads; no machine-wide file is touched)
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-initrode-intro)` (transcribed, no minutes, profile **Claude · work**)
- **A fake Claude Code that tells its version**, in place of the data's fake: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # A minutes run reports the version in $D/fake-version (default 2.1.200) on its init line, then fails after 3 s. Anything else: the data's fake.
  if [ -n "$ECHOCTL_RUN_CALL" ] && [ -z "$ECHOCTL_RUN_MODE" ]; then
    cat > /dev/null
    v=$(cat "$(dirname "$0")/../fake-version" 2>/dev/null || echo 2.1.200)
    echo "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s-026\",\"claude_code_version\":\"$v\"}"
    echo '{"type":"assistant","message":{"model":"sonnet","content":[{"type":"text","text":"Reading the transcript."}]}}'
    sleep 3
    echo '{"type":"result","is_error":true,"result":"fake 026 stopped here"}'
    exit 1
  fi
  exec "$(dirname "$0")/claude.fake" "$@"
  ```
- Minimum Echo needs: Claude Code 2.1.295 (`guard::MIN_CLAUDE`)

**Changes:** `$F`, `$D/fake-version`, `<call>/notes-run.json` and `claude.log`

## Test Data

| Field | Value |
|-------|-------|
| Policies | `{"disableAllHooks": true}` · `{"allowManagedHooksOnly": true}` |
| Hooks refusal | `the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session (<$F>)` |
| Versions | 2.1.200, 2.1.294 (too old), 2.1.295 (enough) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echo '{"disableAllHooks": true}' > "$F"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` | `Error: the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session (<$F>) (log: …/claude.log)` and `exit 1` |
| 2 | `python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'], r['message'])" <call>/notes-run.json` | `failed ` + the same sentence without the `(log: …)` part; the fake never started (no "fake 026 stopped here") |
| 3 | `echo '{"allowManagedHooksOnly": true}' > "$F"`, then `target/debug/echoctl notes <call> --wait` | The same `Error: the minutes run needs Echo's guard …` with `(<$F>)` |
| 4 | `echo 'not json' > "$F"`, then `target/debug/echoctl notes <call> --wait` | The same sentence, ending `(<$F> can't be read)` |
| 5 | `echo '{"model": "sonnet"}' > "$F"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` | Passes the hook check: Claude Code starts and the run ends with `Error: Claude Code 2.1.200 is too old for minutes runs — update it (`claude update`) to 2.1.295 or later, then click Try again (log: …)` and `exit 1` (the fake's default version 2.1.200) |
| 6 | `python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'], r['message'])" <call>/notes-run.json` | `failed Claude Code 2.1.200 is too old for minutes runs — update it (`claude update`) to 2.1.295 or later, then click Try again` |
| 7 | `echo 2.1.294 > "$D/fake-version"; target/debug/echoctl notes <call> --wait` | `Error: Claude Code 2.1.294 is too old for minutes runs — update it (`claude update`) to 2.1.295 or later, then click Try again (log: …)` |
| 8 | `echo 2.1.295 > "$D/fake-version"; target/debug/echoctl notes <call> --wait` | `Error: fake 026 stopped here (log: …)`: a Claude Code that is new enough is not stopped |

## Expected Final State
A policy that switches hooks off (or can't be read) and a Claude Code older than 2.1.295 each end the minutes run with their own sentence and `failed` in `notes-run.json`, before the model works on the call; 2.1.295 is let through.

## Teardown
- `rm -f "$F" "$D/fake-version"; mv "$D/bin/claude.fake" "$D/bin/claude"`; `tests/fixtures/serve.sh down <dir>` and remove `<dir>`.

## Notes
- Machine-wide policy (`managed-settings.json` under `/Library/Application Support/ClaudeCode`, macOS managed preferences, Windows policy keys) is read by the same check but needs root: a person creates the file with `sudo` and removes it after, on test data only.
- Plugin runs and thread syncs use the same checks (`claudejob`, `projectsync`); this case checks the minutes run.
