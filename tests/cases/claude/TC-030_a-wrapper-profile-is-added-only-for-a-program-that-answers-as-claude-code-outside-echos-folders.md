---
id: TC-030
title: A wrapper profile is added only for a program that answers as Claude Code, outside Echo's folders, and never from a run
priority: high
type: functional
requirements: [F-AI-07, F-AI-11]
module: claude
run: agent
platform: macos
data: standard
tags: [claude, ai, profiles, wrapper, cli]
---

# TC-030: A wrapper profile is added only for a program that answers as Claude Code, outside Echo's folders

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set: `tests/fixtures/serve.sh up <dir> <port> --set standard`. Every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`
- **A made-up company wrapper**, `$D/wrappers/acme-claude` (`chmod +x`; outside the data folder `$D/home` and Echo's folder `$D/config`), the script of claude/TC-031's preconditions (here only its `--version` answer matters: `2.1.400 (Claude Code)`)
- **A program that isn't Claude Code**, `$D/wrappers/node-ish` (`chmod +x`): `#!/bin/sh` / `echo v22.1.0`

**Changes:** `<config dir>/ai.json` (the AI profiles), `$ECHOCTL_HOME/plugins/acme/bin/acme-claude`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl ai add-wrapper acme --command "$D/wrappers/acme-claude" --name "Acme Claude"; echo "exit $?"` | `added Acme Claude (acme)`, then `Acme Claude starts <$D>/wrappers/acme-claude (2.1.400 (Claude Code)); its jobs, call transcripts included, go wherever that program sends them` (the path resolved, `/private/…` on macOS), `exit 0` |
| 2 | `target/debug/echoctl ai status acme` | One profile: `"kind": "wrapper"`, `"command"` the full path of step 1, `"offered_by": null`, `"problem": null`, `"default": false` |
| 3 | `PATH="$D/wrappers:$PATH" target/debug/echoctl ai add-wrapper byname --command acme-claude; echo "exit $?"` | Found as the login shell finds it: `added acme-claude (byname)` and the same full path; `exit 0`. Then `target/debug/echoctl ai remove byname` |
| 4 | `target/debug/echoctl ai add-wrapper notclaude --command "$D/wrappers/node-ish"; echo "exit $?"` | `Error: <$D>/wrappers/node-ish doesn't answer --version as Claude Code does`, `exit 1` |
| 5 | `mkdir -p "$ECHOCTL_HOME/plugins/acme/bin" && cp "$D/wrappers/acme-claude" "$ECHOCTL_HOME/plugins/acme/bin/"`, then `target/debug/echoctl ai add-wrapper planted --command "$ECHOCTL_HOME/plugins/acme/bin/acme-claude"; echo "exit $?"` | `Error: <…>/home/plugins/acme/bin/acme-claude is inside Echo's data folder (where plugins and what they keep are): a wrapper is a program you installed`, `exit 1` |
| 6 | `ln -s "$ECHOCTL_HOME/plugins/acme/bin/acme-claude" "$D/wrappers/via-link"`, then `target/debug/echoctl ai add-wrapper linked --command "$D/wrappers/via-link"; echo "exit $?"` | The link is followed: the same `is inside Echo's data folder` refusal, `exit 1` |
| 7 | `target/debug/echoctl ai add-wrapper spaced --command "acme-claude --x"; echo "exit $?"` | `Error: “acme-claude --x” isn't a program name (letters, digits, “.”, “_”, “-”) or a path`, `exit 1` |
| 8 | `ECHOCTL_HEADLESS=1 target/debug/echoctl ai add-wrapper inrun --command "$D/wrappers/acme-claude"; echo "exit $?"` | `Error: refused inside a run Echo started: which program Echo starts as Claude Code is the user's to choose`, `exit 1` |
| 9 | `target/debug/echoctl ai status` | The data's own profiles (`system`, `work`, `other`, `out`, `ollama`) and `acme`: none of `notclaude`, `planted`, `linked`, `spaced`, `inrun` |

## Expected Final State
One wrapper profile, `acme`, starting `$D/wrappers/acme-claude`.

## Teardown
- Keep `acme` when claude/TC-031 follows; else `target/debug/echoctl ai remove acme`, `rm -rf "$ECHOCTL_HOME/plugins/acme" "$D/wrappers/via-link"`.

## Notes
- Step 8 sets what Echo sets for every run it starts (`ECHOCTL_HEADLESS`): the CLI refuses the change as it would inside one.
