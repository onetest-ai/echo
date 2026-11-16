# tests/

Echo's functional tests: cases, scenarios, the functional map, fixtures and tools (`README.md`).

- **QA sessions start here** (`cd tests && claude`, a run: `claude --agent test-run-lead`): only a session started in
  this folder loads the manual-qa agents, their hooks and the Playwright MCP (`tests/.claude/`, `tests/.mcp.json`;
  `tests/install.sh` installs them once per machine). A session started at the repo root gets none of them at start;
  once it reads a file here, Claude Code adds this file and lists the team's skills, never its agents or the MCP.
- **Paths are from the repo root, and every tool gets them absolute.** A relative path resolves from `tests/` here, but
  `app_profile.md`, the cases and the scenarios write `tests/cases/…`, `tests/fixtures/…`, `target/debug/echoctl`:
  give Read, Glob, Grep, Write, Edit and the Playwright MCP's `filename` the repo root in front, and start every Bash
  call at the root (`cd "$(git rev-parse --show-toplevel)"` first; each Bash call is a fresh shell).
- Echo for the test agents (rules, test data, starting the app, routes): `app_profile.md`. How tests are designed and
  run: `../.ai-run/guides/testing/testing.md`.
