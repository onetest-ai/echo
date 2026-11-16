---
id: TC-018
title: Writing the minutes again in a run that fails after replacing them puts the previous minutes back
priority: high
type: functional
requirements: [F-MIN-13]
module: claude
run: agent
platform: macos
data: small
ai: fake
tags: [claude, minutes, failure, cli]
auto: [tests/auto/specs/S-09.spec.mjs]
---

# TC-018: A run again that fails keeps the previous minutes

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Small data set: `tests/fixtures/make_data.py <dir> --ai fake`; every command runs after `. <dir>/env` as `target/debug/echoctl …` (the debug build from this checkout). `D="$(dirname "$ECHOCTL_HOME")"`
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-acme-weekly-sync)`: it has made-up minutes (`notes.md`, "# Acme weekly sync")
- **A fake that writes over the minutes, then fails**, in place of the data's fake `claude` (no model, no account):
  `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # A minutes run: writes over notes.md, then fails. Anything else: the data's fake.
  if [ -n "$ECHOCTL_RUN_CALL" ] && [ -z "$ECHOCTL_RUN_MODE" ]; then printf '# Half-written minutes\n' > "$ECHOCTL_RUN_CALL/notes.md"; echo "test fake: stopped after writing" >&2; exit 1; fi
  exec "$(dirname "$0")/claude.fake" "$@"
  ```

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync (small set), minutes "# Acme weekly sync" |
| What the run writes | "# Half-written minutes", then it exits 1 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `shasum <call>/notes.md > $D/notes.sha` | The minutes' hash is saved |
| 2 | `target/debug/echoctl notes <call> --wait` | Exit 1; it prints "test fake: stopped after writing. The previous minutes are kept." and the log's path |
| 3 | `shasum -c $D/notes.sha; head -1 <call>/notes.md` | "OK": `notes.md` is byte for byte what it was; "# Acme weekly sync" |
| 4 | `python3 -c "import json;r=json.load(open('<call>/notes-run.json'));print(r['state'], r['had_minutes'], r['message'])"` | `failed True test fake: stopped after writing. The previous minutes are kept.` |
| 5 | `cat $ECHOCTL_HOME/.echo/trash/*-acme-weekly-sync.notes-*/notes.md; ls -a <call> | grep -c before-run; tail -2 <call>/claude.log` | "# Half-written minutes" (the run's version, kept in the trash); 0 (no copy left in the call's folder); the log says "The previous minutes are back; what this run wrote is in …/trash/…" |

## Expected Final State
The call has the minutes it had before the failed run; the failed run's text is in the trash.

## Teardown
- `mv "$D/bin/claude.fake" "$D/bin/claude"` (the data's fake back). The failed run's state, its trash folder and `$D/notes.sha` stay: no later stage reads them
