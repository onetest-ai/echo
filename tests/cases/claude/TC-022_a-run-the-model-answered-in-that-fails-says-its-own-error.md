---
id: TC-022
title: A minutes run the model answered in that fails later says its own error, not "can't use the model"
priority: medium
type: functional
requirements: [F-AI-08]
module: claude
run: agent
platform: macos
data: small
ai: fake
tags: [claude, minutes, failure, local-model, cli]
---

# TC-022: A run the model answered in that fails says its own error

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set: `tests/fixtures/make_data.py <dir> --ai fake`; every command runs after `. <dir>/env` as `target/debug/echoctl …` (the debug build from this checkout). `D="$(dirname "$ECHOCTL_HOME")"`
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-acme-weekly-sync)`
- **A fake like Claude Code on a local model**, in place of the data's fake `claude` (no model, no account). It prints the warning Claude Code prints for any model it doesn't know, answers one turn, then fails:
  `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # A minutes run: the local-model warning, one answer, then an error from the server (or, with `$D/fake-022` = `refused`, Claude Code's own refusal). Anything else: the data's fake.
  if [ -n "$ECHOCTL_RUN_CALL" ] && [ -z "$ECHOCTL_RUN_MODE" ]; then
    cat > /dev/null
    echo '[claude-code:unrecognized_model] {"model":"test-model:8b"}' >&2
    echo '{"type":"system","subtype":"init","session_id":"s-022","claude_code_version":"9.9.9"}'
    if [ "$(cat "$(dirname "$0")/../fake-022" 2>/dev/null)" = refused ]; then
      echo '{"type":"assistant","error":"model_not_found","message":{"model":"<synthetic>","content":[{"type":"text","text":"There'"'"'s an issue with the selected model (test-model:8b)."}]}}'
      echo '{"type":"result","is_error":true,"result":"There'"'"'s an issue with the selected model (test-model:8b)."}'
      exit 1
    fi
    echo '{"type":"assistant","message":{"model":"test-model:8b","content":[{"type":"text","text":"Reading the transcript."}]}}'
    [ "$(cat "$(dirname "$0")/../fake-022" 2>/dev/null)" = stream ] && exit 1
    echo '{"type":"result","is_error":true,"result":"API Error: 500 test server stopped answering"}'
    exit 1
  fi
  exec "$(dirname "$0")/claude.fake" "$@"
  ```

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync (small set) |
| What the run prints | the `unrecognized_model` warning (stderr), one answer, then "API Error: 500 test server stopped answering"; with `$D/fake-022` = `stream`, no result at all (a broken stream); with `refused`, only Claude Code's own refusal (an `error` turn of model `<synthetic>`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl notes <call> --wait` | Exit 1; it prints "API Error: 500 test server stopped answering" and the log's path; never "can't use the model" |
| 2 | `python3 -c "import json;r=json.load(open('<call>/notes-run.json'));print(r['state'], r['message'])"` | `failed`, then a message that starts "API Error: 500 test server stopped answering" |
| 3 | `grep -c unrecognized_model <call>/claude.log` | At least 1: the warning stays in the log |
| 4 | `echo stream > $D/fake-022; target/debug/echoctl notes <call> --wait; python3 -c "import json;print(json.load(open('<call>/notes-run.json'))['message'])"` | Exit 1; the message starts "Claude Code exited with 1" — neither "can't use the model" nor the warning line |
| 5 | `echo refused > $D/fake-022; target/debug/echoctl notes <call> --wait; python3 -c "import json;print(json.load(open('<call>/notes-run.json'))['message'])"` (Claude Code refusing the model: its only turn is its own error, model `<synthetic>`) | Exit 1; the message is "Claude Code can't use the model …" with where to change it (menu → AI for calls → Minutes model, `echoctl ai set … --notes-model <name>`) |

## Expected Final State
The runs the model answered in name what went wrong; neither sends the user to change a model that answered. The refused model still does.

## Teardown
- `mv "$D/bin/claude.fake" "$D/bin/claude"; rm -f "$D/fake-022"` (the data's fake back). The failed runs' state stays: no later stage reads it

## Notes
- A model Claude Code really refuses (its only turn is Claude Code's own error, model `<synthetic>`) is still "can't use the model": `claude/TC-012` step 2.
