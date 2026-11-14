# Dev Workflow

**Project**: Echo · **Scope**: build, run and debug a dev Echo; packaging

**Reference**: `.ai-run/reference/development/dev-workflow.md` — features, prerequisites, toolchains, packaging, run and
debug commands, logs (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Rules
- R1. Each agent works in its own git worktree from `origin/main` and builds there into its own target folder, never a shared one outside it; test steps run that worktree's build. Evidence: `make_data.py::Data` (ref: Worktrees)
- R2. A dev app runs beside the installed Echo only with its own config and data folders, never the real ones. Evidence: `app::record_app_path` (ref: A dev app beside Echo)
- R3. Run your build with `echoctl app run`, never `echoctl app start`. Evidence: `app::spawn_app` (ref: A dev app beside Echo)
- R4. Diagnostics never print an environment value. Evidence: `ai::terminal_command` (ref: Debugging)
