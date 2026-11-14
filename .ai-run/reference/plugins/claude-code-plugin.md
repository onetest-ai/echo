# Claude Code Plugin — reference

How the Claude Code plugin in `plugin/` is laid out. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/plugins/claude-code-plugin.md`.

## Launchers

Two launchers find the CLI: `plugin/scripts/echoctl` (sh, also Git Bash on Windows) and `plugin/scripts/echoctl.cmd`.
Both look in this order: `$ECHOCTL_BIN`, then `plugin/bin`, then `<config dir>/app-path`.

## Dev build

`scripts/build-plugin.sh` puts a dev build into `plugin/bin`: it copies to a temp name and renames
(`scripts/build-plugin.sh::mv`). Try it with `claude --plugin-dir plugin`.

## Skills

- A skill calls the CLI as `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` with plain words (no `$`, backticks, pipes or
  redirects); the guard holds a minutes run's Bash calls to `notes::RUN_COMMANDS`, as plain commands.
- An app-only skill (such as project-sync) starts its description with "Use only when…".
- `process-call` is one `SKILL.md` (`guard::own_call`).
- Exemplar for a skill Echo runs headless: `plugin/skills/process-call/SKILL.md::description` (no covering test).

## Status-line mod

- Parsing and time helpers: `plugin/hooks/view.ts` (`parseLines`), tested in `view.test.ts`; `register.tsx` wires them
  to the engine.
- `register.tsx` checks `ECHOCTL_HEADLESS` to do nothing inside a run Echo started.

## The plugin in runs

The plugin shipped with the app is passed with `--plugin-dir` to runs and sessions on `claude` / `local` profiles
(`notes::plugin_dir`).
