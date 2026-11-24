---
id: TC-026
title: A minutes run is refused when admin policy can switch hooks off; on a Claude Code older than Echo asks for it runs, and its log and Inbox say what that version lacks
priority: high
type: functional
requirements: [F-CC-02]
module: claude
run: agent
platform: any
data: standard
tags: [claude, minutes, guard, hooks, policy, cli]
---

# TC-026: A minutes run is refused when policy can switch hooks off; an older Claude Code runs and is warned about

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set: `tests/fixtures/serve.sh up <dir> <port> --set standard`. Every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`
- `F="$ECHOCTL_CONFIG_DIR/claude/work/remote-settings.json"` (server-managed settings in the profile's folder: a policy source Echo reads; no machine-wide file is touched); `mkdir -p "$(dirname "$F")"` (the data has no such folder yet)
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-initrode-intro)` (transcribed, no minutes, profile **Claude · work**)
- **A fake Claude Code that tells its version**, in place of the data's fake: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # `--version` says the version in $D/fake-version (default 2.1.200). A minutes run (ECHOCTL_RUN_CALL set) is recorded
  # in $D/started (and what it was given on stdin, if anything), reports the same version on its init line, and fails after 3 s.
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
- Echo recommends Claude Code 2.1.323 or later and never refuses an older one (README: "Claude Code 2.1.323 or later is recommended: an older one still writes minutes, and Echo shows what it lacks")

**Changes:** `$F`, `$D/fake-version`, `$D/started`, `<call>/notes-run.json` and `claude.log`, `$ECHOCTL_HOME/.echo/claude-versions/work`

## Test Data

| Field | Value |
|-------|-------|
| Policies | `{"disableAllHooks": true}` · `{"allowManagedHooksOnly": true}` |
| Hooks refusal | `the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session (<$F>)` |
| Versions | 2.1.200 (before 2.1.295), 2.1.300 (before 2.1.323), 2.1.323 (as recommended) |
| Warning for 2.1.200 | `Claude Code 2.1.200 is older than Echo asks for: Echo's guard can't stop a tool call when the guard itself fails, and auto mode may start with more permissions when it isn't available — update it to 2.1.323 or later` |
| Warning for 2.1.300 | `Claude Code 2.1.300 is older than Echo asks for: auto mode may start with more permissions when it isn't available — update it to 2.1.323 or later` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echo '{"disableAllHooks": true}' > "$F"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` | `Error: the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session (<$F>) (log: …/claude.log)` and `exit 1` |
| 2 | `python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'], r['message'])" <call>/notes-run.json` | `failed ` + the same sentence without the `(log: …)` part; the fake never started (no "fake 026 stopped here") |
| 3 | `echo '{"allowManagedHooksOnly": true}' > "$F"`, then `target/debug/echoctl notes <call> --wait` | The same `Error: the minutes run needs Echo's guard …` with `(<$F>)` |
| 4 | `echo 'not json' > "$F"`, then `target/debug/echoctl notes <call> --wait` | Refused before the hooks check, by the profile's own policy check: `Error: your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F> can't be read) (log: …)`, then how to write them on Claude Code (as configured) instead (`--profile system`); `notes-run.json` has `failed` (the choice itself: claude/TC-029) |
| 5 | `echo '{"model": "sonnet"}' > "$F"; rm -f "$D/started"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` (the fake's version is 2.1.200) | The policy check passes and the run is not refused for the version: it reaches Claude Code and ends as the fake does, `Error: fake 026 stopped here (log: …)`, `exit 1`; `$D/started` exists (the run reached Claude Code); nothing says "too old" |
| 6 | `grep "older than Echo asks for" <call>/claude.log` | One line with the test data's warning for 2.1.200, right after the run's start |
| 7 | `cat "$ECHOCTL_HOME/.echo/claude-versions/work"`, then `target/debug/echoctl attention \| python3 -c "import json,sys;[print(i['key'],'|',i['title'],'|',i['why'],'|',i.get('go')) for i in json.load(sys.stdin)['open'] if i['key'].startswith('claude-old')]"` | `2.1.200`; one item `claude-old:work \| Update Claude Code for Claude · work \| <the warning for 2.1.200> \| settings/ai` |
| 8 | Navigate to `{{base_url}}/#today`, click "Inbox" (`data-panel-flip="needs"`) and read it | An item "Update Claude Code for Claude · work" with the warning for 2.1.200 under it; clicking it opens Settings → AI (`#settings/ai`) |
| 9 | `echo 2.1.300 > "$D/fake-version"; target/debug/echoctl notes <call> --wait; grep "older than Echo asks for" <call>/claude.log \| tail -1`, then the `attention` command of step 7 | The run reaches Claude Code again (`fake 026 stopped here`); the last warning is the test data's for 2.1.300 (no word of the guard); the item's `why` is that warning too |
| 10 | `echo 2.1.323 > "$D/fake-version"; target/debug/echoctl notes <call> --wait`, then `cat "$ECHOCTL_HOME/.echo/claude-versions/work"` and the `attention` command of step 7 | `fake 026 stopped here`; `2.1.323`; no `claude-old` item, open or done (it leaves quietly: a newer version is nothing decided); the run's newest lines in `claude.log` have no "older than Echo asks for" |

## Expected Final State
A policy that switches hooks off (or can't be read) ends the minutes run with its sentence and `failed` in `notes-run.json`, before Claude Code is started. A Claude Code older than 2.1.323 is never refused: the run goes on, its log and the Inbox say what that version lacks and that 2.1.323 is the one to update to, until a run starts 2.1.323; then the item leaves quietly.

## Teardown
- `rm -f "$F" "$D/fake-version" "$ECHOCTL_HOME/.echo/claude-versions/work"; mv "$D/bin/claude.fake" "$D/bin/claude"`; `tests/fixtures/serve.sh down <dir>` and remove `<dir>`.

## Notes
- Step 4: the user's decision (#404) — an unreadable policy file asks instead of only refusing; see claude/TC-029.
- Machine-wide policy (`managed-settings.json` under `/Library/Application Support/ClaudeCode`, macOS managed preferences, Windows policy keys) is read by the same check but needs root: a person creates the file with `sudo` and removes it after, on test data only.
- Plugin runs and thread syncs use the same checks (`claudejob`, `projectsync`); this case checks the minutes run.
