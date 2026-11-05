# Recall — design

Recall (the code and the CLI still say "brain") is one per-user index of what was said: calls, and the threads' Teams
channels, mail and folders, searchable by words and meaning, plus the dictionary. Every section links back to where
it was said. Nothing is copied into it, and Echo runs no model of its own for it.

Related: rules in `.ai-run/guides/data/recall-store.md` (schema changes, profile-scoped access) and
`.ai-run/guides/security/untrusted-text.md`. Plan, phases, the Brain-vs-Echo table and measurements are in
`docs/brain-design.md`. Thread sources, senders for a thread's own work, and Recall's People view (`people::everyone`)
are in `.ai-run/guides/design/threads.md`. Plugin slots `knowledge.tab` and `knowledge.settings` are in
`.ai-run/guides/design/plugins.md`.

## Modules

| File (`crates/echoctl/src/brain/`) | Role |
|---|---|
| `mod.rs` | `dir`, `EXTRAS`, settings (`brain.json`), `enabled`, `sync` (all lanes, then history, then term mentions), `start_scheduler`, `tell_recall` |
| `store.rs` | SQLite schema (`open`), `sync`, snapshots, `search_in` / `search_at`, `related`, `supersede`, `archived_of`, `kept` aggregates |
| `chunk.rs` | Brain's heading-aware chunker (`MAX_CHARS` 1200, no overlap) and chunk ids |
| `embed.rs` | Embedding endpoint client (Ollama `/api/embed` or OpenAI-compatible `/v1/embeddings`), `Settings` |
| `calls.rs` | A call as a document: minutes sections, packed transcript turns, chat |
| `sources.rs` | Teams channels and mail: fetch, rescope, read state, re-reads, the digest queue |
| `senders.rs` | Who may speak: colleagues, trusted and ignored senders per client, unverified mail |
| `digest.rs` | Quarantine reader: a message becomes facts with checked quotes |
| `files/` | Thread folders: `folder.rs` registry and sync, `office.rs` / `xml.rs` (pptx, docx, Teams/Otter .docx transcripts), `pdf.rs` (PDFium), `sheet.rs` (calamine), `text.rs` (md, txt, vtt, srt, html, `pack`), `render.rs` (LibreOffice to PDF), `vision.rs` (page reader), `mod.rs` (formats, twins, `route`, `exact_date`) |
| `access.rs` | `Access`: which profile's sources a reader may see |
| `mcp.rs` | `echoctl mcp` / `echoctl brain mcp`: stdio MCP server, read-only |
| `advert.rs` | What Recall holds, told to Claude (`<echo_recall>`, server instructions), plus usage counts |
| `views.rs` | Window views: `overview`, `also`, `mentions`, `doc`, `node` |
| `browse.rs` | Paged browsing for a large store (`/api/brain/browse/…`) |
| `taxonomy.rs` | The dictionary (terms), learned vocabulary, term mentions. Its topics part is switched off |
| `insights.rs` | Switched off. Its `quiet()` is still used live by the page reader and the digest reader |
| `numbers/` | Switched off: the numbers lane |

## How it works

### Switches
- `brain.json` `enabled` defaults to **off** on a fresh install and on an update (`embed::Settings::enabled`).
  When off, nothing is read, indexed or tagged: `brain::sync` refuses and the scheduler skips. The window shows no
  Recall (`/api/state` `brain`). Sessions get no Echo MCP server. Transcription hints fall back to glossaries. The
  store stays on disk untouched.
- `auto` (default on) gates the scheduler. `enabled` is set in Settings → Knowledge → Use Recall, with
  `echoctl brain set --enabled`, or with IPC `settings_set brain_enabled`.
- The `brain` cargo feature (default) brings bundled SQLite. It is C, so the Windows cross-check builds without it.
- `brain::EXTRAS` / app.js `RECALL_EXTRAS` = `false`. See "Switched off" below.

### Folder and name
`brain::dir()` = `<home>/recall`. The old `<home>/brain` is renamed to it on first use. In the window, Recall sits
inside the **Knowledge** activity (`#brain/<tab>`): plugins' tabs come first (`knowledge.tab`), then Recall's tabs:
Sources, Dictionary (`terms`) and People.

### Documents
| Kind | `source` id | Built from | Links |
|---|---|---|---|
| Call | `call/<stamp>` | minutes sections, then turns packed to about 1000 chars (`### [mm:ss] A, B`, lines `[mm:ss] Name (Org): text`, names via `people::display`, org from the invite's domain), then the chat | `echo://call/<name>?t=<s>` |
| Channel post | `channel/<key>/<id>` | one post with its replies, as digested facts, titled by its first line that isn't a bare link | Teams `webUrl` |
| Mail conversation | `mail/<conv>` | each message's new part as digested facts, one section each | Outlook `webLink` |
| Folder file | `file/<id>` | text, or the page reader's output, as `## pNN · title` per page | `echo://file/<id>?page=N` (IPC `open_brain_file`) |

Refs on every section are the call or source, its threads and its client. Refs are part of a document's hash, so
moving a call to another thread re-scopes its sections.

### Sync (`brain::sync`, `store::sync`)
1. Build call inputs. Then `sources::sync` (channels and mail) and `files::folder::sync` (folders).
2. Re-chunk a document only when its text or refs hash changed. Re-embed a chunk only when its text changed. A chunk
   that is gone takes its FTS row, vector, related edges and tags with it.
3. Take a snapshot first (`VACUUM INTO`, `SNAPSHOTS` = 3 kept). Write in one transaction. Embed outside it in
   batches (`embed::BATCH`). A failed embedding leaves the text indexed, and the next sync fills the gaps. A
   different model re-embeds everything.
4. `related` = Brain's: k 6, score ≥ `RELATED_MIN` 0.55, other documents only.
5. `store::supersede` applies thread history dates (below). `taxonomy::mentions_after_sync` finds term mentions.
- Scheduler (`start_scheduler`): checks every 5 min and syncs when `fingerprint()` changed (calls' files, threads,
  embedding model). A failure is retried after an hour. Sync now = IPC `brain_sync`.

### Search
- FTS5 `unicode61` covers any script (Brain's query builder dropped Cyrillic). Vectors are plain unit vectors in
  `chunks_vec`, searched in Rust (no sqlite-vec).
- Fusion: weighted RRF, `W_FTS` 0.4 / `W_VEC` 0.6, `RRF_K` 60, 30 hits from each. With no endpoint set, words only.
- History (`store::search_at`): latest (the default; superseded sections hidden), all (MCP `latest_only: false`), or
  `as_of` a day.

### Channels and mail (`sources.rs`)
- Configured on a thread's Profile → Sources (see threads.md). Read with the Microsoft 365 sign-in when calls sync
  and every half hour. First read covers `FIRST_DAYS` 90, at most `CAP` 200 per read.
- Mail: Graph `from:X OR to:X`, then checked by address (Graph's search is fuzzy, `participants:` fails). Calendar
  invites, mail to more than 30 people, and the user's own addresses at any domain are dropped. Channels drop system
  events.
- A document's threads = every open thread whose sources match it. `rescope` recomputes this on each sync without
  fetching anything. A document matching no thread is removed.
- Read state per source lives in `sources.json` (`since`, `found`, error shown on the chip). Bumping `READER` (now 4)
  drops all mail and channel documents and re-reads them.

### Who may speak (`senders.rs`)
A message is read only if it comes from:
- the user,
- a colleague: mail with Exchange `AuthAs: Internal` from the tenant's verified domains, or a Teams author whose
  People-API entry carries their own directory id (never a name match), or
- a sender the user trusted for **every** client of the document's threads (`senders.json`, per client).

More rules:
- "Ignored" anywhere wins over "trusted".
- A trusted outside sender's mail also needs Exchange's topmost `Authentication-Results` to show DMARC or compauth
  passing for the From domain (`graph::mail_auth`). SPF + DKIM alone can pass for an attacker's domain.
- Mail with no verdict at all is listed as `unverified` for Read anyway / Keep out. A failed check is never read.
- An author not looked up yet (`LOOKUPS_MAX` 40 per sync), or no organisation domains known, defers the document.
  Echo never guesses.
- Others wait (count, last subject), with nothing of theirs read, for Trust / Ignore / Forget (Sources tab, IPC
  `brain_sender`, `echoctl brain senders`).
- A decision queues that client's sources for re-reading (`reread.json`). `save_merged`: the user's lists on disk win.

### Quarantine reader (`digest.rs`)
- Each post-with-replies or conversation is read once by a no-tools model on its threads' profile
  (`livenotes::complete_text`). Output: a neutral title and facts per message, ≤ `MAX_FACTS` 12, each quote
  ≤ 25 words and checked word for word against that message (else dropped).
- Only those facts are indexed, never the messages' text.
- Cache: `digests/`, keyed by content, profile and `PROMPT_VERSION`.
- Limits: ≤ `DIGEST_MAX` 30 per sync, given up after `READER_TRIES` 3. Waits while a call records or is transcribed,
  and, on macOS, while on battery (`insights::quiet`). A source with documents still waiting reads no further.

### Thread folders (`files/`)
The registry, skip reasons, twins, readers, routing thresholds, the page reader's rules, the two representations and
dates are all as planned in `docs/brain-design.md` (Phase 5). Code-level facts not in that doc:
- `files` table in the store (`folder.rs`). Hashed only when size or mtime changed. A move keeps `file/<id>`.
  Bumping `READER_VERSION` re-reads everything. Statuses: `indexed`, `pending`, `skipped`, `twin`, `unsupported`,
  `error`. Limits: `MAX_FILES` 5000, `MAX_BYTES` 200 MB.
- PDFium is loaded at runtime from `ECHOCTL_PDFIUM_DIR`, then `Echo.app/Contents/Frameworks/pdfium`, then
  `<config dir>/pdfium`. `scripts/fetch-pdfium.sh` pins it by SHA-256.
- Decks and Word files with figures are routed on LibreOffice's PDF of them (`render.rs`, once per file version).
  Their text and tables still come from the XML. Without LibreOffice they are indexed from text and marked degraded.
- Page reader (`vision.rs`): Claude on the default profile's minutes model (`livenotes::complete_vision`) or a
  vision endpoint, `PER_REQUEST` 4 pages. Cached in `page_render` by image sha + `PROMPT_VERSION`. At most
  `PAGES_PER_RUN` 48 per run, only while `quiet()`. The rest stay `pending`, indexed from text meanwhile, and are
  re-checked every 10 min.
- Spreadsheets: one section per row. A sheet over `REPORTING_ROWS` 200 rows gives only its schema plus
  `SAMPLE_ROWS` 8 rows.
- Teams and Otter transcripts saved as Word (`office::teams_turns`) become packed turns (`text::pack`). Their
  preface is kept because it dates them.
- `echoctl brain files`, and `echoctl brain read-file <path>` (hidden) to see one reader's pages and routing.

### Access and routing (`access.rs`)
- A call's text belongs to the profile pinned in its `meta.json` (`ai::call_profile_id`). If that can't be read,
  the call belongs nowhere. Mail, posts and folder documents belong to their threads' profiles.
- The MCP server serves one reader: `--profile P [--client C | --thread T… | --call call/<stamp>]` (default: the
  default profile). Others' sources answer "not found", and searches are filtered before the cut.
- Sessions: the Echo-folder session gets its profile. A call's session gets the call's profile and client, else its
  threads, else the call itself. `terminal::brain_mcp_config` writes one config file per access.
- The `<echo_view>` hook drops the open call and the selection for a session on another call or profile
  (`view::restrict`).
- Background jobs use the default profile and only its sources. A folder whose threads run on other profiles is
  indexed from text and never page-read.

### Scopes
`scopes(chunk_id, kind, id)` holds client and thread per section (`-` = none). Search, related and MCP take
`client` / `thread`. `list_scopes` names them. There is no Ask screen: asking means a Claude session with Recall
attached.

### Thread history (`store::supersede`)
- A thread's `Project.history_before` (Profile → History, `project set --history-before`) marks documents, posts and
  mail dated before **every** one of their threads' dates as `SUPERSEDED` (`valid_to` = that date).
- Only marks recorded in `date_superseded` are undone, when a date moves or is removed.
- Calls are never superseded (a thread's Since decides its calls). Undated documents never are either.

### Closed threads are the archive
- A closed thread keeps its mail, posts and folder documents, but reads nothing new (`sources::with_sources`,
  `files::folder::with_folders`). Its read state is kept.
- A section whose threads are all closed (`store::archived_of`, `projects::closed_threads`) still shows in search,
  marked `closed`, at `ARCHIVE_WEIGHT` 0.5 (but full weight within that thread's own scope).
- Terms seen only in closed threads are `archived`: kept out of new calls' hints (`taxonomy::terms_for_threads`) and
  folded in the Dictionary. The advert lists one "Archive: N closed threads" line.
- All of this is computed from the stages, so reopening a thread restores it with nothing re-read.

### MCP (`mcp.rs`)
- Read-only stdio JSON-RPC, server name `echo`. Tools are `mcp__echo__…`; `guard::BRAIN_TOOLS` allows them in a
  call session's ask mode only, never in a minutes run.
- Tools: `list_scopes`, `search_sources`, `list_sources`, `read_source`, `read_link`, `find_related`,
  `get_evidence`, `find_people`, `get_person`.
- `canonical` maps Brain's old names (`search_knowledge`, `read_document`, `find_related_content`) to the new ones.
  `get_taxonomy`, `get_metric` and `health` are gone and answer an error.
- `get_evidence` returns the stored page text, tables and PNG for folder pages (`files/folder.rs` assets).

### Telling Claude it is there (`advert.rs`, no model)
- `summary` covers what this reader's Recall holds (kinds with counts and dates, clients and threads with ids,
  people, last update), plus fixed rules on when to look there first.
- Where it goes: the MCP server's `instructions`, an `<echo_recall>` block at `SessionStart` of Echo's sessions
  (`session_block`, again after compact / clear, on resume only if it changed), routine prompts that have tools
  (`claudejob`), and a line in `<echo_view>`.
- `usage.json` counts tool calls by day and by `--who folder|call|routine`, plus window views. Shown in Settings.

### At scale (`browse.rs`)
- The window never loads everything. `/api/brain` carries no documents and no terms.
  `/api/brain/browse/{groups, sources, folder, terms, …}` pages 50 at a time, with cursor `date\tsource`.
- `documents` keeps list facts (`store::DOC_FACTS`).
- Aggregates over every section are `store::kept`: reused while `changed_stamp` is unchanged, recounted in the
  background otherwise. The last answer is served meanwhile, or the fresh one if it arrives within 250 ms. Counted
  once at app start.
- Term mentions: one scan per section (table `term_scan`). A sync scans only changed sections (`mentions_changed`).
  An edit re-reads only its terms through FTS (`mentions_of`), off the window's thread.

### Dictionary (`taxonomy.rs` terms)
- A term has a name, `synonyms` (other names, abbreviations, old names, used for search and mentions), `variants`
  (spellings and mishearings, used for the transcription hint and mentions), a group from `GROUPS` (else
  `group_of(kind)`), and a scope: the practice (shared layer) or one client. A client's terms are matched only in
  its sections.
- `split_variants` splits old terms at load: an abbreviation or a name under 0.75 alike becomes a synonym; a near
  spelling or another script becomes a spelling.
- Edit ops: `set`, `split`, `rename`, `merge` (`into_layer: shared`), `scope`. Renames and merges keep the old name
  as a synonym. Versions go to `taxonomy/` (`vN.json`, `current.json`, `decisions.jsonl`).
- Learned vocabulary lives here (`Term.learned`). A minutes run's `vocab add` calls `taxonomy::learn`: the term goes
  to the call's client's layer (else shared), pinned to its threads, merged by name or variant. The old
  `config.learned` list moved once (`vocab::migrate_learned`, a copy in `.echo/learned-before-taxonomy.json`).
  `vocab::learned_all` reads both. Transcription reads this even with Recall off.
- Plugins add terms through `taxonomy::dictionary_add` (merge or new, never rename or remove).

### Window views
- **Also discussed** (panel `also`, `views::also`): for each moment of a call, the nearest sections elsewhere.
  Threshold 0.6 for the same client, 0.75 for another client.
- **Mentioned also in** (work item details, `views::mentions`): for each quote, nearby sections in the same thread.
- Source panel `brain-doc` (`views::doc`). Sources tab folds a channel or folder into one row.
- Graphs (Sigma.js + graphology in `ui/vendor/`, `GraphView` in app.js): ForceAtlas2 runs in the page, not in a
  worker (CSP). `GraphView.batch` edits with the renderer off, because per-node redraws froze the page.

## Switched off (code kept)
`brain::EXTRAS` / `RECALL_EXTRAS` = `false`: these did not hold up in testing. Nothing runs them and the window hides
them. `tell_recall` sends one notice to users who had topics. Lasting facts now belong to the Knowledge plugin.
- **Topics**: two levels per layer (practice + per client, one tree via `Topic.parent`), built by `induce`, tagged
  by `classify`. Also `drift`, `moves`, `split`, `fold_into_practice`, and their Inbox items (`attention.rs` is
  gated).
- **Insights** (`insights.rs`): per-scope moved / slipping / recurs / contradiction / open, each citing at least 2
  sources with checked quotes.
- **Numbers** (`numbers/`): Brain's tabular semantic layer. Claude proposes the families, the user accepts, SQLite
  computes values. Measurements are in `docs/brain-design.md`.
- **Graph and Coverage tabs** of Recall.

## Decisions

| Decision | Why | Where |
|---|---|---|
| Off by default, including after an update | indexing mail and folders is the user's choice | `brain::embed::Settings::enabled` |
| Mail and posts are indexed only as digested facts | a forged instruction in a mail must never reach a session with tools | `brain::digest`, `brain::sources` |
| Trusted outside mail needs DMARC/compauth for the From domain | SPF + DKIM alone can pass for an attacker's domain | `graph::mail_auth` |
| Colleague by directory id, never by name | a display name proves nothing about the author | `brain::senders` |
| Unknown author or no org domains → defer, don't read | never a stranger's verdict | `brain::sources` (`LOOKUPS_MAX`) |
| Strict profile routing for every reader and job | a call's text must not leave its AI profile | `brain::access::Access` |
| Brain's chunk ids `sha256(source\0ord)[:8]>>1` | a re-sync keeps ids, tags and related edges | `brain::chunk` |
| Refs are part of the document hash | moving a call between threads re-scopes it | `brain::store::sync` |
| Snapshot before each changing sync; embed outside the transaction | roll back cleanly; an embedding outage keeps text search | `brain::store::sync` |
| `unicode61` FTS | Brain's ASCII-only query dropped Cyrillic | `brain::store::open` |
| Vectors searched in Rust, no sqlite-vec | fine for one user's store; nothing to ship | `brain::store` |
| Slides routed on LibreOffice's PDF, not their XML | the XML misjudged half the slides in real decks | `brain::files::render` |
| Old MCP tool names still answer | existing sessions and `kb` skills keep working | `brain::mcp::canonical` |
| Closed threads archive instead of dropping | never lose sources silently; reopening restores them for free | `brain::store::archived_of` |
| Expensive aggregates cached by store stamp | 2 million sections took seconds per view | `brain::store::kept` |

## Files and state

| Path | What | Written by |
|---|---|---|
| `<config dir>/brain.json` (0600) | enabled, auto, embedding / vision endpoint, model, key | Settings, `brain set` |
| `<home>/recall/knowledge.sqlite` | the store (chunks, FTS, vectors, documents, related, scopes, files, page_render, term_scan, date_superseded, taxonomy tables) | sync |
| `<home>/recall/snapshots/` | 3 last `VACUUM INTO` copies | sync |
| `<home>/recall/sources.json` | per-source read state | sources sync |
| `<home>/recall/senders.json` | trusted / ignored / unverified senders per client | user decisions, sync (merged) |
| `<home>/recall/reread.json` | clients whose sources to read again | sender decisions |
| `<home>/recall/digests/` | quarantine reader cache | digest |
| `<home>/recall/assets/<id>/` | page text, tables, PNG, LibreOffice PDF | folder sync |
| `<home>/recall/taxonomy/` | dictionary versions, `current.json`, `decisions.jsonl` | edits, `learn`, plugins |
| `<home>/recall/usage.json` | MCP and view usage counts | `advert::count` |
| `<home>/recall/recall-notice` | the one-time "Brain is now Recall" notice was sent | `tell_recall` |
| `<home>/recall/insights.json`, `numbers/` | switched-off lanes | — |

## Known gaps
- Bumping `READER` drops every mail and channel document and its read state, including a closed thread's archive.
  Reopened, the thread reads only its normal window again.
- No "Forget on close" for a thread's sources yet.
- The `mcp.rs` module doc still says it uses Brain's tool names and that lanes answer `not_modeled`. The tools were
  renamed and the extras removed.
