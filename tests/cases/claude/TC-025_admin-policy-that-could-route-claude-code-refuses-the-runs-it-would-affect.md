---
id: TC-025
title: Admin policy that could route Claude Code refuses a Claude profile's minutes run and sign-in, and says to use "Claude Code (as configured)"
priority: high
type: functional
requirements: [F-AI-06]
module: claude
run: agent
platform: any
data: standard
tags: [claude, ai-profiles, policy, cli]
---

# TC-025: Admin policy that could route Claude Code refuses the runs it would affect

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set: `tests/fixtures/serve.sh up <dir> <port> --set standard` (the server is not used by the steps; the data and its env are). Every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`
- `F="$ECHOCTL_CONFIG_DIR/claude/work/remote-settings.json"`: the file Claude Code caches in the profile's own folder for a `claude` profile (server-managed settings) is one of the policy sources Echo reads. No file under `/Library/Application Support/ClaudeCode` is touched
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-initrode-intro)`: a transcribed call without minutes on the profile **Claude · work** (the data's fake `claude`, which refuses every run with "test data: Claude Code is a fake here")
- Wait 10 seconds after the server starts

**Changes:** `$F` (created, rewritten, removed); `<call>/notes-run.json` and `claude.log`

## Test Data

| Field | Value |
|-------|-------|
| Routing policy | `{"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:9"}}` |
| Helper policy | `{"apiKeyHelper": "/usr/bin/true"}` |
| Unreadable policy | the text `not json` |
| Harmless policy | `{"model": "sonnet"}` |
| The refusal | `your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F>)`; an unreadable file adds ` can't be read` inside the last parentheses: `(<$F> can't be read)` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl notes <call> --wait; echo "exit $?"` (no policy file yet) | `Error: test data: Claude Code is a fake here (log: …/claude.log)` and `exit 1`: the run was allowed to start |
| 2 | `echo '{"env": {"ANTHROPIC_BASE_URL": "http://127.0.0.1:9"}}' > "$F"`, then `target/debug/echoctl notes <call> --wait; echo "exit $?"` | `Error: your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F>) (log: …/claude.log)` and `exit 1` — not "test data: Claude Code is a fake here": Claude Code never started |
| 3 | `python3 -c "import json,sys;r=json.load(open(sys.argv[1]));print(r['state'], r['message'])" <call>/notes-run.json` | `failed your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F>)` |
| 4 | `target/debug/echoctl ai login work; echo "exit $?"` | `Error: your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F>)` and `exit 1`: no sign-in starts |
| 5 | `target/debug/echoctl ai status work \| grep problem` | `"problem"` holds the same sentence, ending `(<$F>)` |
| 6 | `echo '{"apiKeyHelper": "/usr/bin/true"}' > "$F"`, then `target/debug/echoctl notes <call> --wait` | `Error: your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F>) (log: …)` |
| 7 | `echo 'not json' > "$F"`, then `target/debug/echoctl notes <call> --wait` | `Error: your organisation's Claude Code policy decides where requests go — use the "Claude Code (as configured)" profile (<$F> can't be read) (log: …)`: a policy Echo can't read refuses too (to confirm with the user, see Notes) |
| 8 | `echo '{"model": "sonnet"}' > "$F"`, then `target/debug/echoctl notes <call> --wait` | `Error: test data: Claude Code is a fake here (log: …)`: a policy that can't route lets the run start |

## Expected Final State
While a policy that could route (a routing variable in `env`, a key helper, or a file that can't be read) is in the profile's folder, neither a minutes run nor a sign-in starts Claude Code, and each says where to go instead; a harmless policy changes nothing.

## Teardown
- `rm -f "$F"`; `tests/fixtures/serve.sh down <dir>` and remove `<dir>`.

## Notes
- To confirm with the user: step 7 (a policy file Echo can't read is refused too) and the refusal's wording for it are not stated in F-AI-06's text ("admin policy that could route Claude Code … refuses the runs it would affect"); the step stays as the safe reading, to be kept or dropped by the user's answer.
- The machine-wide sources (`/Library/Application Support/ClaudeCode/managed-settings.json` and its `managed-settings.d/`, macOS managed preferences, Windows policy keys) are read by the same check but need root or the registry: a person checks them (create the file with `sudo`, run step 2 with a real profile, remove it with `sudo rm`; never on the user's real `~/echo`). Not covered by this case.
- The profile "Claude Code (as configured)" (`system`) is not refused by this check: it is the user's own setup, so its policy is the user's own.
