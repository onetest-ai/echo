# AGENTS.md

Primary AI entrypoint for `Echo`. Load the relevant `.ai-run/guides/` file before changing code.

## About Echo

A standalone desktop app (Rust, one binary `echoctl`: tray icon + call watcher + window + CLI) that records the Teams
calls the user opts in to — their mic and Teams' audio as two tracks — and transcribes them on-device, plus a Claude
Code plugin (`plugin/`) that is its client. The app never depends on Claude Code; the plugin drives it only through
the CLI and the data folder's files. Minutes and action items are written only by a Claude Code run Echo starts under
a guard. macOS and Windows both. Start with `.ai-run/guides/architecture/architecture.md` (overview, data layout, flows,
where new code goes); user-facing docs are `README.md` and `docs/install.md`.

## Working Rules

Behavioural rules for every change; for a trivial task, use judgment.

1. **Think before coding.** Don't assume, don't hide confusion, surface tradeoffs. State your assumptions; if several
   readings exist, present them instead of picking one silently; if a simpler approach exists, say so and push back
   when warranted; if something is unclear, stop, name it and ask.
2. **Simplicity first.** The minimum code that solves the problem, nothing speculative: no features beyond what was
   asked, no abstractions for single-use code, no unrequested flexibility or configuration, no error handling for
   impossible cases. If 200 lines could be 50, rewrite it. Would a senior engineer call it overcomplicated? Simplify.
3. **Surgical changes.** Touch only what you must; clean up only your own mess. Don't "improve" adjacent code,
   comments or formatting, don't refactor what isn't broken, match the existing style. Mention unrelated dead code,
   don't delete it; remove only the imports, variables and functions your change made unused. Every changed line
   traces to the request.
4. **Goal-driven execution.** Turn the task into checks you can verify — "add a feature" → its functional cases in
   `tests/cases/` first, then make them pass; "fix the bug" → a test that reproduces it, then make it pass;
   "refactor X" → tests pass before and after. For multi-step work, state a short plan with a
   check per step (`1. … → verify: …`), then loop until each check holds (for a review, the check is a reported list, not
   fixes). Here the final check is the gates in
   `.ai-run/guides/quality-gates.md`.

## Guide Imports

<!-- ai-run-init:guide-imports start -->
| Category | Guide Path | Purpose |
|---|---|---|
| Quality | .ai-run/guides/quality-gates.md | Quality gates and how to run them |
| Architecture | .ai-run/guides/architecture/architecture.md | where code goes: paths, control files, CLI vs skill, feature gates, per-OS files |
| Capture | .ai-run/guides/capture/capture-and-transcription.md | recording state machine, capture tracks, transcription engines and guards, live notes |
| Claude Code plugin | .ai-run/guides/plugins/claude-code-plugin.md | plugin/ launchers, skills, status-line mod, dev build |
| Data | .ai-run/guides/data/recall-store.md | Recall SQLite store schema changes and profile-scoped access |
| Development | .ai-run/guides/development/dev-workflow.md | day-to-day build, run and debug: features, prerequisites, dev app beside Echo, UI review, runtimes, packaging |
| Development | .ai-run/guides/development/rust-conventions.md | atomic writes, meta.json and state-file locking, call stamps, version bump |
| Development | .ai-run/guides/development/threads-and-ledger.md | threads, clients and the work ledger: evidence-checked proposals, user edits pinned, Microsoft 365 sends |
| Echo plugins | .ai-run/guides/plugins/echo-plugins.md | rules for Echo's side of plugins: capabilities, host services, grants, client-half ctx and slots, widget kinds, scripts, marketplaces |
| Echo plugins | .ai-run/guides/plugins/plugin-authoring.md | rules for writing, testing, publishing and updating a third-party Echo plugin |
| Integration | .ai-run/guides/integrations/claude-code-runs.md | starting Claude Code on AI profiles, strict routing, leases, the fake in tests |
| Security | .ai-run/guides/security/untrusted-text.md | headless-run guard, run allowlists, safe Markdown, path checks |
| Standards | .ai-run/guides/standards/git-workflow.md | commit, branch, PR and release conventions |
| Testing | .ai-run/guides/testing/testing.md | test design in the plan, functional cases and their gate, Rust test isolation, fakes, node tests |
| Window | .ai-run/guides/window/window-ui.md | the webview UI: escaping, IPC actions, compiled-in assets, testable app.js blocks |
<!-- ai-run-init:guide-imports end -->

## What the guides are

The guides hold rules: how Echo must work and how we work on it. Not the reasons behind a decision once made, and not a
record of what was built: what Echo does is the functional map (`tests/feature-map/`), how it is built is the code. Why something was decided
is answered in GitHub: the issue and its PR carry the reasoning and the alternatives weighed, so a "why X, not Y"
question is researched in closed issues and PRs (`gh issue list --state closed --search …`, `gh pr view`), never
written into a guide or a code comment. A guide changes only when a rule changes, in its own `Rules:` PR the user approves, never as a side effect of a feature or a refactor. Cite evidence
by symbol (`ledger::apply`), not by file path, so moving code doesn't touch the guides.

A reference is not a guide: it holds facts — an API, its limits and numbers, formats, payloads, what Echo does with
a file — under `.ai-run/reference/<area>/`. A rule points to the reference section it relies on; a reference holds no
rules. A reference changes in the same PR as the code it describes and is checked by that PR's review, with no `Rules:`
PR; when the code and a reference differ, the code wins and the reference is fixed.

## References

| Area | Reference Path | Holds |
|---|---|---|
| Echo plugins | .ai-run/reference/plugins/plugin-api.md | the plugin API: manifest, `uses` and the sheet, host and client `ctx`, events, store, widgets and charts, work rows, scripts, `agent/`, testing hosts, publishing (`echoctl plugin guide` prints it) |
| Echo plugins | .ai-run/reference/plugins/echo-host.md | Echo's side of plugins: store files and locks, script runs, sessions, the `.plugins` layout, dashboards' settings |

## Task Classifier

<!-- ai-run-init:task-classifier start -->
| Category | User Intent | Example Requests | P0 Guide | P1 Guide |
|---|---|---|---|---|
| Architecture | add a module, CLI command, app request or platform code | "add an echoctl subcommand", "make the app react to a CLI request", "add Windows-only code" | .ai-run/guides/architecture/architecture.md | - |
| Capture | change recording, capture, transcription or live notes | "change when a recording starts or stops", "add a speech engine", "change the vocabulary hint", "change live notes" | .ai-run/guides/capture/capture-and-transcription.md | - |
| Claude Code plugin | change the Claude Code plugin in plugin/ | "edit a skill", "change the launcher", "change the status line" | - | .ai-run/guides/plugins/claude-code-plugin.md |
| Data | change the Recall store | "add a column", "serve Recall data to a session" | - | .ai-run/guides/data/recall-store.md |
| Development | write or change files the app keeps | "write a new JSON state file", "change meta.json", "bump the version" | .ai-run/guides/development/rust-conventions.md | .ai-run/guides/development/dev-workflow.md |
| Echo plugins | build or extend Echo plugins | "add a ctx service", "add a capability", "add a slot", "test a plugin locally", "give a plugin a Today widget", "add a widget kind" | .ai-run/guides/plugins/echo-plugins.md | .ai-run/guides/plugins/plugin-authoring.md |
| Integration | run Claude Code or change AI profiles | "start a new Claude job", "change routing variables", "test a sign-in" | .ai-run/guides/integrations/claude-code-runs.md | - |
| Security | change what a run, plugin or page may do with untrusted text or paths | "let process-call use a new command", "render model output as HTML", "accept a path from a plugin" | .ai-run/guides/security/untrusted-text.md | - |
| Standards | commit, branch, PR, merge or release | "write a commit message", "cut a release" | .ai-run/guides/standards/git-workflow.md | - |
| Threads and ledger | change threads, the ledger, people or Microsoft 365 sends | "change how a proposal is checked", "add a field a user can pin", "send a Teams message from a thread" | .ai-run/guides/development/threads-and-ledger.md | .ai-run/guides/security/untrusted-text.md |
| Testing | write or run tests; plan any change in behaviour (its cases first) | "add a unit test", "add a window test", "plan a feature", "run the cases before a PR", "prepare a release test run" | .ai-run/guides/testing/testing.md | tests/app_profile.md |
| Window | change the Echo window | "add a button that calls the app", "add a setting", "add a node test for app.js" | .ai-run/guides/window/window-ui.md | - |
<!-- ai-run-init:task-classifier end -->

## Critical Rules

<!-- ai-run-init:critical-rules start -->
| Rule | Trigger | Action |
|---|---|---|
| Check Guides First | ANY task | Match request to category and load the P0 guide before searching broadly |
| Test design in the plan | planning ANY change in behaviour — a feature, a fix, a removal (before the first line of code) | Load `.ai-run/guides/testing/testing.md` and `tests/app_profile.md`; the plan has a **Tests** part, written before the code — testing R1 |
| Functional gate | before calling a change or fix done, or opening its PR | Rebuild, then run the touched cases with the gates — `.ai-run/guides/quality-gates.md` R1–R4 |
| Release test bar | a release, or merging a long-lived branch into `main` | `.ai-run/guides/quality-gates.md` R5 |
| Reviews and merges | asked to review code or a PR; a PR ready to merge | `.ai-run/guides/standards/git-workflow.md` R8 (review) and R5 (merge) |
| Git Operations | "commit" / "push" / "PR" / "MR" | Only then; load `.ai-run/guides/standards/git-workflow.md` |
| Map before push | before pushing a branch, opening its PR or merging it | Update the functional map; the PR's `Map:` line from `feature-map.py changes origin/main` — `.ai-run/guides/standards/git-workflow.md` R4, testing R6 |
| Shell | ANY shell command | Use bash/Linux-compatible syntax |
| A call's text goes only to its pinned AI profile; Claude Code starts through ai::claude_command | starting a Claude job or changing AI profiles | load `.ai-run/guides/integrations/claude-code-runs.md` |
| Echo plugins (echo-plugin.json, main.js host half, client.js) follow the plugin guide, not the Claude Code plugin in plugin/ | building or extending Echo plugins, capabilities, slots or marketplaces | load `.ai-run/guides/plugins/echo-plugins.md` |
| Never put real call data (transcripts, chat, screenshots of real meetings) in tests, fixtures or issues | writing tests, fixtures or bug reports | load `.ai-run/guides/testing/testing.md` |
| Others' words are data: headless runs stay inside the guard's allowlists, model or call text reaches HTML only through the safe writers | changing a skill Echo runs, a run's permissions, or rendering of call/model text | load `.ai-run/guides/security/untrusted-text.md` |
| Recording stays opt-in per meeting and transcription on-device or on a server the user connected; macOS and Windows both | changing capture, transcription, recording start or platform code | load `.ai-run/guides/capture/capture-and-transcription.md` |
<!-- ai-run-init:critical-rules end -->

## Commands

<!-- ai-run-init:commands start -->
| Need | Source Guide | Source Evidence | Notes |
|---|---|---|---|
| lint | .ai-run/guides/quality-gates.md | CONTRIBUTING.md; README.md (strict: -D warnings) | Load guide before running |
| type-check | .ai-run/guides/quality-gates.md | CONTRIBUTING.md; README.md | Load guide before running |
| test | .ai-run/guides/quality-gates.md | CONTRIBUTING.md; README.md | Load guide before running |
| mod-test | .ai-run/guides/quality-gates.md | scripts/check-mod.sh | Load guide before running |
| Git / review workflow | .ai-run/guides/standards/git-workflow.md | git history and project context | Load guide before git operations |
<!-- ai-run-init:commands end -->
