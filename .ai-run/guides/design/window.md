# The Echo window and tray — design

The tray / menu-bar icon, the Dock, notifications, `echo://` links, and the one Echo window: a workbench
(webview) with Calls, Calendar, Settings, the ⌘K palette, the Inbox, the Claude terminal panel and My notes.

Related: rules in `.ai-run/guides/window/window-ui.md` (escaping, IPC actions, compiled-in assets, testable
blocks) and `.ai-run/guides/security/untrusted-text.md` (safe Markdown, path checks). Workbench areas and plugin
slots: `docs/plugins-design.md` ("a workbench, like VS Code"). Live notes in the window: `docs/live-notes-design.md`.
Not here: AI profiles, sign-in and minutes runs (`.ai-run/guides/design/ai-and-minutes.md`); the Recall/Brain views (`.ai-run/guides/design/recall.md`);
the Threads, People, Work and Tasks views (`.ai-run/guides/design/threads.md`).

## Modules

| File | Role |
|---|---|
| `src/tray.rs` | tao event loop, tray menu, Dock policy, window commands, the leave (close/quit) handshake |
| `src/mark.rs` | the octopus tray mark drawn from signed-distance shapes (`assets/octopus.svg` is the same shape) |
| `src/app.rs` | native dialogs (`yes_no`, `ask_checked`), notifications and their click (`Click`), `quit.asking` / `quit.cancelled` |
| `src/window.rs` | wry webview, the `echo://` protocol (`reply`, `respond`), `/api/*`, IPC `handle`, settings snapshot/set, layout file, dev modes |
| `src/links.rs` | `echo://` link parsing, routing (`target`) and text for Claude (`read`) |
| `src/terminal.rs` | PTY sessions for the Claude panel (portable-pty), output ring, session ids |
| `src/view.rs` | what the window shows → `<echo_view>` for Claude sessions (`view-hook`), `<echo_howto>` (`howto_block`) |
| `src/attention.rs` | Needs you (Inbox): collects pending decisions, Done list |
| `src/calendar.rs`, `src/schedule.rs` | Calendar data; "Record when it starts" list |
| `src/series.rs` | a meeting series' rule: record every meeting or not, its language |
| `src/prep.rs` | a meeting's prep: gathered facts + "In short"; Coming up today; the scheduler |
| `src/teams_transcript.rs` | import an unrecorded meeting from its Teams transcript; background check / scan |
| `src/minutes.rs` | `render_for_window`, `with_theme`, `EMBED_JS` (the minutes frame) |
| `ui/index.html`, `ui/app.js`, `ui/app.css` | the page (vanilla JS, compiled in); `ui/vendor/` xterm.js, sigma, graphology |
| `ui/*.test.mjs` | node tests over no-DOM blocks: workbench, palette, my-notes, notes-sync, calendar-dates |

## How it works

### Tray, icon, Dock, notifications
- Tray icon (`tray` feature, default; `app run --no-tray` disables): idle = template mark, armed = green,
  recording = orange.
- App icon: `echoctl dev-icon dist/echo.icon --from …` writes an Icon Composer document that `actool` (Xcode 26)
  compiles into `Assets.car` (`CFBundleIconName`) + `echo.icns`; without that actool, a rounded-square
  `.iconset`. Dev builds (no bundle) set the round logo as the Dock icon at runtime.
- macOS Dock: the app sets `ActivationPolicy::Regular` at start, so it has a Dock icon while it runs; Echo.app
  stays `LSUIElement`, so the CLI (same binary) never shows one. A Dock click with no window open opens it
  (`Event::Reopen`). `config.hide_dock_icon` (off by default): the icon shows only while the window is open
  (`tray::apply_dock`).
- Notifications (macOS) go through UserNotifications as Echo.app; a click opens the window, except "Update
  available", which asks to install (`app::Click`). Dev builds fall back to osascript (shown as Script Editor).
- Menu: status, Record, Stop, recent calls, Settings, Speech models, Claude Code, AI for calls, Health, Check for
  updates…, Quit. Clicks call the same `arm` / `stop` as the CLI, so every client sees the same events.
- The menu is rebuilt when its content changes (checked every 5 s), but on macOS never while it is open
  (`tray::menu_open`); the rebuild waits for the first tick after it closes.
- macOS needs the event loop on the main thread, so the watcher's `run_loop` runs on a worker thread.

### `echo://` links (`links.rs`)
| Link | Opens |
|---|---|
| `echo://call/<folder>[?t=<s>]` | with `t`: Transcript at that moment; else Minutes |
| `echo://thread/<id>` · `echo://client/<id>` | the thread's Brief · the client's Overview |
| `echo://person/<People key>` | their thread's People tab with their panel |
| `echo://mail/<source id>` | its Outlook / Teams link |
| `echo://task/<thread>/<item id>` | the thread's Work tab with that item's details (Details → Copy link) |

- `Link::parse` refuses `..` and extra slashes.
- Registered with the OS: macOS `CFBundleURLTypes` → tao's `Event::Opened`; Windows `HKCU\Software\Classes\echo`
  → `echoctl app open --link "%1"`, rewritten at each start beside `app-path`.
- Routing: `window.echo.openLink` → `/api/link` (`links::target`). The page and the minutes frame
  (`{echo: 'link'}`) take clicks; `markdown_html` keeps such links (`data-echo-link`).
- `echoctl read <link>` gives Claude the text behind a link, read-only (a call's minutes + turns ±90 s around the
  moment, a person's card, a work item with where it was said, …).

### The webview
- wry (WKWebView / WebView2) on the tray's tao loop, not full Tauri: Tauri would take over the loop, tray,
  packaging and updater.
- Content only through the `echo://` protocol (`http://echo.localhost` on Windows): compiled-in page, JSON under
  `/api/…`, `/file/<call>/frames/<img>` after a path check, strict CSP.
- `/api/calls?limit=&before=<name>&q=` pages the list (`calls_page`: 60 a page, ≤ 500, newest first, `{calls,
  next, total}`); the list loads the next page near its end; a refresh re-reads the first page and puts it on top;
  `patch` keeps redrawn regions' scroll positions.
- `render()` replaces a region (sidebar, list, call header, pane) only when its HTML changed, so the minutes frame
  never reloads on a refresh. A focused notes editor is updated in place, caret kept (`syncEditor`).
- The page polls `/api/call/<name>/rev` every 2 s (all the call's files, live notes, run and automatic-minutes
  state) and refetches the call only when it changed.
- IPC: messages only from the window's own page (`window::from_page`: on macOS wry gives the frame's URL, since
  WebKit exposes the handler to frames; Windows listens to the top level only). `window::handle` → `{refresh,
  reply}`; actions call the same functions as the CLI.
- macOS: transparent title bar with content under it (`with_fullsize_content_view`); the strip posts
  `window_drag` / `window_zoom` (`window::window_cmd`). Windows keeps its native title bar.

### Workbench layout
- Fixed areas: a 40 px top strip with the **Record pill** at its centre (idle Record ▾ with language + profile;
  armed + Cancel; meeting soon + Record it; recording: clock, title, Stop that asks first), the activity bar, the
  primary view, right and bottom panels, a status bar.
- The activity bar is folded to icons by default (48 px; each icon's name and keys in an instant hint beside it) or
  open with names and counts (200 px): ⌘\\ / Ctrl+\\, the switch in the title strip after the traffic lights
  (`barButton`), ⌘K; `Layout`'s `bar`, saved with the layout. Below 900 px it opens over the page and closes on a click
  beside it or Esc (never saved). Its switch sits in the strip as on the design it came from.
- Registries in a no-DOM block of `app.js` (`// ── workbench` … `// end of the workbench block`): `ACTIVITIES`
  (Calls `list-detail`, Calendar `single`, Settings `single` at the foot; ⌘/Ctrl+1…9, ⌘/Ctrl+,), `PANEL_VIEWS`,
  `CALL_TABS`, `EVENT_ACTIONS`. Echo's own views are entries like a plugin's.
- Call tabs: Minutes / Transcript / Chat / Screens / Actions. One Transcript tab: live (first, with a recording
  dot) while the call records, final afterwards, its live notes beside it either way (`#call/x/live`).
- `Layout` keeps panel placement and open panels in `<config dir>/window-layout.json` (`/api/layout`, IPC
  `save_layout`, ≤ 16 KB). The right panel starts closed at every window open (`Layout.started()`).
- Responsive: below 900 px wide the calls list folds behind "Calls ▾" and the right panel overlays the page; below
  600 px high the bottom panel folds.

### ⌘K palette (`Palette`, `ui/palette.test.mjs`)
- ⌘K (Windows: Ctrl+K or Ctrl+Shift+P; status bar "Actions"); not while a terminal has the keys.
- Actions on a target chip (the open call, the selected calendar event, or none; ⌫ on an empty box drops it) plus
  Go to (activities, Settings sections, calls by title, people or day).
- Unavailable actions stay listed, greyed, with the reason. Each runs as the click on its button elsewhere
  (`press`), so it can't behave differently.
- Ranking: every typed word in the label (anywhere) or a keyword (word start); word starts and earlier matches
  first; unavailable last; groups kept together.

### Needs you (Inbox) (`attention.rs`, `/api/attention`, panel id `needs`)
- Collected from where each decision lives, no model: sign-ins (urgent when the default profile or waiting minutes
  need it), failed minutes (14 days), senders, people suggestions, Found for you, To review, topics/numbers to
  accept, permissions, an update. Each item carries the IPC message its own place sends, or the hash to go to.
- Entry above Settings in the activity bar (count; warn colour when something blocks work); toggles the panel
  (⇧⌘I; Ctrl+Shift+U on Windows, where Ctrl+Shift+I is WebView2's DevTools).
- A clicked decision keeps its item "working" until it is gone: a background job's (`busy` by item key) while it
  runs — its end, a panic too (`OnEnd`), marks the cache stale (`attention::stale`) — any other up to 15 s. A
  failed job gives the buttons back with why (`failed`).
- An item gone moves to Done for 7 days (`<home>/.echo/attention.json`) — only when its source was read; an
  unreadable source keeps its items open. Collected at most every 20 s, one at a time (`?fresh=1` after an
  action). `echoctl attention` only reads (a terminal sees neither sign-in nor update checks).

### Settings (`#settings/<section>`)
- Sections: General (incl. Appearance), Recording, Transcription, AI & minutes, Calendar & chat, About me, Layout,
  Health (⚠ badge = missing required permissions); plugins add theirs.
- `/api/settings` = one snapshot (`window::settings_snapshot`). Plain values save at once with the value shown —
  never a toggle of a stale value — through IPC `settings_set` (config.json, or ai.json under its lock). Flows with
  a dialog, sign-in or download run the tray's own `Action` by name (`settings_action` → `tray::Ui::page_action`).
- Appearance (System / Light / Dark) is per computer in the webview (`echo.theme`, `:root[data-theme]`); graphs and
  terminals redraw; the minutes frame is served with its dark rules on or off (`minutes::with_theme`, `?theme=`).
- About me's `me.md` is shown through `minutes::markdown_html` and edited in place (`settings_set me_profile`).
- Vocabulary is a panel view (book icon beside Calls' heading; `#settings/vocabulary` opens it).

### Claude in the window (`terminal.rs`)
- One panel view, "Claude": the Echo-folder session (two sessions — this call / Echo folder — confused people).
  "Ask Claude about this call" opens it; the open call reaches it via `<echo_view>`. Entry points: status bar,
  ⌘/Ctrl+`, the call's ⋯ menu. The guarded per-call session still exists in `terminal.rs` (`pty_open {kind:
  call}`) but not in the window.
- PTYs (macOS PTY, Windows ConPTY) are owned by the app: closing the window keeps them, Quit ends them
  (`terminal::close_all`). Output: a 2 MB ring per terminal numbered by byte; the page long-polls
  `/api/pty/<id>?from=<byte>` (≤ 25 s), so a reopened window gets the scrollback. Keys / size / close over IPC.
- Each xterm element is made once and moved into its panel slot after every render, so renders and panel moves
  keep it.
- Keys inside `.xterm` go to the terminal (⌘/Ctrl+Esc returns focus) except `termKey`'s: macOS ⌘C / ⌘A / ⌘V
  (paste through the app, IPC `pty_paste`; no text → Ctrl+V for Claude Code's image paste) because the window has
  no Edit menu; Ctrl/Shift+Enter send ESC CR (new line in Claude Code's prompt).
- At start the session gets `<echo_howto>` (`view::howto_block`): do Echo's things through this Echo's CLI, and
  mail through `deliver send mail` (Echo's Microsoft 365 sign-in, the user confirms) — not a Claude connector,
  which a session otherwise tried.
- Sessions run in the data folder (never a call's: Windows renames), start with `--session-id`, resume with
  `--resume` (`<home>/.echo/sessions.json`; an id refused within 4 s is forgotten). Profile, lease and launcher:
  see the AI doc (`ai::terminal_command`).
- ⌘K Restart (same id, plugins re-read) and New conversation (fresh id; old kept in
  `<home>/.echo/session-history.json`) (`claudeSessionActions`, `pty_open {session}` → `choose_session`). Both ask
  first while it runs; the page waits until the old processes are gone (`terminal::stopping`; a start of a key
  still stopping is refused); a session Echo stopped never forgets its conversation (`Term.stopped`).
- `/api/ptys` `stale`: the profile's `installed_plugins.json` (Echo's entries) or a plugin's session skills changed
  since start → **Restart to update** on the panel.

### What the window shows, in each prompt (`view.rs`)
1. The page reports its view (activity, open call + tab + threads, thread + tab, the work item open, calendar
   event, panels, selected text — also from the minutes frame, `{echo: 'sel'}`) over IPC `view_state` on change
   and every minute → `<config dir>/window-view.json`.
2. The session's `--settings` carry a `UserPromptSubmit` hook, `echoctl view-hook`, which adds an `<echo_view>`
   block as `additionalContext`: page text treated as data (no `<`/`>`, ≤ 400 chars each), nothing if older than
   15 min.
3. Sent only when it changed since that session's last block (hash per session id in
   `<config dir>/window-view-sent.json`, 50 sessions, 14 days); a `SessionStart` hook on `compact|clear` forgets
   the session so the next prompt gets it again.
4. Answers only with `ECHOCTL_TERMINAL=1` (set on Echo's terminal sessions) — never minutes/sync runs, the plugin,
   or the user's other sessions. Sessions of another call or profile get the call and selection dropped
   (`view::restrict`).

### My notes (panel view `notesPanel`; `NoteDoc`, `ui/my-notes.test.mjs`)
- A timeline of moments, one line of `my-notes.md` each, in time order: `- [12:05] …`, `? ` question, `! `
  decision, `⚑` flag, `- [ ] [12:22] …` to-do, `  > "…"` the quoted words.
- The box times a note from its first key while recording. Text typed (or edited in place) without Enter is added
  as Enter would before a close/quit (`prepareClose`), Write minutes, or another call opened; Esc drops it.
- Add with a quote: select transcript words → Comment, or right-click a transcript turn or a minutes line → Add
  note (the frame's `EMBED_JS` posts `{echo: 'note', …}`, accepted only from that frame).
- Shown as a dot per note on transcript turns, and as callouts in the Minutes frame under the line with the same
  time (else nearest within 3 min) — sent to the frame (`{echo: 'notes'}`), never written into the page or file.
- Edit as text (`NotesSync`, `ui/notes-sync.test.mjs`): saves 1 s after typing, on blur, before Write minutes; one
  acknowledged save in flight per call. IPC `save_my_notes {text, base}` → `calls::save_my_notes` under
  `.meta.lock`: base unchanged → text; appended since (tray) → text + tail; otherwise the other version is kept as
  `my-notes.<HHMMSS>.md`. An unchanged text is never written: its mtime is the minutes' clock.

### Minutes frame
- `/minutes/<call>?v=<revision>` (`minutes::render_for_window`, rendered fresh) in
  `<iframe sandbox="allow-scripts">` under the page's own CSP. `EMBED_JS` posts `{echo: 'at'|'open'|'link'|'note'
  |'sel'}`, accepted only from that frame's `contentWindow`.

### Close and quit handshake (`tray.rs` `Leaving`)
1. Closing the window, Quit (macOS's own too: tray.rs answers tao's `applicationShouldTerminate:` with "later"),
   `echoctl app quit`, installing an update and changing the data folder first ask the page to save.
2. Saving: `prepareClose(n, close)` → IPC `close_ready {ok, n}`; editors read-only until Echo is gone or thawed.
3. On failure or no answer in 5 s, ask the user (`app::ask_checked` tells "couldn't show" from "not answered"; a
   close's failure the page explains itself); then go. `act` runs each step.
- One leave at a time. A new one replaces the pending one or is dropped (`takes_over`: anything replaces a close, a
  quit replaces an update / folder change, `app quit` replaces a quit). While the user is asked, a newcomer waits
  (`queued`) and gets its own turn; once Echo is stopping all are dropped. So macOS's waiting quit is answered by
  the quit it rides on. The question's thread answers once, even on failure (drop guard).
- `app quit` takes this path with or without a tray: `quit.asking` (pid) while asked; `quit.cancelled` → exit 1.
- Closing the window keeps the app in the menu bar and the Dock (unless Hide from the Dock).

### Calendar
- Day (today) and a rolling Week (yesterday, today, 5 ahead) from `/api/calendar?start=&days=` (Graph
  `calendar_view`, cached 2 min, served by the async protocol handler so Microsoft 365 never blocks the UI; no
  sign-in → today's agenda). Events carry `recorded` (meeting id, or subject within the event's time) and
  `scheduled`; unmatched calls come back as `calls`.
- "Record when it starts" → `<home>/.echo/scheduled.json` (`schedule.rs`): the watcher arms it from 60 s before
  start until its end. Removed when its recording starts (Stop never re-arms), its arm is cancelled, its recording
  fails to start (one notification, no re-arm every second), or after the end.
- **A meeting series** (`series.rs`; a recurring event's card: "Every meeting of this series" — Record / Don't record
  and a language, nothing else, the user's call: no other settings there). `<home>/projects/series.json`, keyed
  `id:<seriesMasterId>` else `subject:<normalised>` (a rule of either key applies). Don't record is the default: a
  series' meeting gets no "Record?" offer unless its series says Record (`may_offer`); Record puts each occurrence in
  "Record when it starts" once after an agenda sync (`schedule_always`, `<home>/.echo/series-armed.json` — cancelling
  one is never undone); its language fills an arm where nobody chose one (`fill`, in `watch::arm`). A one-off meeting
  is asked about as before; a choice on the Record button wins. A thread's Profile → Meetings lists its series read
  only; × = not this thread's (`Project.not_series`). `echoctl series list [--thread] | set <key> <json>`.
- The event card is 400 px wide (`min(400px, 100vw − 24px)`).

### A prep before each meeting (`prep.rs`)
- For today's calendar meetings of an open thread, written without being asked (the app checks every 5 minutes —
  the morning's, one added later; ≥ 5 min before the start; `prep::start_scheduler`): `<home>/prep/<id>/prep.json`,
  `id` = 16 hex of sha256(event id). Nothing in a call folder: a call folder is still made only by a recording.
- Gathered without a model: Last time (the previous meeting of the subject with minutes — highlights, open actions,
  its link), Open with these people (the threads' open tasks owned by someone in the invite or the user, late first;
  risks with no response; questions not answered — each `echo://task`), New since then (the threads' mail and chat
  evidence after the last meeting: titles and senders, never the text), Who's coming (the invite less the user, with
  role / stance from People, calls together, new; the window shows ten).
- Then "In short", 3–5 sentences (≤ 90 words) to the user as "you", on the threads' AI profile
  (`livenotes::complete_text`) — only when every thread runs on the same profile (strict routing), else facts only.
- Calls → **Coming up today** (the list's first group, `/api/upcoming`, `#upcoming/<id>`): a meeting still to come is
  a call with its Prep tab (Transcript and Minutes once recorded; Prepare now / Write it again: IPC `project {op:
  prep_write, prep}`, one at a time). A recorded call finds its prep by its event (`prep::of_call`) → its first tab,
  Prep. Not in Calendar: the user, "it belongs in Calls".
- `echo://prep/<id>` (recorded → its call's Prep tab, else `#upcoming/<id>`), `echoctl read`, MCP `read_link` (only
  when all its threads are the reader's), `echoctl prep --upcoming | --show ID | --write ID`.
- Measured on a copy: a weekly with 31 people, 8 open items, 8 new mails — about 6 s on sonnet.

### Import a Teams transcript (`teams_transcript.rs`)
1. Calendar → an ended Teams event with no call → Import Teams transcript (IPC `calendar_import`, background;
   state on the event, outcome as a notification), ⌘K, or `echoctl calendar import <event id | subject> [--date]`.
2. Graph: join link → `/me/onlineMeetings?$filter=JoinWebUrl eq '…'` → its transcripts as `text/vtt`
   (`text/plain` is refused). A series shares one online meeting, so only transcripts started within the
   occurrence ±30 min count; several are joined in time order.
3. The call: `meta.json` `source: teams-transcript` (event times, widened by the transcripts; `imported_at`; AI
   profile pinned), `meeting.json` from the event (threads match as for a recording), segments + transcript from
   `<v Name>` cues ("Me" = About me's name, else the account's), the chat (`watch::fetch_call_chat`).
- Refused when a call of the event exists; says so when there's no transcript or Teams doesn't give it to this
  account (outside organiser). No audio, no live notes; the UI says "from Teams transcript". Automatic minutes count
  from `imported_at` (`notes::auto_due`), so an old meeting still gets them.
- The calendar marks events whose transcript is known (blue, dashed, "Transcript"; `teams_transcript::known`);
  unknown ones are checked in the background (`teams_transcript::check`, one at a time, ≤ 25 meetings, ~45 s for
  25). With each agenda sync `teams_transcript::scan` offers, once, the last 3 days' ended meetings of open threads
  with no call in Needs you (Import · Not this).

### Dev modes
- `echoctl window --selftest`: the real webview; fetches over `echo://`, reports over IPC; proves a sandboxed frame's
  IPC is dropped, the close handshake answers, a shell runs in a terminal.
- `echoctl window --serve <port>` (hidden): page + API live on 127.0.0.1 (`respond`); `ECHOCTL_SERVE_IPC=1` adds
  `POST /ipc` → `handle`, only with that run's SameSite=Strict cookie and its own Origin.
- `echoctl window --export <dir>`: the same responses as files (minutes as `minutes/<call>/index.html`).

## Decisions

| Decision | Why | Where |
|---|---|---|
| wry on our tao loop, not Tauri | Tauri would own the loop, tray, packaging and updater | `window.rs`, `tray.rs` |
| Dock icon while the app runs (macOS), Echo.app stays `LSUIElement` | the menu-bar icon can hide behind the notch and Echo looked gone; the CLI must never show one | `tray::apply_dock` |
| No menu rebuild while it is open (macOS) | AppKit tracks it in another run-loop mode; a replaced menu closes under the pointer | `tray::menu_open` |
| IPC only from the page itself | WebKit exposes the handler to frames; the minutes frame shows others' text | `window::from_page` |
| Minutes in a sandboxed iframe; its messages checked by `contentWindow` | rendered model output must not drive window actions | `minutes::EMBED_JS`, `app.js` |
| Regions replaced only when their HTML changed | a refresh would reload the minutes frame and lose scroll | `app.js` `render` / `patch` |
| Palette actions run as their button's click | an action can't diverge from the button | `app.js` `press` |
| Settings save the value shown, never a toggle | a stale page would flip the wrong way | `window::settings_set` |
| Settings flows run the tray's `Action` by name | one code path for menu and page | `tray::Ui::page_action` |
| An unchanged `my-notes.md` is never written | its mtime is the automatic minutes' clock | `calls::save_my_notes` |
| A conflicting notes save keeps the other version as a file | never lose the tray's or another editor's text | `calls::save_my_notes` |
| `<echo_view>` only on change, only for Echo's terminals | keeps prompts small; runs and other sessions must not see the page | `view.rs` (`ECHOCTL_TERMINAL`) |
| PTYs owned by the app, scrollback in a byte ring | closing the window keeps the session | `terminal.rs` |
| One leave (close/quit/update) at a time, save first | notes typed in the window must not be lost on quit | `tray.rs` `Leaving`, `takes_over` |
| Inbox Done only when the source was read | an unreadable source must not mark items decided | `attention.rs` |
| Teams transcripts: VTT only, occurrence ±30 min | `text/plain` is refused; a series shares one online meeting | `teams_transcript::import` |
| Calendar fetched in the async handler, cached 2 min | Microsoft 365 must never block the UI | `window.rs` `/api/calendar` |

## Files and state

| Path | What | Written by |
|---|---|---|
| `<config dir>/window-layout.json` | panel placement, open panels (per computer) | window (`save_layout`) |
| `<config dir>/window-view.json` | what the window shows now | window (`view_state`) |
| `<config dir>/window-view-sent.json` | last `<echo_view>` hash per session | `echoctl view-hook` |
| `<home>/.echo/sessions.json`, `session-history.json` | Claude session ids per profile; past conversations | `terminal.rs` |
| `<home>/.echo/attention.json` | Inbox Done list (7 days) and open items | `attention.rs` |
| `<home>/.echo/scheduled.json` | meetings to record when they start | `schedule.rs` |
| `<home>/projects/series.json` | meeting series' rules (record or not, language) | `series.rs` |
| `<home>/.echo/series-armed.json` | occurrences a series already put in "Record when it starts" | `series.rs` |
| `<home>/prep/<id>/prep.json` | a meeting's prep | `prep.rs` |
| `<home>/.echo/teams-transcripts.json` | known transcripts / offers (rechecked hourly until a day after, kept 30 days) | `teams_transcript.rs` |
| `<home>/.echo/quit.asking`, `quit.cancelled` | an `app quit` being asked / refused | `app.rs` |
| `<call>/my-notes.md`, `my-notes.<HHMMSS>.md` | the user's notes; a kept conflicting version | window, tray (`echoctl note`) |

## Known gaps
- Windows keeps its native title bar (no strip-drag yet).
- `/api/conversations` lists past conversations but no UI reads it.
