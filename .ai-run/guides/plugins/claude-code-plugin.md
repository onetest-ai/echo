# Claude Code Plugin

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: plugin/ launchers, skills, status-line mod, dev build

## The plugin in plugin/

### Rules
- R1. Change the CLI lookup order in both launchers, `plugin/scripts/echoctl` (sh, also Git Bash on Windows) and `plugin/scripts/echoctl.cmd`; both start with `$ECHOCTL_BIN`, then `plugin/bin`, then `<config dir>/app-path`. Evidence: `plugin/scripts/echoctl::ECHOCTL_BIN`
- R2. Put a dev build into `plugin/bin` with `scripts/build-plugin.sh`, which copies to a temp name and renames; never overwrite `plugin/bin/echoctl` in place. Evidence: `scripts/build-plugin.sh::mv`
- R3. Start a skill's `description` with "Use when…" (or "Use only when…" for an app-only skill such as project-sync) and keep triggers of different skills from overlapping. Evidence: `plugin/skills/process-call/SKILL.md::description`
- R4. Keep the status-line mod's parsing and time helpers pure in `plugin/hooks/view.ts` (no `$`) and test them in `view.test.ts`; `register.tsx` only wires them to the engine. Evidence: `plugin/hooks/view.ts::parseLines`
- R5. In a skill, call the CLI as `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` with plain words (no `$`, backticks, pipes or redirects). Evidence: `notes::RUN_COMMANDS`
- R6. The status-line mod never calls a model or asks for minutes, and does nothing inside a run Echo started (`ECHOCTL_HEADLESS`). Evidence: `plugin/hooks/register.tsx::ECHOCTL_HEADLESS`
- R7. Keep `process-call` one `SKILL.md`. Evidence: `guard::own_call`
- R8. Pass the plugin shipped with the app with `--plugin-dir` to runs and sessions on `claude` / `local` profiles. Evidence: `notes::plugin_dir`

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| writing a skill Echo runs headless | `plugin/skills/process-call/SKILL.md::description` | no covering test |
