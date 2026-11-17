---
id: TC-031
title: A minutes run through a wrapper gets its prompt only on stdin after Echo's hook answered, its tools wait for Echo's check, and a start that doesn't match stops it
priority: critical
type: functional
requirements: [F-AI-04, F-AI-12]
module: claude
run: agent
platform: macos
data: standard
tags: [claude, minutes, guard, wrapper, cli]
---

# TC-031: A minutes run through a wrapper gets its prompt only after Echo's check; a start that doesn't match stops it

**Module:** Claude Code | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set: `tests/fixtures/serve.sh up <dir> <port> --set standard`. Every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-initrode-intro)` (transcribed, no minutes)
- **A made-up company wrapper**, `$D/wrappers/acme-claude` (`chmod +x`), standing in for Claude Code as Claude Code behaves (measured on 2.1.325: session-start hooks report before any input; the start event comes with the first message):
  ```sh
  #!/bin/sh
  # acme-claude: a made-up company wrapper. `--version` answers as Claude Code. A print run: runs the session-start hook
  # of its --settings file as Claude Code would and reports it, takes the first message, reports a start event (the
  # permission mode in $w/mode, else dontAsk), runs the last PreToolUse hook (Echo's gate) for one Read and records its
  # exit code, then ends with a result.
  w="$(cd "$(dirname "$0")" && pwd)"
  case "$1" in --version) echo "2.1.400 (Claude Code)"; exit 0;; esac
  printf '%s\n' "$@" > "$w/args"
  s=""; prev=""; for a in "$@"; do [ "$prev" = "--settings" ] && s="$a"; prev="$a"; done
  hook() { python3 -c "import json,sys;h=json.load(open(sys.argv[1]))['hooks'][sys.argv[2]];print(h[-1]['hooks'][-1]['command'])" "$s" "$1"; }
  out=$(sh -c "$(hook SessionStart)" </dev/null)
  python3 -c "import json,sys;print(json.dumps({'type':'system','subtype':'hook_response','hook_event':'SessionStart','output':sys.argv[1]+'\n','stdout':sys.argv[1]+'\n'}))" "$out"
  IFS= read -r line; printf '%s\n' "$line" > "$w/stdin"
  mode=$(cat "$w/mode" 2>/dev/null || echo dontAsk)
  echo "{\"type\":\"system\",\"subtype\":\"init\",\"claude_code_version\":\"2.1.400\",\"permissionMode\":\"$mode\",\"tools\":[\"Read\"],\"mcp_servers\":[]}"
  echo '{"tool_name":"Read","tool_input":{"file_path":"/etc/hosts"}}' | sh -c "$(hook PreToolUse)" 2>"$w/gate-err"; echo "gate exit $?" > "$w/gate"
  echo '{"type":"result","subtype":"success","is_error":true,"result":"acme-claude stops here (a test wrapper)"}'
  exit 1
  ```
- **Shared asset:** the wrapper profile `acme` (claude/TC-030 step 1, or add it: `target/debug/echoctl ai add-wrapper acme --command "$D/wrappers/acme-claude" --name "Acme Claude"`)

**Changes:** `<call>/notes-run.json`, `<call>/claude.log`, `$D/wrappers/{args,stdin,gate,gate-err,mode}`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl notes <call> --profile acme --wait; echo "exit $?"` | `Error: acme-claude stops here (a test wrapper) (log: …/claude.log)`, `exit 1`: the run reached the wrapper and ended with its result |
| 2 | `cat "$D/wrappers/gate"` | `gate exit 0`: Echo's gate let the Read through to the guard — Echo had checked the start event and marked the run before the first tool call |
| 3 | `python3 -c "import json;print(json.load(open('$D/wrappers/stdin'))['message']['content'][:60])"` | `/echo:process-call <call>` …: the prompt came as one stream-json message on stdin |
| 4 | `grep -c "nobody is watching" "$D/wrappers/args"; grep -A1 -- "--input-format" "$D/wrappers/args"` | `0` (the prompt never on its command line); `--input-format` then `stream-json` |
| 5 | `ls "$ECHOCTL_CONFIG_DIR/minutes-run" | grep -c '^proof-'; ls -A "$ECHOCTL_HOME/.plugins/.proof"` | `0`, and nothing listed: the run's proof settings and its mark went with it |
| 6 | `echo bypassPermissions > "$D/wrappers/mode"; rm -f "$D/wrappers/gate"`, then `target/debug/echoctl notes <call> --profile acme --wait; echo "exit $?"` | `Error: Echo stopped this run: it started Claude Code in permission mode "bypassPermissions", not Echo's "dontAsk" (through <$D>/wrappers/acme-claude) (log: …)`, `exit 1` |
| 7 | `cat "$D/wrappers/gate"; python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'],'|',r['message'][:40])" <call>/notes-run.json` | No `gate` file (stopped before any tool call); `failed | Echo stopped this run: it started Claude` |
| 8 | `mv "$D/wrappers/acme-claude" "$D/wrappers/acme-claude.gone"`, then `target/debug/echoctl notes <call> --profile acme --wait; echo "exit $?"`; then move it back | `Error: Acme Claude starts <$D>/wrappers/acme-claude, which isn't there any more — install it again, or choose another AI profile (log: …)`, `exit 1`: never on another profile |

## Expected Final State
The call has no minutes; its last run failed, saying why.

## Teardown
- `rm -f "$D/wrappers/mode"`; `target/debug/echoctl ai remove acme`.

## Notes
- The wrapper doesn't write minutes: it shows where the prompt went, when the tools could run, and when Echo stopped it. Unit tests cover a wrapper whose settings never answer (no prompt sent, stopped within its limit) and one waiting on a sign-in (`ai::wrapper` tests).
