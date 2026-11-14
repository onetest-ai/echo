# Claude Code Plugin

**Project**: Echo · **Scope**: plugin/ launchers, skills, status-line mod, dev build

**Reference**: `.ai-run/reference/plugins/claude-code-plugin.md` — launchers, lookup order, skill and mod layout
(`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## The plugin in plugin/

### Rules
- R1. The two launchers find the CLI in the same order; change it in both. Evidence: `plugin/scripts/echoctl::ECHOCTL_BIN` (ref: Launchers)
- R2. Put a dev build into `plugin/bin` only by copy-and-rename, never by overwriting the binary in place. Evidence: `scripts/build-plugin.sh::mv` (ref: Dev build)
- R3. Start a skill's `description` with "Use when…" (or "Use only when…" for an app-only skill) and keep triggers of different skills from overlapping. Evidence: `plugin/skills/process-call/SKILL.md::description` (ref: Skills)
- R4. Keep the status-line mod's parsing and time helpers pure and tested; the registration only wires them to the engine. Evidence: `plugin/hooks/view.ts::parseLines` (ref: Status-line mod)
- R5. In a skill, call the CLI through the plugin's launcher with plain words: no shell syntax. Evidence: `notes::RUN_COMMANDS` (ref: Skills)
- R6. The status-line mod never calls a model or asks for minutes, and does nothing inside a run Echo started. Evidence: `plugin/hooks/register.tsx::ECHOCTL_HEADLESS` (ref: Status-line mod)
- R7. Keep `process-call` one `SKILL.md`. Evidence: `guard::own_call`
- R8. Runs and sessions on `claude` / `local` profiles get the plugin shipped with the app. Evidence: `notes::plugin_dir` (ref: The plugin in runs)
