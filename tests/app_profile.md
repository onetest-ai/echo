---
app_name: Echo
base_url: http://127.0.0.1:47930
auth_type: none
last_updated: 2026-11-08
---

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
   installs, quits or restarts the real Echo app, except the clean-install runs (S-15, S-16: "Clean-install runs"
   below), which install the release on a Mac where Echo has never been.
3. **Every `echoctl` command runs with the run's data folder**: `. <dir>/env` first (it sets `ECHOCTL_HOME` and
   `ECHOCTL_CONFIG_DIR` and unsets Claude Code's session variables). A command without it would touch the real data.
4. **Steps are the window (Playwright MCP) or the CLI (Bash).** The runner may use Bash only for `echoctl` with the
   run's env, reading files in the run's data folder, and `tests/fixtures/*`. No other scripts, no edits to the repo.
   `tests/fixtures/*` includes `recordings.py` (copies of the calls recorded once per build) and `wait.py` (wait for
   the app's state or a file, with a timeout). One exception: a case that says it records with the test call may also
   run TestCall, `afplay`, `nohup`, `kill`, `sleep`, `ls`, `grep`, `cat`, `find`, `date`, `printf`, `pgrep -P <the run's
   app pid>` and `scripts/fetch-nemo-speech.sh` into the run's config, and click Echo's own dialog through System Events
   by that dialog's pid (see "Recording with the test call").
   Another: the clean-install runs use the commands their cases name ("Clean-install runs" below).
5. **`run: person` cases are for a person** (a real call, Teams, macOS permissions, an install, the tray, Windows,
   Claude Code runs that need a signed-in account). The lead lists them in the report as `skipped (person)` and never
   tries them. A case or scenario that is `run: agent` but needs the user for a moment (a macOS prompt, a password,
   System Settings, a logout) lists those moments in `user_steps:`, and its steps say "**The user does:** …".
6. **A PASS needs evidence**: a Playwright snapshot for a window step, the command's output for a CLI step.
7. **Agent runs execute scenarios, not single cases** (see "Scenarios" below): one runner per scenario, fresh data
   per scenario, and **the browser stays open** from one runner to the next — never close the browser while a run goes on.
   **Clean up after yourself:** when a scenario (or a one-off check) is done, close its tab and stop its server
   (`serve.sh down <dir>`); at the end of a run only one blank tab is left and no test server runs (the user's own
   ports never touched).

## Starting the app under test

For a scenario, one command builds fresh data and serves the window on it, and one stops it:

```bash
cargo build                                            # once per run
tests/fixtures/serve.sh up "$(mktemp -d)/s01" 47930    # prints base_url=… env=<dir>/env
tests/fixtures/serve.sh down <that dir>                # after the scenario
```

What it does, step by step (to do it by hand):

```bash
dir="$(mktemp -d)/echo-test"; mkdir -p "$dir"
tests/fixtures/make-data.sh "$dir"
. "$dir/env"
ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve 47930 &   # base_url = http://127.0.0.1:47930
echo $! > "$dir/pid"                                              # the pid, for the stop step
```

- `window --serve` answers the window's page and its API from the run's data. `ECHOCTL_SERVE_IPC=1` lets clicks
  reach the app (behind a cookie the page gets on load: always load `{{base_url}}/` first, from that origin).
- One port per run; stop the server at the end with the saved pid: `kill $(cat "$dir/pid")` (`kill %1` only works
  in the shell that started the job, and every Bash call of an agent is a fresh shell). Check with
  `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:47930/` (no answer = stopped). Two runs never share a
  data folder or port; another dev server may already own 47911 — never touch it.
- The data lives in `$dir/home` (`ECHOCTL_HOME`; calls in `$dir/home/calls/<folder>`); `$dir/config` is
  `ECHOCTL_CONFIG_DIR`. Remove `$(dirname "$dir")` when the run is over.
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

Left activity bar (icons, top to bottom): Today, Calls, Calendar, Threads, Tasks; Settings at the bottom.
The bar is icon-only until "Expand the sidebar" (`data-cmd="bar-flip"`) is pressed. The top bar (`#r-top`) holds, left
to right: that switch, the Record pill (Record, the call's language, Memo) and, at its right end, the account chip
(`.acct`: the default AI profile) and the Inbox icon. **Inbox is not a view**: it is a tab of the right-hand panel
(button `data-panel-flip="needs"`, in the top bar), there is no `#inbox` route. Viewport used: 1201x868.
Routes (`{{base_url}}/#…`; load `{{base_url}}/` first, then a route can be opened by `browser_navigate` to
`{{base_url}}/#…` — it changes the hash without reloading):

| Route | View |
|---|---|
| `#today` (or `#`) | Today: hello, **Your day** (meetings and calls in time order, the line "now"), **My work · this week**, Coming up today |
| `#calls` · `#call/<folder>` · `#call/<folder>/<tab>` | Calls list + a call: tabs `minutes`, `transcript` (a call opens on the tab last used, `minutes` at first, kept from call to call; `doc()` falls back to `minutes` only when the kept tab does not exist for the call). `#calls` alone depends on how it is reached: a **full page load** of `{{base_url}}/#calls` (goto from another page, or a reload) opens the newest call and rewrites the URL to `#call/<folder>/minutes` (the start-up code sets `S.selected` to the first call), and so does the activity bar's Calls button `[data-go="calls"]`; a **hash change** to `#calls` from another view (`browser_navigate` after the page is loaded, `location.hash`) picks no call and shows "Pick a call". A case that needs a call open clicks its row `[data-call="<folder>"]` |
| `#calendar` · `#calendar/day` · `#calendar/week` | Calendar. A full load of `#calendar` shows the Week view ("Week" pressed) and keeps the URL at `#calendar`; clicking "Week" or "Day" sets `#calendar/week` or `#calendar/day`. A hash change from `#calendar/day` to `#calendar` stays on Day |
| `#projects` · `#projects/<id>` · `#project/<id>/<tab>` | Threads (UI name; `projects` in the CLI). The app itself writes `#project/<id>/<tab>` (singular), tabs `brief`, `work`, `calls`, `people`, `profile`; the plural form is also accepted. `#projects` (Threads button) opens the last thread on its `work` tab |
| `#client/<id>` · `#client/<id>/<tab>` | A client, tabs `overview`, `calls`, `tasks`, `people`, `profile` (opened from a thread's "Thread actions" → "Open Acme") |
| `#tasks` | Tasks: piles I owe, owed to me, waiting, nobody, plus a folded "Others" pile |
| `#settings` · `#settings/<section>` · `#settings/plugins/<id>` | Settings; `#settings` lands on `general`. Sections: `general`, `recording`, `transcription`, `ai`, `templates`, `calendar`, `brain` (labelled "Knowledge"), `me`, `deliver` ("Sends to you"), `plugins`, `layout`, `health`. `plugins/<id>` needs an installed plugin (none on test data) |

Not routes: the Details drawer, My notes, Vocabulary, Asks and Inbox are panels on the right (or bottom); the
actions palette (`Actions ⌘K`, `[data-pal-open]`) is a dialog. A "Knowledge" activity (`#brain`) exists only while
Recall is on; it is off on test data.

Live notes are in **My notes** (`[data-panel-flip="notes"]`; the right panel `aside#r-right`, outside `main`), not in the
Transcript tab, which is the transcript alone at full width: a box `section[aria-label="Live notes"]` above the list (Now
`.nl-now`, the status line `.nl-status`, the ⋯ menu `[data-live-menu]` → `.nl-menu` with the call's model
`[data-callprofile]`, Pause / Resume while recording, Ask Claude about this call), then one list `#nt-list` where the
live notes' items (`.nt-entry.nt-livei`, marked ✦, read-only) sit among the user's notes by time, and the answers
under "Asked about this call". A call with no live notes kept (or live notes off) has no box and no ✦ rows.

Selectors: prefer roles and visible text (`getByRole('button', {name: 'Prepare'})`, headings such as "Your day").
Elements the app acts on carry `data-*` attributes (`data-call`, `data-go`, `data-prep-write`, `data-schedule`,
`data-tk`, `data-pj`); use them when text repeats. Thread ids are the name's slug (`acme-rollout`). Task ids
(`T001`) are per thread: Acme rollout and Globex pilot both have a `T001`, so pair `data-node` with `data-project`.

## CLI

`echoctl --help` lists everything; most commands print JSON. Useful in steps: `project list`, `project show <id>`,
`project tasks`, `project ledger <id> tree`, `status`, `context <call>`, `actions <call>`, `attention`.
`<call>` is a folder name under `$ECHOCTL_HOME/calls/` or its path.

## Fragile areas

- Today redraws every minute (the clock on the line "now"): take a snapshot again rather than reuse an old ref.
- Buttons that start Claude Code (Prepare, Write minutes, Send to Claude, Yolo) need a signed-in Claude Code account:
  a case may check the button and the dialog, not the run's result, unless it is `run: person`.
- Microsoft 365 (mail, Teams, calendar sync) is not signed in on test data: those views show their sign-in state.
- The data folder's `.echo/` holds the app's state files; a case reads them only when its steps say so.
- **First paint is empty.** Right after `browser_navigate` the snapshot can show `main` empty and an extra
  "Knowledge" activity button (it vanishes once the state is loaded). Wait for text first
  (`browser_wait_for` "Your day" / the view's heading), then snapshot. Same after clicking a view button: re-snapshot.
- **The minutes are in an iframe**: `iframe#minutes-frame` (`sandbox="allow-scripts"`, cross-origin, so
  `evaluate` cannot read it and `browser_wait_for text` does not see its text — it times out). The accessibility
  snapshot does list its content (refs `f1eN`); to act inside use `iframe#minutes-frame >> internal:control=enter-frame >> .ts >> nth=0`
  (time chips are `.ts`), or `page.frameLocator('#minutes-frame')` in `browser_run_code_unsafe`.
- **A right panel narrows the minutes.** The minutes frame takes the whole Minutes tab (What moved is a right panel
  view, not a column beside it): at 1201 px with the calls list and a right panel open it is about 380 px wide, with the
  panel closed about 700 px. Close the panel (`[aria-label="Close the panel"]`) when a step needs the minutes wide.
- Menus: "Call actions" and "Thread actions" open on click (no hover needed) and close on Escape. Native
  `<select>`s (Thread, Who does it, Order, Group by, Layout, Kind, Owner; task Status once its button is clicked) need `browser_select_option`, not clicks.
- **`browser_run_code_unsafe` keeps no globals between calls**: each call runs in a fresh context, so `globalThis.x` set in one call is gone in the next. Keep state across calls on the `page` object (`page.__dlg`), which is the same page from call to call; the hostile cases' guard and check D do so.
- **The served window sends no Content-Security-Policy header**: `window --serve` forwards only the Content-Type of `respond`'s answer (`window/mod.rs`, the serve loop), so the window's own CSP and the minutes page's header policy apply only in the native webview. The minutes page also carries its policy in a `<meta http-equiv="Content-Security-Policy">` tag, which the browser enforces in both; read the policy there.
- **Reload after a CLI change.** A hash-only Navigate does not refetch data after an `echoctl` step that changed it: reload the page fully (`location.reload()` or reload `{{base_url}}/`), then navigate.
- Transcript "Find" highlights (class `hit` on `.turn`) a moment after the text is entered; read the DOM after a short wait.
- A right panel keeps its last tab: after opening a task it shows Details, and clicking Inbox flips it back.
- Settings → General and Health read the **real machine**, not the test data: "Command line", "Claude Code plugin"
  show Installed, Health shows the real Claude Code path, version and macOS permission state, "Echo plugin" lists real
  Claude Code sessions. A case must not assert them, and a screenshot of Health shows a real home path: do not keep it.
- Settings → Plugins → Discover lists the built-in marketplace (`onetest-ai/echo-plugins`, "3 plugins · read today");
  its content comes from a read of that repository (network or cache), so assert only the tabs, not the plugin list.
- Known issues seen on the test data (2026-11-06; verify before relying on them):
  - `#call/<folder>/minutes`: the time chip `00:37` (first Summary line) highlights the turn at 0:29, not 0:37 —
    the jump picks the first turn whose end is within 2 s of the chip (`end >= t - 2`) and the 0:29 turn ends at 0:35.
    `window/TC-004` expects the 0:37 turn, so it fails on this data. The same off-by-one hits chip `00:13` (turn 0:05).
  - Every page load logs one console error: `favicon.ico` answers 404. It is the only console message in a whole session.
  - Settings → "Search settings" filters the section names only: "Dock" (a setting in General) finds nothing and
    no "no match" message shows.
  - One thing, three names: Settings nav "Knowledge", its page "Recall", a thread's Profile "turn it on in Settings → Brain".
  - Task filter "Who does it" → "Claude can do" or "By hand" shows every pile empty on test data: no task has a mode
    set (`mode: null` in `echoctl project tasks`), so those two filters can only be tested after a task's
    "Claude can" / "By hand" toggle (`data-mode-set`) is pressed.

## What else the test data holds (seen in the window)

| Thing | Value |
|---|---|
| Call people | Acme weekly sync: Sam Rivera, Dana Lee, Omar Haddad · Initech planning: Sam Rivera, Alex Chen · Globex pilot kickoff: Sam Rivera, Kim Park |
| Call folders | `2026-11-06_HHMMSS-acme-weekly-sync`, `2026-11-06_HHMMSS-initech-planning`, `2026-11-05_140000-globex-pilot-kickoff`: **today's two carry the run's start time, so a folder name differs on every run** (a hard-coded `#call/2026-11-06_172025-…` gives "This call can't be opened · Error: 404"). Reach a call by clicking its row: `[data-call$="acme-weekly-sync"]`, or read the folder from `ls $ECHOCTL_HOME/calls` |
| Meeting kind chip | Acme weekly sync = Client, Initech planning = 1:1, Globex design review = Client |
| Acme weekly sync transcript | five turns at 0:05, 0:13, 0:21, 0:29, 0:37 (Me / Remote); minutes: 2 Summary lines, 1 decision, 2 action items, 1 open question, each with a time chip |
| Upcoming | Globex design review (Kim Park), Acme demo (Dana Lee) — "Coming up today · 2" in Calls; Industry webinar shows in the calendar but not in Today |
| Acme rollout | client Acme, stage Delivery · Active; work tree: epic/workstream "Acme rollout", story **S001 SSO for the Berlin pilot** (criteria 0 / 1), tasks **T001** (Sam, due today) and **T002** (Omar) — all "to do" |
| Globex pilot | client Globex, Delivery · Active; story **Pilot plan** with task **T001 Draft the Globex pilot plan** (Sam, no date) |
| Initech | the user's company ("Initech · your company"), no threads |
| Not on the data | Brief text, people in a thread, meeting series, installed plugins, Microsoft 365, "Asks", live notes — empty states |
| Today numbers | "4 meetings · 1 task on you" (Draft the Globex pilot plan has no date, so only the SSO task is in "My work · this week") |

## Key Pages

| Page | URL | Description | Roles |
|---|---|---|---|
| Today | `#today` | Greeting, Your day (Acme weekly sync with "New minutes" + "1 decision · 2 actions · 1 question", Initech planning "Open the call", the "now" line, Globex design review and Acme demo each with "Prepare" and "Record when it starts"), My work · this week (task card) | user |
| Calls | `#calls` | List grouped Coming up today / Today / Yesterday; search box "Search calls" (title and people); "Vocabulary" button | user |
| A call | `#call/<folder>/minutes` · `/transcript` | Title, date, length, kind chip, attendees, thread chip + "+ Thread", "My notes" toggle, "Call actions" menu (Show folder, Open minutes in browser, Show minutes in Finder, Write minutes again, Write again with Default/Discussion, Edit templates…, Ask Claude about this call, Delete…), tabs Minutes/Transcript, "What moved" aside (thread + "Sync now"). Call without minutes: Minutes tab says "No minutes yet" with "Write minutes" and a copyable `/echo:process-call <folder>` | user |
| Calendar | `#calendar/week` · `#calendar/day` | Day/Week toggle, Earlier ‹ / Today / Later ›, banner "Showing today's meetings only. Connect Microsoft 365…", one card per meeting; a card opens a side card (organizer, people, "Record when it starts", language, "+ Thread", "Prepare") | user |
| Threads | `#projects` → `#project/<id>/<tab>` | Left list grouped by client (fold "Hide its threads", client link `data-pj="client"`, thread `data-pj="open"`), "New thread", "Found for you". Tabs: Brief ("What it is" + Edit, Latest calls), Work (Board/Tree saved views, + New view, filters Kind/Owner/Open only/Group by/Fields, Sync now, Board columns To do · In progress · Waiting · Blocked · Done), Calls (pinned calls, "Unpin"), People (Map · Org chart · List, + Person), Profile (stage, client, organisations, since, calls rules, meetings, sources, notes for Claude, look, AI profile) | user |
| A client | `#client/<id>/<tab>` | Overview (About + Edit, its threads with story and task counts), Calls, Tasks (piles), People, Profile; "Claude about Acme" | user |
| Tasks | `#tasks` | Filters Thread / Who does it / Order (Due, Quiet longest, Thread); piles "I owe 2", "Owed to me 0", "Waiting for others 0", "Nobody named 0"; folded "Others · 1" (Omar's task); a task opens the Details drawer | user |
| Inbox | right panel tab | "Open" / "Done" + "this week"; empty: "Nothing needs you" (Open), "Nothing decided this week yet." (Done) | user |
| Right panel | any view | Tabs Inbox, Vocabulary (names and terms, "Add a term", learned from calls), Details (a task), Asks, My notes (on a call); buttons "Move to the bottom panel", "Close the panel" | user |
| Details drawer | right panel, tab Details | A task: id, kind, "Send to Claude", Edit, title, thread › story, description Add, Status (a button: click it to get the select "Status"), Owner, Handed over by, Due, Who does it (Claude can / By hand), Link "Copy link" (`echo://task/<thread>/<id>`), link to another thread, History with "undo" | user |
| Settings | `#settings/<section>` | Left list + "Search settings" (matches section names only). General: data folder, Appearance System/Light/Dark, Start at login, Hide from the Dock, Command line, Claude Code plugin, version "Echo 0.8.2", Check for updates. Recording: when to record (Ask/Manual), languages, new recordings language, during a call switches. Transcription: speech models (Download), server, audio. AI & minutes: profiles, minutes switches. Templates: Minutes 4 · Meeting types 6 · Skills 1. Calendar & chat: Microsoft 365 "Sign in…". Knowledge: Use Recall. About me. Sends to you. Plugins. Layout. Health | user |
| Plugins | `#settings/plugins` | Tabs Installed ("No plugins yet — Discover lists what your marketplaces offer."), Discover (category chips All/Automation/Chat/Knowledge, "Search plugins", Featured: Obsidian vaults 0.7.1, Scheduler 0.3.0, Telegram 0.3.1, each with "Install"), Marketplaces 1 (onetest built in; "Read again"; "Add a marketplace" owner/repo + Name + Add) | user |
| Actions palette | dialog, `Actions ⌘K` | Input `[aria-label="Action"]`, groups TODAY, MY WORK, ECHO, CLAUDE, GO TO (every view and "Settings: <section>"), THREADS, and calls and people while typing; Enter runs, Esc closes | user |

Only one role exists (the user, Sam Rivera); there is no login.

## Reliable Selectors

All verified on the test data in a live session. Roles and names first, then the app's own attributes.

| Element | Selector | Page | Notes |
|---|---|---|---|
| Activity buttons | `nav[aria-label="Activities"] [data-go="today"\|"calls"\|"calendar"\|"projects"\|"tasks"\|"settings"]` | all | Inbox is `[data-panel-flip="needs"]` in the top bar, not in this nav; role `button` named "Today", "Calls", … . Scope to the nav: on Today a second `[data-go="tasks"]` (the "Tasks" link in My work) makes the bare selector ambiguous |
| Expand/fold sidebar | `[data-cmd="bar-flip"]` | all | label switches "Expand the sidebar" / "Fold the sidebar" |
| Record controls | `[data-cmd="record"]`, `[data-cmd="record-memo"]`, `select[aria-label="Language of the call"]` | top bar | do not press Record on test data; the pill holds no profile picker |
| Account chip | `#r-top .acct`: with two or more profiles a label over `select[aria-label="AI profile"]` (choosing one makes it the default, as Make default); with one a button named "AI profile: <name>" that opens `#settings/ai` | top bar, left of Inbox | a native select: `browser_select_option` |
| Actions palette | `[data-pal-open]`, input `[aria-label="Action"]`, dialog `.pal`, list `#pal-list` | all | |
| Right panel | `[aria-label="Right panel"]`; tabs `[data-panel-show="needs"\|"vocabulary"\|"work-item"\|"asks"]`; `[aria-label="Close the panel"]`, `[aria-label="Move to the bottom panel"]` | all | the My notes tab is `[data-panel-show]` too |
| Call row | `[data-call="<folder>"]` (role `option`, in `[role=listbox][aria-label="Calls"]`) | #calls | Today's "Open the call" / "New minutes" use `[data-call]` and `[data-call-tab="minutes"]` |
| Search calls | `[aria-label="Search calls"]` | #calls | filters by title and by person ("Dana") |
| Call tabs | `role=tab[name="Minutes"]`, `role=tab[name="Transcript"]` | #call | |
| Call menu | `[aria-label="Call actions"]`, then `role=menuitem[name="…"]` | #call | |
| My notes toggle | `[aria-label="My notes"]` (a toggle, `aria-pressed`) | #call | panel box `[aria-label="New note"]` (Enter adds), buttons Note / ? Question / ! Decision / ☐ To-do |
| Minutes frame | `iframe#minutes-frame` → `.ts` time chips | #call/minutes | see Fragile areas |
| Transcript | `[aria-label="Find in transcript"]` / `#find`; turns `.turn[data-t]` (`me`/`remote`), match = class `hit`; speaker `.who` ("Me" + a `.lang` chip "en") | #call/transcript | the visible name reads "Meen": Me + the language chip |
| What moved | a right panel view: the call header's button `[data-panel-flip="moved"]` (beside My notes, with the count of changes; only for a call in a thread), its panel tab `[data-panel-show="moved"]` (the same count), the view `#r-right .mv`; "Sync now" `[data-cc-sync="<thread>"]` | #call | the minutes take the whole tab; Sync now starts Claude Code: do not press |
| Kind of meeting | `select[aria-label="Kind of meeting"]` | #call, calendar card | |
| Calendar | `role=button[name="Day"]`, `"Week"`, `"Earlier"`, `"Today"`, `"Later"`; cards `role=button[name=/Globex design review/]` | #calendar | the side card stays open when Day/Week is switched |
| Today cards | `[data-tk="open"][data-node="T001"]`, `[data-schedule="ev-globex-review"]`, `[data-prep-write="<hash>"]`, `[data-today-close="T001"]`, `[data-claude-task="T001"]` | #today | `data-schedule` ids: `ev-globex-review`, `ev-acme-demo`; Prepare/Close/Claude start Claude Code or write: not pressed |
| Thread list | `[data-pj="open"][data-id="acme-rollout"]`, `[data-pj="client"][data-id="acme"]`, `[data-pj="fold"][data-id="acme"]`, `[data-pj="new"]`, `[data-pj="found"]` | #projects | |
| Thread tabs | `[data-pj="tab"][data-tab-id="brief"\|"work"\|"calls"\|"people"\|"profile"]` | #project | the tab label carries the count ("Work3", "Calls1") — use the attribute |
| Thread menu | `[aria-label="Thread actions"]` → `role=menuitem` "Open Acme", "Claude about this thread", "Sync now", "Delete…" | #project | |
| Work views | `role=button[name="Board"]`, `"Tree"`, `"+ New view"`; selects `[aria-label="Layout"]`, `"Kind"`, `"Owner"`, `"Group by"`; `role=button[name="Open only"]`, `"Fields ▾"` | #project/work | a board card is `[data-pj="node"][data-node="T001"]` (also `data-drag`) |
| Client tabs | `[data-pj="ctab"][data-tab-id="overview"\|"calls"\|"tasks"\|"people"\|"profile"]` | #client | `[data-pj="brief-edit"]` edits About |
| Tasks filters | `[aria-label="Thread"]` (option "Globex pilot · 1" or value `globex-pilot`), `select[aria-label="Who does it"]` (bare `[aria-label="Who does it"]` also matches the button group in Details), `[aria-label="Order"]` | #tasks | |
| Task card | `[data-tk="open"][data-project="<thread>"][data-node="<id>"]`; "Others" `[data-tk="more"][data-pile="others"]` (on a client's Tasks tab `[data-pile="client-others"]`) | #tasks, #client/<id>/tasks | |
| Task status | click the Status button (`[data-pj="edit-field"][data-field="state"]`), then `[aria-label="Right panel"] select[data-ledger-set="state"]` (options to do, in progress, blocked, done, cancelled) | Details | verified: change shows in History ("you set state · undo") and `echoctl project tasks` shows `in_progress` |
| Task mode | `[data-mode-set="claude"\|"manual"]` | Details | |
| Settings nav | `[data-setsec="general"\|"recording"\|…\|"health"]`; search `[aria-label="Search settings"]` | #settings | switches `[data-set="<key>"]` with `role=switch` (`autostart`, `hide_dock_icon`, …) |
| Plugin tabs | `role=tab[name="Installed"]`, `"Discover"`, `role=tab[name=/Marketplaces/]` | #settings/plugins | tab change does not change the hash |

## Suggested Test Suites

Areas that have **no `run: agent` case yet** (from `tests/README.md` → Areas): Tasks, Threads (brief/work/calls/people/profile),
clients, Inbox and right panels, Settings (all sections), Plugins, the actions palette, Calls extras (What moved,
Call actions menu, thread chip), Today's task details (only TC-003). `windows` and the rest of `claude`, `install`, `permissions`, `lifecycle` stay person-run (their S-15/S-16 cases are agent runs with user steps) (`recording`, and part of `privacy`,
`transcription`, `window` and `cli`, now run with the test call). Proposed, all on the test data, `platform: any`:

| Suite | Folder | Priority | Description |
|---|---|---|---|
| smoke | tests/cases/smoke/ | Run on every deploy | One pass over the happy paths below: Today loads with its three blocks, a call opens minutes and transcript, Tasks shows the piles, a thread opens on its work board, Settings opens every section without an error text, no console error but the favicon 404 |
| tasks | tests/cases/tasks/ | High, weekly | (1) piles and counts match `echoctl project tasks` ("I owe 2", Others · 1); (2) Thread filter to Globex pilot leaves one task and drops the Others pile; (3) Order Due / Quiet longest / Thread keeps the same tasks; (4) a task card opens Details with thread › story, owner, due; (5) Status select to "in progress" shows History "you set state" and `project tasks` state `in_progress`, then undo; (6) "Who does it" filters are empty until a task is set to "By hand" (then it appears) |
| threads | tests/cases/threads/ | High, weekly | (1) Threads lists Acme rollout under Acme and Globex pilot under Globex, Initech has none; (2) Work tab: Board columns To do 2, Tree shows S001 > T001, T002 and "criteria 0 / 1"; (3) Group by Story / Owner and Owner filter "Omar Haddad" leave the right cards; (4) Calls tab lists Acme weekly sync "added by hand" with Unpin, and the row opens the call; (5) Profile tab shows stage Delivery · Active, client Acme, AI profile "The default"; (6) People tab shows "Nobody in this thread yet" and "2 people were in its calls"; (7) Brief "Nothing yet" |
| clients | tests/cases/clients/ | Medium | (1) "Thread actions" → "Open Acme" lands on `#client/acme/overview` with "1 thread · acme.example"; (2) Overview lists Acme rollout with "0 of 1 story done" and "1 I owe"; (3) Tasks tab: I owe 1 (SSO checklist), Others · 1; (4) Calls tab lists only Acme weekly sync; (5) Initech shows as "your company" without threads |
| inbox | tests/cases/inbox/ | Medium | (1) Inbox button opens the panel on the Inbox tab, Open shows "Nothing needs you" on clean data; (2) Done shows "Nothing decided this week yet."; (3) panel switches Inbox / Vocabulary / Details and keeps the last tab; (4) "Move to the bottom panel" docks it, Settings → Layout "Reset" restores; (5) Vocabulary: add a term (`Add`) and see it under EVERY CALL, then check `echoctl vocab` |
| calls-extras | tests/cases/window/ (extend) | Medium | (1) "Call actions" lists the 10 items and Escape closes it; (2) "Coming up today" expands to Globex design review and Acme demo, not the webinar; (3) the thread chip "Acme rollout" opens the thread, "What moved" names it; (4) a call without minutes (Initech planning) opens on Minutes (the tab is kept from call to call; it opens on Transcript only if Transcript was the last tab) and Minutes says "No minutes yet" with a "Write minutes" button (not pressed); (5) kind-of-meeting select shows Client / 1:1 as guessed; (6) fix the time-chip case (see Known issues) |
| settings | tests/cases/settings/ | Medium | (1) every `data-setsec` section opens with its heading and no error text; (2) General: Appearance Dark sets the document theme and System restores; (3) Recording: the two Ask/Manual groups and the English language chip; (4) Transcription: models list shows "Download (574 MB)" and no download is started; (5) Calendar & chat shows "Not connected" with "Sign in…" (not pressed); (6) "Search settings" "rec" leaves only Recording, clearing shows all 12; (7) Templates counts Minutes 4 / Meeting types 6 / Skills 1 |
| plugins | tests/cases/plugins/ | Medium | (1) Installed tab says "No plugins yet"; (2) Discover shows the category chips and Featured cards each with "Install" (not pressed); (3) category chip "Chat" leaves Telegram only; (4) "Search plugins" "sched" leaves Scheduler; (5) Marketplaces tab shows the built-in "onetest" with "Read again" and the Add form (not submitted); installs, updates and a marketplace add stay manual |
| palette | tests/cases/window/ (extend) | Low | (1) Actions opens the dialog with TODAY, MY WORK, ECHO, GO TO groups; (2) typing "Globex pilot" lists the thread and its call, Enter opens `#project/globex-pilot/brief`; (3) "Settings: Plugins" opens that section; (4) Esc closes and focus returns |

## Recorded calls (record once, use everywhere)

Recording and transcribing a call is a **prerequisite** of most recording, minutes, window and privacy cases, not what
they test. So it is done **once per build**, before the run, and every scenario copies the result:

```bash
tests/fixtures/recordings.py prepare     # ~4 min, once per build: outside any sandbox (inside one the audio is silent)
tests/fixtures/recordings.py list        # what it holds, each recording's score
. <dir>/env; tests/fixtures/recordings.py copy minutes-call --title "CLA-004 write now"   # prints the copy's folder
```

- `prepare` plays each known conversation through TestCall (both ends on this Mac, the user's line into BlackHole),
  records it with `echoctl capture` (stopped 3.5 s after the conversation, before TestCall hangs up: after it the mic
  falls back to the room, #140), transcribes it once with the build's model and keeps the call folders with what the
  commands printed (`capture.out`, `capture.err`, `capture.exit`, `during.ls`, `after.ls`, `transcribe.out`,
  `score.json`, the tracks) in `target/test-recordings/<key>/<name>/` (git-ignored), `<key>` the last commit that
  changed the app (`crates/`, `Cargo.*`; `-dirty` with uncommitted changes there): a commit that only changes tests
  keeps it. Recordings: `acme-sync`, `very-short`, `long-silence`, `two-remotes` (transcribed with NVIDIA's NeMo
  runtime: speakers), `mixed-languages-pinned` (`capture --languages uk`, workspace `en,uk,ru`), `mixed-languages` (the
  same tracks, no pin), `minutes-call` (213 s, built from the clips: the minutes cases need a long call, not a
  capture). Seven transcriptions in all.
- `copy <name> [--title T]` puts a copy into `$ECHOCTL_HOME/calls` with a fresh stamp, ending now (so automatic
  minutes and Today treat it as a call that just ended); `--keep-time` keeps the recorded times; `--untranscribed`
  copies `meta.json` only, and `finish <folder> <name>` then adds the transcript as a transcription would.
- **Which cases still record:** only those whose subject is capturing — the watcher's arm, start, stop, ask mode,
  drop and rejoin, a crash, Ctrl-C, only the call app's audio, notes while recording, a call appearing while it
  records. **Which still transcribe:** the transcription server and the fallback (one short clip per engine), the
  repair of a crashed recording, and what the app transcribes by itself after a watcher recording (no case scores it).
  Words, speakers and languages are scored once, in S-14, on the prepared transcripts.
- **The test clock.** Data built with `--clock fast` (`make_data.py`, `serve.sh up … --clock fast`) has
  `ECHOCTL_TEST_AUTO_MINUTES_DELAY_S=90` in `<dir>/env`: the debug build writes automatic minutes 90 s after the call
  or the last note instead of 15 minutes (`notes::auto_delay`; a release build never reads it). Every process of the
  run (the app, `window --serve`, the CLI) sources the env, so they agree on the time. Cases still check the rule —
  not before the delay, at it, moved by a later note — in seconds.
- **Waits are for conditions.** `tests/fixtures/wait.py <seconds> <condition>` (`app`, `armed`, `recording [suffix]`,
  `not-recording`, `in-call`, `event <name> [n]`, `file <glob>`, `sh <command>` …) checks every second and fails the
  step with the last state on its timeout. A fixed `sleep` stays only where it is the call's timeline (arm 16 s in,
  stop 20 s in) or a check that something does **not** happen.

## Recording with the test call

Recording cases (`tests/cases/recording/`, some of `privacy/`, `transcription/`, `window/`, `cli/`) run with TestCall
instead of Teams: a debug build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall` treats TestCall as the call app
(`crates/echoctl/src/apps.rs`, `test_call_app`). The user's lines are played by ffmpeg into BlackHole 2ch, which
TestCall holds as its microphone (the mic track); the other side's lines are what TestCall plays (the remote track, the
tap on TestCall). The known conversations (`tests/fixtures/audio/README.md`: `acme-sync` 36 s, `very-short` 9 s,
`long-silence` 36 s, `two-remotes` 34 s, `mixed-languages` 31 s) are built with `make_audio.py build` and scored with
`make_audio.py score`.

- **Needs on the Mac:** TestCall built (`tests/tools/testcall/build.sh`), BlackHole 2ch, ffmpeg, the speech model linked
  into the run's config (see `transcription/TC-001`). Both ends of the call run on this Mac (`serve` and `dial` on `127.0.0.1`). Port 7792; TestCall serves one call and exits when the connection drops, so it cannot
  rejoin and cannot switch devices.
- **Prelude** (each Bash call is a fresh shell): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- **A call, recorded by `echoctl capture`** (no app running): `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 42 --title "<title>"; wait`.
  The last line `capture` prints is the call folder. Then `cp "$CONV/expected.json" <call folder>/`,
  `target/debug/echoctl transcribe <call folder> [--keep-audio]`, `make_audio.py score <call folder>`.
- **The watcher** (arm, record when the call starts, stop, ask mode) runs only inside `echoctl app`. Cases that need it
  start a second app instance on the run's own data folder and config: `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app`
  (never `app start`, which may open the installed app), and stop it with `echoctl app quit`. The single-instance lock
  is `<config dir>/app.lock` (`app.rs`, `try_lock`), so the user's own Echo does not refuse it; if "Echo is already
  running" appears in `$D/app.out`, the case is BLOCKED. Such cases are tagged `second-instance`, set
  `config --unscheduled manual --scheduled manual` (no native "Record?" dialog) and `ai live off` (no live notes; the
  app's other Claude Code jobs are kept off a real account by the data's `--ai fake|local`, below).
- The first seconds of a watcher-recorded call can be clipped (the watcher needs a moment to see the call): watcher
  cases assert on words from the middle and end of a call, not on a whole-call score.
- **The app runs Claude Code by itself** (preps for the day's meetings, meeting types, who does a task) about a minute
  after it starts. On plain `small` data that goes to the built-in profile — the user's own Claude account. So every
  case that starts the app builds its data with `make_data.py <dir> --ai fake` (the fake `claude` refuses every run;
  the default for the recording scenarios) or `--ai local` (only S-19 on small data: only **minutes** runs reach the
  local model, the app's preps, meeting types and task modes are refused at once). Both turn the app's own thread
  sync off (`project_sync` false): two model runs per thread that queued in front of the minutes. And they set
  `--scheduled manual` too: `small`'s agenda has a meeting about
  an hour after the build, and "ask" (the default) opens a real "Record?" dialog a minute before it.
- **Events accumulate** in `.echo/events.jsonl` across the stages of a scenario: a case notes the count before it
  (`cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"`) and counts only its own
  (`tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep …`). The same for `.wav`
  files: earlier stages keep audio on purpose (`--keep-audio`), so a "no audio" check uses `find … -newer <a file of
  this case>`.
- **After the hang-up the mic reopens on the Mac's microphone** for the seconds left of a `capture --seconds N` or the
  watcher's 10 s grace ("Teams released its mic; using the default input"): whatever is said in the room then lands in
  the transcript as Me. Keep `--seconds` close to the call, and keep the room quiet; a stray line after the call's end
  is that defect, not the case's.
- **`capture` exits 134** after it has saved the call (a ggml-metal assertion at process exit, 2026-11-07): the folder
  and tracks are fine; keep its stdout (the folder) apart from its stderr (the backtrace).
- **Echo's "Record?" dialog** (ask mode, `app::ask`) is an `osascript` child of the run's app: find it with
  `pgrep -P "$(cat "$D/app.pid")" osascript` and click it with `osascript -e 'tell application "System Events" to tell
  (first process whose unix id is <its pid>) to click button "Record" of window 1'` — always by that pid.
- **No tray, no native window.** With the user's own Echo running, System Events can't keep the two `echoctl`
  menu-bar items apart (asked for the dev app's menu by its pid, it returned the installed Echo's), so a click could
  reach the user's Echo: cases run the app with `--no-tray` and use `echoctl` and the served window instead.
- **One TestCall call at a time** on a Mac (port 7792, BlackHole). Every running dev app sees every TestCall call (as
  `call_started`, unarmed: nothing recorded): runs that share the Mac never arm at the same time.
- **Speakers** (`recording/TC-018`) need NVIDIA's NeMo-Speech runtime, which a debug build doesn't carry:
  `TMPDIR="$D" scripts/fetch-nemo-speech.sh "$ECHOCTL_CONFIG_DIR/nemo-speech"` once per run.
- **Minutes on a call** need at least 3 minutes and 1,500 characters (`calls::gate`): a copy of the prepared
  `minutes-call` (213 s). Automatic minutes come 15 minutes after the call (`notes::AUTO_DELAY`); on `--clock fast`
  data the debug build makes it 90 s ("Recorded calls" above).

## Clean-install runs

S-15 and S-16 test the release as a new user meets it, on a Mac where Echo has never been: no test data, no
`serve.sh`. Rules 1–3 don't apply to them; everything else does.

- **Before:** `tests/tools/clean-mac.sh` (a dry run) must find nothing of Echo's. If it finds something, the run stops
  and asks the user to reset: `tests/tools/clean-mac.sh --yes --app`, then the `tccutil reset` lines it prints (the
  user runs those). `RUN="$(mktemp -d)/S-15"` (or S-16) holds the DMG and the run's notes.
- **The installed app's CLI:** `E=/Applications/Echo.app/Contents/MacOS/echoctl` (`$E app`, `$E status`,
  `$E model list`, `$E doctor --json`, `$E claude`) until cli/TC-003 puts `echoctl` on the PATH.
- **The window:** `ECHOCTL_SERVE_IPC=1 nohup $E window --serve 48150 > "$RUN/serve.log" 2>&1 & echo $! > "$RUN/pid"`,
  then Playwright on `http://127.0.0.1:48150/`: the real install's window, whose buttons reach the running app.
  Stop it with `kill "$(cat "$RUN/pid")"` (only that pid). It shows the real Settings, but its buttons that start an
  app action (Connect, Install…, a model's Download / Use / Remove, Ask for permissions, Re-check) do nothing there:
  only Echo's own window runs them (`settings_action` → `tray.rs`), so **the user clicks them in Echo's own window**
  when the run asks, and the run checks the result with `$E`. Its Health permissions are checked in the server's own
  process (they show "?"): read them with `$E doctor --json`, which asks the running app.
- **Install by dragging in Finder** (a user step): an app copied in Terminal keeps the quarantine flag without the
  "moved by the user" mark, and macOS runs it translocated (`/private/var/folders/…/AppTranslocation/…`). Check
  after the first start: `ps -o command= -p $($E app | python3 -c 'import json,sys;print(json.load(sys.stdin)["running"])')`
  and `~/Library/Application Support/echo/app-path` point into /Applications/Echo.app.
- **Notifications:** macOS shows no banner for the app in front; a notification Echo sends while its window is in
  front is only in Notification Center. Bring another app to the front before triggering one.
- **Echo's first-run questions** are AppleScript dialogs (`display dialog`, `choose folder`; they give up after
  120 s). The run clicks them through System Events (`osascript -e 'tell application "System Events" to …'`) if its
  terminal is allowed to (Accessibility, the user's choice); otherwise it names the button and the user clicks it.
- **User steps:** before each, the run says in its session exactly what will appear and what to do, and waits. The run
  never clicks a macOS prompt, types a password or changes System Settings; a check that needs a look at a banner or
  an icon is confirmed by the user (or by a screenshot, if the user gave the terminal Screen Recording).
- **Processes:** the run signals only the server it started (its saved pid), never anything matched by a pattern.
- **After S-15** Echo stays installed. S-16 starts from a reset, and only when the user says so.

## Out of Scope / Manual Setup Required

Cases that need these are `run: person` or only check the button and its dialog, never the result (except the
clean-install runs, below, on a Mac where Echo has never been):

- Microsoft 365: sign-in, calendar sync, mail, Teams, meeting chat (Settings → Calendar & chat "Sign in…", "Sends to you").
- Claude Code runs: Write minutes / Write minutes again, Prepare, Send to Claude, Yolo, Ask Claude about this call,
  "Sync now" (What moved, thread work), "Claude" panel in the footer, "Claude can" work, "Claude about this thread".
- The tray and the native window while the user's own Echo runs (see "Recording with the test call": No tray), "Memo",
  Teams itself and permissions prompts ("Record next call" and recording a call run with the test call): Health's
  "Ask for permissions".
- Plugin marketplaces over the network, installs, updates, "Check for updates", speech model downloads, "Read again".
- Anything that reads the real machine (Settings → General "Installed ✓", Health): see Fragile areas.

## Writing a case (Echo additions to the format)

Cases live in `tests/cases/<suite>/TC-NNN_<slug>.md` in the manual-qa format, with two more frontmatter fields:

- `run: agent | person` — who can run it (rule 5).
- `platform: any | macos | windows` — `any` for the window and the CLI in serve mode.
- `user_steps: ['…', …]` — only for a `run: agent` case that needs the user for a moment (rule 5): what the user
  does, so the lead can say so before the run.
- `requirements: [F-AREA-NN, …]` — the features of the functional map (`tests/feature-map/areas/*.json`) the case
  checks; a case that checks a behaviour the map doesn't list adds the feature to its area's file first.

Steps use `{{base_url}}` for the window and `echoctl …` (after `. <dir>/env`) for the CLI; test data comes from the
table above — a case that needs more adds it in its Preconditions with the `echoctl` commands that make it.

## Scenarios

A **case** is one behaviour (the unit of coverage, `tests/cases/`). A **scenario** is a user's journey that chains
cases in one run (`tests/scenarios/S-NN_<slug>.md`): its frontmatter lists the case files in order (`cases:`), and its
**Stages** table gives each stage's case, the stages it depends on, and any glue step before it. Agent runs execute
scenarios: far fewer runners than cases, and the cases meet each other the way a user meets them.

**Running one (the runner):**
1. Load `{{base_url}}/` first (the cookie), in the browser already open — don't open a new one, don't close it.
2. For each stage in order: do its "Before it" glue, read the stage's case file, and run its steps exactly as a single
   case (each step's expected result checked by snapshot or command output; the case's teardown at its end).
3. A failed step fails **that stage**: record actual vs expected, a screenshot, the console; then go on with the next
   stage, unless it lists the failed stage under "Depends on" — then it is BLOCKED (with the reason).
4. **Time every stage.** Before a stage's first step run `python3 -c 'import time;print(int(time.time()*1000))'` (milliseconds; macOS's `date` has no `%N`) and again after its teardown; the difference is the stage's `duration_ms`, waits and glue included. Never estimate it.
5. End with one JSON block for the scenario: `{"scenario_id", "title", "result" (PASS only if every stage passed;
   FAIL if any failed; BLOCKED if none could run), "stages": [{"stage", "case" (area/TC-NNN), "title", "priority",
   "result", "steps_total", "steps_completed", "failure_step", "failure_reason", "screenshot", "duration_ms",
   "started_at"}], "console_errors", "duration_ms", "duration_seconds", "notes"}` (the scenario's `duration_ms` from
   its first stage's start to its last stage's end; `started_at` ISO local time). Screenshots: `tests/reports/screenshots/<S-NN>-<stage>-<area>-<TC-NNN>.png`.

**Leading a run (the lead):** for recording scenarios (S-14, S-17, S-18, S-19, S-21), run `tests/fixtures/recordings.py prepare` once first (it says the cache is there when it is) and give its time to the reporter as a stage of its own ("prepare"). Run the scenarios, then the standalone cases (those in no scenario), each on fresh data as a one-stage scenario. Per scenario, `tests/fixtures/serve.sh up <new dir> <port>`, dispatch one runner with
the scenario path, base_url and env file, then `serve.sh down`. Scenarios with `data: standard` each get fresh data
(they may change it). The report counts stages (cases) as well as scenarios, and gives each stage's and each scenario's time (`duration_ms`) and the five slowest stages. `run: person` cases are listed as
skipped (person).

**Writing one:** order the stages so that a stage that changes data comes after the stages that read what it
changes, and give a stage "Depends on" only when it cannot run without the earlier one. A case may be a stage of
one or more scenarios, or stand alone: put it into a scenario when it belongs to a user's journey (that is cheaper to
run and closer to how the app is used), and leave it alone when it doesn't fit one.
