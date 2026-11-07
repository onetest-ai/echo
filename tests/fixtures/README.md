# Test data for functional tests

`make_data.py` builds a made-up Echo data folder; `serve.sh` builds one and serves the window on it. Everything in
it is invented (acme.example, globex.example, umbrella.example, initech.example and other `.example` domains): never
real calls, people or companies, and never `~/echo`. Times are relative to now; the rest is fixed, so two builds
differ only in their times. Standard library Python, and `target/debug/echoctl` (`cargo build` first; `ECHOCTL_BIN`
points elsewhere).

```bash
tests/fixtures/make_data.py <empty dir> [--set small|standard|hostile]     # default small
tests/fixtures/make-data.sh <empty dir>                                    # the same as --set small
tests/fixtures/serve.sh up <empty dir> <port> [--set small|standard|hostile]
tests/fixtures/serve.sh down <dir>
```

Build between 06:00 and 22:00: today's calls lie hours before now and its meetings hours after, so near midnight
the generator refuses (it would pin several calls to one stamp). For checks that don't need Today to match the real
clock, `--now 2026-11-06T12:00` builds as of that time (`serve.sh up <dir> <port> --set standard --now …`).

`<dir>/home` is the data folder (`ECHOCTL_HOME`), `<dir>/config` the settings (`ECHOCTL_CONFIG_DIR`), `<dir>/env`
sets them for a shell (`. <dir>/env`), and `<dir>/manifest.json` says what the set holds.

## The sets

| Set | What it is for | Size |
|---|---|---|
| `small` | The data the existing cases and scenarios are written against: Sam Rivera at Initech, clients Acme and Globex, 3 calls, 2 threads, 7 work items, a 5-meeting agenda (`tests/app_profile.md`, "What the test data holds") | 3 calls · 2 threads · 7 items · 5 people · 5 meetings |
| `standard` | Every value of every state axis of `tests/feature-map/axes/` that test data can hold (the rest are skipped, with the reason, below), and the axis groups of `combinations.json` covered pairwise | 96 calls (56 main ones, 40 all-hands of last spring so the list pages past 60) · 10 threads · ~133 items · 19 people · 37 meetings |
| `hostile` | `standard`, plus others' text in every place of `hostile.json`: HTML and script (`<img onerror>`, `<script>`, `javascript:` links), a 10 000-character word, right-to-left and combining characters, emoji and Cyrillic, Markdown that looks like links or headings. Each piece carries a marker `HOSTILE-NN` | standard + 5 calls, 1 thread, 3 items, 2 meetings; ~120 marked pieces |

The pairwise groups need more items than a person would keep: 7 states × 5 dues alone need 35 tasks, 6 responses
× 3 timings 18 meetings today. The extra items are spread over five open threads; the extra meetings make today
a busy day.

### `standard` in short

- **People**: Sam Rivera (sam@initech.example) and colleagues Alex Chen, Priya Nair, Tom Becker, Lena Vogel; Acme
  (Dana Lee, Omar Haddad, Rita Gomez, Ivan Petrov, Ola Svensson, Yuki Tanaka, Jo Weber), Globex (Kim Park, Noor
  Aziz, Ben Okafor at globex-labs.example), Umbrella (Zoe Martin, Raj Mehta), partner Vandelay (Kai Larsen),
  analysts Northwind (Mia Novak).
- **Clients and organisations**: Initech (yours), Acme (aliases, About, logo), Globex (two domains), Umbrella,
  Tyrell (closed threads only), Duff (no threads); organisations Vandelay, Northwind Analysts, Stark Advisory (no
  domain); Soylent removed after its thread was made.
- **Threads, one per stage**: acme-rollout (active), acme-renewal (negotiation), globex-pilot (kickoff),
  umbrella-proposal (proposal), umbrella-lead (lead, no calls), initech-hiring (wrap_up, the user's company),
  internal-tooling (on_hold, no client), legacy-migration (done, client removed), tyrell-bid (lost five days ago),
  acme-hackathon (cancelled).
- **Work**: acme-rollout's items come from its calls through the ledger's own path (`ledger sync`, `apply`,
  `verify-apply`): created, verified, weak, wrong, to check, kept yours, a pending merge and move. acme-renewal's
  quote is the same item as one in acme-rollout, whose "done" is heard there. The user's own items cover the node
  axes pairwise across five threads; umbrella-proposal's are quiet (its last call is five days old).
- **Calls**: today, yesterday, this week, older, last year, undated, and one in the trash; every status but
  recording; minutes runs done, failed, skipped, waiting for a sign-in; automatic minutes now, at a time (moved by
  the user's notes), held, gated, before the switch; memos, an import from a Teams transcript; chat, frames, the
  user's notes of every kind (and an unreadable one), kept live notes, named, numbered and multi-language speakers.
- **Today's agenda**: every response and timing, recorded and imported meetings, Teams transcripts known or not,
  scheduled meetings, preps ready (with every section, with none, with more than ten people) and in error.
- **Inbox**: sign-in (blocking), failed minutes, people suggestions (more than five), work to review, Found for
  you (an existing client, a new client from a domain, a known organisation, a series), senders to trust and
  unverified, a Teams transcript to import, a stale Claude Code plugin, a plugin update, a plugin's own item, and
  two decided this week.
- **Settings**: several AI profiles (signed in, signed out, another account, a local model), plugins from a local
  marketplace (on, off, an update, an update needing a newer Echo, one whose marketplace is gone, one that can't
  be read, one needing a newer Echo) and a marketplace that can't be read; skills (customized, yours, off, with
  problems), templates (edited, yours), meeting types (edited, yours), vocabulary (always, learned in every scope).

### What the standard and hostile sets change around the app

They are hermetic. `<dir>/env` puts these in front of the app, and `serve.sh` sources it:

- `PATH` starts with `<dir>/bin`, which has a **fake `claude`**. It answers `--version` and `auth status` (signed in,
  signed out or another account, by the AI profile's folder), and refuses everything else. A click on Write
  minutes, Prepare or Send to Claude fails at once instead of reaching a real account.
- `<dir>/bin/open` catches mail drafts: a `mailto:` link (`echoctl compose`, a Yolo run's draft) is appended to
  `<dir>/mail-drafts.log` instead of opening the user's real mail app; anything else goes to the system's `open`.
- `ECHOCTL_PLUGIN_MARKET` makes the built-in marketplace a local folder, `<dir>/markets/built-in` (no network).
- `CLAUDE_CONFIG_DIR` is `<dir>/claude-config`. It holds an old Echo plugin record (the stale-plugin Inbox item).
- `ECHOCTL_NOTES_NO_SHELL=1` starts Claude Code directly, not through the login shell.

The `hostile` set also has **Vendor onboarding**, a plain call whose other side orders the model writing its minutes
to break out (run commands, write `PWNED` files, edit other calls, change settings, send the transcript), each order
marked `HOSTILE-INJ-N`; `manifest.json` `injection` lists what must not happen (`hostile/TC-018`).

### Real Claude Code runs on a local model (`--ai local`)

`--ai local` (standard and hostile) makes the AI profile "Qwen · this computer" (Ollama at `http://localhost:11434`,
model `qwen3.8:27b-mlx`; `qwen3.8:27b` on Windows) the default, so calls and threads without a profile of their own
run on it. The fake `claude` then passes a run to the real Claude Code when, and only when, the profile routes it to
a loopback address (`ANTHROPIC_BASE_URL` on localhost); every other run is refused as before, so no Claude account
is used. Needs Claude Code on `PATH` and Ollama running with that model. A faster model server on the network: `--ai-url
http://<host>:11434 --ai-model qwen3.8:27b` (the profile is then "Qwen · test server"; that exact address is let through
too, nothing else; the server must listen on the network and have the model). To make that machine the default for every
`--ai local` build here, put `{"url": "http://<host>:11434", "model": "qwen3.8:27b"}` in
`tests/fixtures/ai.local.json` (git-ignored): it is used when it answers, else Ollama on this computer. A minutes run takes about 5–7 minutes:
cases that need one say `ai: local` and run one model job each, no more.

Automatic minutes are on and several calls are due. **Serve these sets with `window --serve` only, never the Echo
app**: the app would start minutes runs (against the fake, so they fail).

## The manifest

`<dir>/manifest.json` (both sets; `small` has the first parts only):

| Key | What |
|---|---|
| `calls` | `[{folder, title, key, started_at, status, run, auto, kind, threads, type_by, in_threads}]`. Folder names carry the build's time, so find a call by `key` or title. `threads` is what the window shows (one, several, a suggestion, none), read back from `echoctl project of` |
| `threads`, `clients` | ids, names, clients, stages; organisations that aren't clients have `account: false` |
| `items` | `[{thread, id, kind, title, state, …axis values}]`. Ids are per thread (`T001` in more than one thread): pair `id` with `thread` |
| `people`, `agenda`, `preps`, `plugins` | who is placed where; today's meetings with their axis values; the prep files by event; the plugins and marketplaces |
| `axes` | **for every axis value, where it lives**, e.g. `"call.status=failed": "calls/<folder>"`, `"thread.stage=lost": "tyrell-bid"` |
| `skipped` | the axis values the set doesn't hold, and why |
| `combinations` | per group of `combinations.json`: rows, pairs needed and covered, and any missing |
| `hostile` | `[{marker, kind, place, where}]`: each hostile piece, the place of `hostile.json` it tests, and the file or command it went into |
| `tasks_piles`, `counts` | the Tasks piles as `echoctl project tasks` gives them; how many calls, threads, items, people, meetings |

## Combinations covered (pairwise, greedy all-pairs)

| Group | Pairs |
|---|---|
| node owner × delegated × wait × state × due × mode | 270/270 |
| node kind × type × state × text × wait | 188/188 |
| call status × run × auto | 37/37 |
| call kind × threads × type by | 39/39 |
| meeting timing × flags × recorded × transcript × scheduled | 48/48 |
| meeting response × timing × prep × recorded | 77/77 |
| thread rules × membership × since × stage (stage by family) | 66/66 |
| person in thread × levels × side | 20/20 |

Pairs only among the values the files can hold: for example, a minutes run implies "tried", and a person seen only
in calls has no levels. Not covered as a group:

- **One per machine, not per item.** `ai.signin` and `set.claude_code` in the call group, and the `app.record`,
  `live.*` groups.
- **Values that come and go.** The plugin group's busy and marketplace-read states; the Inbox group's `inbox.count`
  per kind (both ≤ 5 and > 5 are there).

## Skipped (`standard` and `hostile`)

- **Needs a live process or service**:
  - `call.status=recording`, `app.record`, `live.state`, `live.content` while a call runs: need the app recording.
  - `call.run=queued/running`, Inbox minutes waiting: a queued run needs its owner alive; a dead one reads as failed.
  - `prep.state=writing`, `meeting.prep=writing`, `meeting.transcript` import busy/error, `inbox.busy`: only while a
    job runs.
  - `ai.signin=signing in`: only while a browser sign-in runs.
- **Needs Microsoft 365**: signed in; Teams and mail delivery; calendar events of other days and their flags
  (cancelled, declined, no notetakers, all-day); a meeting's chat rule.
- **Needs the network**: GitHub and git marketplaces; a transcription server; a model on another host; the update
  check (Inbox update).
- **The real machine**: speech models, autostart, the installed CLI, macOS permissions (Health and Inbox), the
  Claude Code checks (here, the fake).
- **One per data folder**: config.json and ai.json unreadable, auto minutes off, one AI profile, live notes off.
  Making one unreadable would break every view; a case corrupts it on purpose.
- **Only the user's act in the window**: merged or split people, a notes conflict kept, skill fit, the theme (kept
  in the browser).
- **Kept readable on purpose**: a call pinned to a missing or unreadable profile, `projects_error`.
- **Areas being reworked**: Today's minutes states (the files are there: `minutes-seen.json`), Knowledge (Recall
  off; `inbox.kind=brain` is compiled off).
- **No such state**: `call.type_by=none` (every call gets at least a guess); `meeting.action` (derived in the
  window).

`<dir>/manifest.json` → `skipped` lists each one with its reason.

## Changing it

- Read the Rust code that parses a file before writing it: `crates/echoctl/src/` (`calls.rs`, `notes.rs`,
  `window.rs`, `ledger.rs`, `projects.rs`, `people.rs`, `prep.rs`, `attention.rs`, `plugins.rs`, …).
- Use `echoctl` wherever it can make the thing. Ledger quotes must be verbatim from the call's transcript: the
  ledger refuses anything else.
- Check a build against the window's API: serve it, then load `/api/calls`, `/api/call/<folder>`,
  `/api/project/<id>/work`, `/api/tasks`, `/api/attention`, `/api/calendar`, `/api/plugins`, … and compare them with
  the manifest.
- `small` must stay byte-for-byte what the cases expect (times and ledger timestamps aside).
