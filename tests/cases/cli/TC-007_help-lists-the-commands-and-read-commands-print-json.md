---
id: TC-007
title: echoctl --help lists the commands, each subcommand's --help lists its own, and the read commands print JSON
priority: low
type: functional
requirements: [F-CLI-01]
module: cli
run: agent
platform: any
data: standard
tags: [cli, help]
auto: [tests/auto/specs/S-28.spec.mjs]
---

# TC-007: echoctl --help lists the commands, each subcommand's --help lists its own, and the read commands print JSON

**Module:** CLI | **Priority:** Low | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- **In S-28** `echoctl project list` was run once on the data as built, in the scenario's prelude, into `<dir>/project-list.json` (it also feeds `cli/TC-008` step 1): step 6 reads that file instead of running it again. On its own, step 6 runs it

## Test Data

| Command | What it shows |
|---|---|
| `echoctl --help` | every command, one line each with its description, under "Commands:" (`help` last) |
| `echoctl project --help` | the thread commands, `ledger` among them |
| `echoctl status`, `echoctl project list` | JSON |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl --help` | Exit code 0; the first line is "Records the calls you opt in to and transcribes them on this computer (tray app + CLI)"; "Usage: echoctl <COMMAND>"; under "Commands:" one line per command, each a name and its description, among them `project`, `client`, `org`, `series`, `config`, `read`, `m365`, `context`, `actions`, `action`, `title`, `type`, `note`, `html`, `delete`, `restore`, `language`, and the last is `help` |
| 2 | Run `target/debug/echoctl project --help` | Exit code 0; "Usage: echoctl project <COMMAND>"; the commands `list`, `show`, `create`, `set`, `rule`, `source`, `include`, `exclude`, `reset`, `term`, `person`, `of`, `people`, `tasks`, `found`, `delete`, `ledger`, `help`, in that order |
| 3 | Run `target/debug/echoctl project ledger acme-rollout edit --help` | Exit code 0; the line "An edit of yours (JSON: {op: edit\|ac_add\|ac_set\|link\|unlink\|merge\|delete\|accept\|reject\|undo\|create, …})" and "Usage: echoctl project ledger <ID> edit <JSON>" |
| 4 | Run `target/debug/echoctl help series` | Exit code 0; the same text as `echoctl series --help`: "Usage: echoctl series <COMMAND>" with `list`, `set`, `help` |
| 5 | Run `target/debug/echoctl --version` | Exit code 0; one line `echoctl <x.y.z>`, the version Settings → General shows as "Echo <x.y.z>" |
| 6 | Run `target/debug/echoctl status` and `target/debug/echoctl project list` | Each prints one JSON object (it parses): `status` with the keys `agenda`, `armed`, `watcher`; `project list` with `clients`, `orgs`, `projects`, `stages` |
| 7 | Run `target/debug/echoctl frobnicate` | Exit code 2; "error: unrecognized subcommand 'frobnicate'" and "For more information, try '--help'." |

## Expected Final State
Nothing changed: help and read commands only.

## Notes
- The step names the commands other cases use, not their count (it changes whenever a command is added): a new command that a case uses joins the list with that case.
