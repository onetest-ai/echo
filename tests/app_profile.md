# Echo — app profile for functional tests

Echo is a desktop app (one Rust binary, `echoctl`: tray icon, call watcher, window, CLI) that records the Teams calls
the user opts in to, transcribes them on the computer, and has Claude Code write the minutes. Its window is a web page
(vanilla JS, `crates/echoctl/ui/`) shown in a webview — and the same page can be served on localhost, which is how an
agent tests it. This file is given to every manual-qa role; where it differs from a role's own rules, **this wins**.

## Rules

1. **Made-up data only.** Every run builds its own data folder with `tests/fixtures/make-data.sh <empty dir>`:
   Acme (acme.example), Globex (globex.example), Initech (the user's company), the user "Sam Rivera". Never open,
   copy or name the real data folder (`~/echo`), real calls, people or companies — not in cases, steps, reports or
   screenshots.
2. **The build under test** is `target/debug/echoctl` from this checkout. Build it first: `cargo build`. A case never
   installs, quits or restarts the real Echo app.
3. **Every `echoctl` command runs with the run's data folder**: `. <dir>/env` first (it sets `ECHOCTL_HOME` and
   `ECHOCTL_CONFIG_DIR` and unsets Claude Code's session variables). A command without it would touch the real data.
4. **Steps are the window (Playwright MCP) or the CLI (Bash).** The runner may use Bash only for `echoctl` with the
   run's env, reading files in the run's data folder, and `tests/fixtures/*`. No other scripts, no edits to the repo.
5. **`run: manual` cases are for a person** (a real call, Teams, macOS permissions, an install, the tray, Windows,
   Claude Code runs that need a signed-in account). The lead lists them in the report as `skipped (manual)` and never
   tries them.
6. **A PASS needs evidence**: a Playwright snapshot for a window step, the command's output for a CLI step.

## Starting the app under test

```bash
cargo build
dir="$(mktemp -d)/echo-test"; mkdir -p "$dir"
tests/fixtures/make-data.sh "$dir"
. "$dir/env"
ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve 47930 &   # base_url = http://127.0.0.1:47930
```

- `window --serve` answers the window's page and its API from the run's data. `ECHOCTL_SERVE_IPC=1` lets clicks
  reach the app (behind a cookie the page gets on load: always load `{{base_url}}/` first, from that origin).
- One port per run; stop the server at the end (`kill %1`, or the pid). Two runs never share a data folder or port.
- After a rebuild the page is new only after a reload of `{{base_url}}/`.
- Times in the data are relative to the run's start: build the data right before the run, not hours earlier.

## What the test data holds

| Thing | Value |
|---|---|
| The user | Sam Rivera, sam@initech.example (Initech, internal) |
| Clients | Acme (acme.example), Globex (globex.example) |
| Threads | **Acme rollout** (client Acme), **Globex pilot** (client Globex) |
| Calls today | **Acme weekly sync** (≈3 h ago; minutes, 2 action items, in Acme rollout), **Initech planning** (≈1.5 h ago; transcript, no minutes) |
| Call yesterday | **Globex pilot kickoff** (14:00; minutes, 1 action item, in Globex pilot) |
| Today's agenda | Acme weekly sync, Initech planning (both past), **Globex design review** (+1 h), **Industry webinar** (+1.5 h, not accepted), **Acme demo** (+2.5 h) |
| Tasks on Sam | **Send Acme the SSO checklist** (due today, Acme rollout), **Draft the Globex pilot plan** (Globex pilot) |
| Task on others | **Answer the security questionnaire** (Omar Haddad, Acme rollout) |

## The window

Left activity bar (icons, top to bottom): Today, Calls, Calendar, Threads, Tasks; Inbox and Settings at the bottom.
Routes (`{{base_url}}/#…`):

| Route | View |
|---|---|
| `#today` (or `#`) | Today: hello, **Your day** (meetings and calls in time order, the line "now"), **My work · this week**, Coming up today |
| `#call/<folder>` · `#call/<folder>/<tab>` | A call: tabs `notes`, `minutes`, `transcript`, `chat`, … |
| `#calendar` · `#calendar/day` · `#calendar/week` | Calendar |
| `#projects` · `#projects/<id>` · `#projects/<id>/<tab>` | Threads (UI name; `projects` in routes and the CLI) |
| `#client/<id>` | A client |
| `#tasks` | Tasks: piles I owe, owed to me, waiting, nobody |
| `#settings` · `#settings/<section>` · `#settings/plugins/<id>` | Settings |

Selectors: prefer roles and visible text (`getByRole('button', {name: 'Prepare'})`, headings such as "Your day").
Elements the app acts on carry `data-*` attributes (`data-call`, `data-go`, `data-prep-write`, `data-schedule`,
`data-tk`, `data-pj`); use them when text repeats. Thread ids are the name's slug (`acme-rollout`).

## CLI

`echoctl --help` lists everything; most commands print JSON. Useful in steps: `project list`, `project show <id>`,
`project tasks`, `project ledger <id> tree`, `status`, `context <call>`, `actions <call>`, `attention`.
`<call>` is a folder name under `$ECHOCTL_HOME/calls/` or its path.

## Fragile areas

- Today redraws every minute (the clock on the line "now"): take a snapshot again rather than reuse an old ref.
- Buttons that start Claude Code (Prepare, Write minutes, Send to Claude, Yolo) need a signed-in Claude Code account:
  a case may check the button and the dialog, not the run's result, unless it is `run: manual`.
- Microsoft 365 (mail, Teams, calendar sync) is not signed in on test data: those views show their sign-in state.
- The data folder's `.echo/` holds the app's state files; a case reads them only when its steps say so.

## Writing a case (Echo additions to the format)

Cases live in `tests/cases/<suite>/TC-NNN_<slug>.md` in the manual-qa format, with two more frontmatter fields:

- `run: agent | manual` — who can run it (rule 5).
- `platform: any | macos | windows` — `any` for the window and the CLI in serve mode.

Steps use `{{base_url}}` for the window and `echoctl …` (after `. <dir>/env`) for the CLI; test data comes from the
table above — a case that needs more adds it in its Preconditions with the `echoctl` commands that make it.
