# The Brain in Echo — cross-call intelligence (plan)

Status: plan (2026-10-30), from a read of `onetest-ai/applied_skills` `bundles/brain` and `mcp/brain`. Decisions:
**one Brain per user**, **migrated into Echo and run by it** (a core feature, not a pipeline the user drives),
**facts only** (calls, minutes, chats, Teams channels, mail, local folders — the stakeholder map and the work ledger
are *referenced*, never ingested), **its intelligence shows everywhere** (calls, threads, clients, tasks and their
history). Sources are not copied into Echo: every fact points to the exact call moment, Teams message, mail or file.

## What we keep from Brain, and what changes

Brain's one rule stays: **meaning is agentic, numbers are computed** — retrieval and the graph explain, the numbers lane
computes, and every answer is cited or says "not modeled". Decided 2026-10-30 too: **the Brain is its own chapter** — its configuration per source lives where the source does
(a thread's folders, mail, channels — on the thread), its settings (the embedding endpoint and model, …) in Settings →
Brain, and exploring it (Ask, Topics, the taxonomy review, coverage, sources) in its own **Brain** activity, as Threads
has one. It is built on the `projects` branch (the epic).

Its store stays **schema-compatible** (`knowledge.sqlite`):
`chunks` (+ `chunks_fts`, `chunks_vec` — a plain table of vectors searched in Rust, no sqlite-vec), `related`, `graph_nodes` / `graph_edges` / `graph_aliases`,
`chunk_topics`, `chunk_verdicts`, `meta`, `sources`, `synced_files`, later `facts*` and `memory_*` — so the `kb`
skills and anything else that speaks Brain's MCP tools work on Echo's Brain unchanged.

| Brain today | In Echo | Why |
|---|---|---|
| Python scripts + a venv, an agent orchestrates create/update | Rust inside the app (`brain/` module); the app's scheduler orchestrates; LLM steps are guarded Claude jobs on an AI profile, as the sync is | core feature, no Python shipped, nobody has to drive it |
| `brain.toml` source roots, `parsed/*.md` on disk | sources = Echo's own: calls, threads' channels / mail / folders; each source's Markdown is built in memory and hashed (no `parsed/` copy) | nothing copied; the delta logic (sha per document) is unchanged |
| fastembed `bge-small-en-v1.5` (English only), run in-process | **no model in Echo**: embeddings from an endpoint the user configures — Ollama (`/api/embed`) or any OpenAI-compatible `/v1/embeddings` — with a multilingual model (e.g. `bge-m3`, `multilingual-e5`); the vector size is whatever it returns (`meta.embed_model`, `embed_dim`; another model re-embeds) | uk / ru / es calls; nothing to ship or run; the user's own server |
| FTS query keeps `[A-Za-z0-9]` only (Cyrillic dropped) | `unicode61` tokens of any script | BM25 works on every language we record |
| VTT/SRT → turns packed by time | calls come from Echo's segments: real speaker names (+ their organisation), `[mm:ss]`, the minutes' sections, the chat, the call's threads and client | better facts than any transcript file |
| taxonomy review app (137 KB HTML + server) | review in the Echo window (agents propose additions; renames, merges, removals are the user's) | one UI, Echo's tokens |
| FastMCP server (stdio / HTTP) | `echoctl brain mcp` (stdio, read-only, same tool names and shapes) — Echo's Claude sessions attach it (`--mcp-config`, strict); kb recognises it by its tools | kb and Claude Code use it as any Brain |
| marts from Excel (pandas) | later, from folder spreadsheets (calamine) | needed once folders come in |
| VLM page / frame transcription | later, a Claude job over page images of folder documents (and Echo's kept frames) | folders' slides and scans |

## Checked against a Brain built by the Python bundle (a reference, not migrated)

| Brain (Python) | Echo | Status |
|---|---|---|
| `chunks` (all columns), `chunks_fts`, `related`, `chunk_topics`, `chunk_verdicts`, `graph_nodes` / `graph_edges` / `graph_aliases`, `brain_schema`, `documents` | same; Echo adds a section's `link` and `refs`, a document's kind, title, date, refs, and `scopes` (client / thread) | kept |
| `chunks_vec` (vec0) | a plain table of unit vectors searched in Rust | equivalent |
| `meta` name, goal, audience, taxonomy_sha256 | written with each taxonomy version | kept |
| `entity_kind` nodes; entities in `current.json` by kind | entity-kind nodes, and each term a node (`term__…`) under its kind, with `mentions` edges | kept, richer |
| `current.json` goal, summary, intent tree, descriptions, aliases, history, demoted, review_flags | topics (with descriptions), terms (with variants), summary, demoted, review_flags (near-duplicate topics / terms), aliases, history | kept |
| entity kinds of its own (system, segment, initiative, region…) | open kinds (the model's or the user's) | to do with the per-client layer |
| `sources` + `synced_files` (registry of original files: path, sha, size, type, removed) | with folders (phase 5) | later |
| `page_render` (pages read by a model) | with folders (phase 5) | later |
| `facts`, `fact_versions`, `fact_reports`, `fact_merges`, `fact_notes`; `metrics` | the numbers lane with folders (phase 5) | later |
| `date_superseded`, status SUPERSEDED | newer revisions of a file (phase 5) | later |
| `memory_*` (assertions, questions) | unused there; covered by the work ledger and Insights | not carried |

Lessons from its `brain.toml`: sources are read in place (`import` from a synced folder); curated with reasons —
format twins dropped (PDF kept over PPTX), internal stand-ups and status calls excluded because evals scored better
without them. In Echo, a client's scope keeps internal calls out of its answers; a thread's folders get
include / exclude patterns with reasons, and twins are dropped; a question set per client can score a rebuild.

## Sources and identity

A source is a document with a stable id, a link and Echo's references:

| Source | Document | `source` id | Link | References kept on each chunk |
|---|---|---|---|---|
| Call | minutes sections, then the transcript as packed turns (`[mm:ss] Speaker (Org): text`), then the meeting chat | `call/<stamp>` | `echo://call/<name>?t=<s>` (opens the moment) | client, threads, speakers, time |
| Teams channel | one document per channel per day, messages in order, replies under their message | `teams/<team>/<channel>/<day>` | the message's `webUrl` | thread(s), client, authors |
| Mail | one document per conversation | `mail/<conversationId>` | the message's `webLink` | thread(s), client, people |
| Folder file | PDF / Word / PowerPoint text by page or heading; spreadsheets to the numbers lane | `file/<thread>/<relative path>` | `file://…` (Reveal in Finder) | thread(s), client |

- Chunk ids stay Brain's: `sha256(source \0 ord)[:8] >> 1`, so a re-sync keeps ids, tags and `related` edges.
- **Referenced, not ingested**: a chunk keeps the call's threads and client and the speakers' person keys; the Brain's
  MCP answers carry them, and Echo turns them into links (thread, person, task). The ledger's tasks and the
  stakeholder map are reached through those ids (`echoctl project …`), never copied into chunks.
- A source gone (a deleted message, a file removed) leaves the index at the next sync.

## Running it (no human pipeline)

- **Index** (deterministic, in the app): after a call's minutes are written, and every 15 min for channels / mail /
  folders (Graph delta queries; folder sha scan), build changed documents → chunk (Brain's heading-aware chunker, 1200
  chars, no overlap) → embed changed chunks only → FTS → `related` (k = 6, ≥ 0.55, across documents). One transaction,
  a snapshot before, roll back on failure (Brain's `brain_sync apply`).
- **Taxonomy** (Claude jobs, guarded like the sync): seeded from the threads' tags and glossaries and the user's
  `me.md`; *map → reduce (deterministic clustering) → judge → emit* on the first build; then **classify** new chunks in
  batches (≤ 150 a batch, 0–3 labels from the vocabulary, `__no_topic__` for filler). New categories are only
  proposed; the user decides renames, merges and removals in the window (migrations keep the tags).
- **Coverage** and drift checks after each run (Brain's `chunk_coverage`); a provisional taxonomy blocks nothing but is
  shown as such.
- The Obsidian vault is a view, regenerated: `<home>/brain/vault/` (one note per section, `_topics/`), opt-in.

## The taxonomy supersedes the glossaries (decided 2026-10-30)

One vocabulary for the user, kept by the Brain (`<home>/brain/taxonomy/`: `vN.json` immutable, `current.json`, history
and `decisions.jsonl`, as Brain's):
- **Topics** — two levels (Brain's L1 / L2 intent classes), what is talked about; sections are tagged with 0–3 of them.
- **Terms** — Brain's entities: names, systems, products, acronyms, milestones, each with its spelling, variants (how
  it is misheard or abbreviated) and a description. **This is what a thread's glossary was.**
- **Scope instead of lists**: a topic or term belongs to the threads (hence clients) whose sections are tagged with it
  or mention it; a term can be pinned to a thread. A thread's Profile shows its topics and terms instead of a glossary.
- **Transcription** reads the taxonomy: a call's hint = the terms scoped to its threads + the user's always-on ones
  (`vocab::for_call` changes source, not shape). What a minutes run learns (`vocab add`) becomes a proposed term.
  Today's thread glossaries and learned vocabulary move in once (as terms, scoped as they were).
- **Changes are reviewed** (Brain's rule): Claude proposes additions only; renames, merges, moves and removals are the
  user's, recorded as migrations so tags move with them, never silently pruned.
- **Jobs** (guarded Claude runs on the user's default AI profile): *induce* on the first build — map each call's minutes
  (topics, terms with evidence) → reduce (deterministic clustering of names) → judge → a proposed v0 the user accepts;
  then *classify* new sections in batches (≤ 150, 0–3 labels from the vocabulary, `__no_topic__` for filler) and
  *propose* new topics / terms when coverage drops. Coverage (tagged / untagged / filler) after each.

## Intelligence, everywhere (to be designed on the canvas first)

- **A call**: "Also discussed" — related sections from other calls, channels, mail and files (`find_related_content` of
  its chunks), each with its source and link; topics as chips.
- **No Ask screen** (decided 2026-10-30): asking is a Claude Code session with the Brain attached (both window sessions
  have it; a thread or client opens Claude scoped to it, and `<echo_view>` says which). **Client and thread are the
  Brain's dimensions** (`scopes`: every section indexed by its client and threads, `-` for calls in none; search,
  related, the taxonomy's counts and the MCP tools take `client` / `thread`; `list_scopes` names them).
- **A thread / a client**: **Insights** — themes that recur, positions that
  moved, contradictions, questions open across several calls, commitments that keep slipping — each citing ≥ 2
  sources word for word (checked as the ledger's quotes), with a last-verified date and retirement; **Topics** (the
  taxonomy facet of its sources).
- **When Insights update** (decided 2026-10-30 — never an agent per minute): a scope (client / thread) is redone only
  after new or changed sources in it since its last run; the run reads its previous insights, the new sections and
  their related older ones, and keeps / updates / retires / adds — cost follows what happened, not the history. At most
  once a day per scope, a daily cap of scopes, only while the computer is idle and on power; Refresh runs one now. An
  insight whose cited section changed or vanished is marked to re-check (no model) and handled at the next run.
- **Graph** (Brain activity): documents as dots coloured by client, clustered around the topics they are tagged with,
  related documents linked; filter by client / thread; a click opens the document. Drawn by Echo's own script (no
  library, no network).
- **Tasks and their history**: a task's timeline gains "mentioned also in…" from other sources (its quotes'
  related sections), so a commitment made in a call and confirmed in mail shows both.
- **Claude · <thread / client>** and the Echo-folder session have the Brain attached; `<echo_view>` tells them which
  thread / client / call "this" is.

## Phases

1. **Store + calls**: the schema (SQLite through `rusqlite`, the `brain` feature), the chunker, embeddings from the
   configured endpoint, FTS of any script, `related`, sync with snapshots; calls as sources; `echoctl brain
   status|search|sync|mcp`; Settings → Brain; the Claude sessions attached. Measured: index speed and answer quality
   on our calls (en / uk / ru).
2. **Taxonomy**: induction and classification as Claude jobs; the review in the window; coverage; it supersedes the
   thread glossaries and the learned vocabulary (moved in once; transcription reads it).
3. **Intelligence surfaces** (canvas first): the Brain activity (Topics, Terms, Sources, Coverage), Also discussed,
   Insights, Topics & terms on threads, task timelines; Claude scoped to a thread or client (no Ask screen).
4. **Channels and mail**: thread Sources (channels picked per thread, mail matched), Graph delta sync, links.
5. **Folders**: thread folders; PDF / Word / PowerPoint text; page images by a model; spreadsheets to the numbers lane.

Each phase ends with the usual checks and a run on the user's real data.

## Open questions

- Which embedding model to recommend (bge-m3, multilingual-e5, nomic…): measured on our calls in phase 1.
- Vectors are searched in Rust (brute force; ~50K chunks × 1024 floats is ~200 MB of reading — fine for one user's
  Brain); an index (sqlite-vec, HNSW) only if that proves slow.
- Existing project Brains (built with the bundle) stay as they are; importing one into Echo's is not planned.

## Phase 4 as built (2026-10-30)
Teams channels and mail are thread sources (`brain/sources.rs`), read into the index and linked back, never copied.
Measured on a test account: a domain rule first matched mostly calendar invites, mass mailings and the user's own address
at that domain — so those three are filtered; a team's General channel gave a few dozen posts (embedded in ~20 s, tagged
in ~30 s, some left as small talk). Searching that client now finds channel posts beside its calls.
Graph notes: `participants:` KQL → 500 (`OrFilter`); `cc:` matches loosely; channel messages need `$expand=replies`.

## Phase 5: thread folders — Brain's document lane, ported (plan, 2026-10-30)
Brain's lane (`parse_corpus`, `visual-parse`, `source_registry`, `scribe_marker`) is the reference; nothing of it is
migrated. A thread names folders (`Source { kind: "folder", id: <absolute path>, exclude: [globs] }`); a folder in two
threads is in both.
- **Registry** (`files` table in the store): root + relative path (NFC), sha256, size, mtime, state (active /
  removed), method, fidelity (full / degraded), event date + where it came from, skip reason, twin of. A file seen
  at a new path with a known hash is a **move** (nothing re-read); changed bytes → re-read; gone → removed with
  its sections.
- **Skips, each with its reason** (shown per file): hidden (any dot component, `._x`, `~$x`, OS-hidden), excluded
  (the folder's globs, `**` spans folders), Echo's own output (a minutes page's generator meta), unsupported format,
  too big, unreadable (with the reader's words), **twin** (the same stem in another format: the editable one —
  pptx > docx > xlsx > pdf — is read; the other is recorded as its twin).
- **Readers** (deterministic, no model): pptx and docx from their XML (text in order, headings, real table cells as
  Markdown grids; slides in presentation order, hidden slides out); xlsx/xlsm (`calamine`: per sheet, the densest
  early row as the header, one section per data row, ≤ 20 MB in full, else a sample); pdf (PDFium, loaded at runtime:
  the text layer per page, page objects counted); md/txt as they are; vtt/srt and Teams transcript .docx as packed
  turns; html as text (degraded).
- **Routing per page** (Brain's `render_pages` rule): a page/slide goes to the vision reader when its text likely
  misses the meaning — thin text (< 220 chars) and no table; dense drawing (≥ 60 drawing objects; for slides,
  shapes + connectors); a lighter diagram (≥ 28) with < 1000 chars; a chart or SmartArt with < 1000 chars. Other pages
  are indexed from their text.
- **Vision reader**: Claude on the default profile (its minutes model; images sent as content blocks, no tools), or
  an Ollama / OpenAI-compatible vision endpoint (Settings → Brain). Brain's deck instructions: a flow → an ordered
  list, a timeline → a table, a diagram → its relations, extracted table cells trusted for numbers, `# <title>` first.
  Images: PDF pages by PDFium; slides via LibreOffice → PDF (once per file version), then PDFium; no LibreOffice →
  that slide stays text, fidelity degraded. Cached by (image sha, prompt version): an unchanged page is never read
  twice.
- **Two representations**: the semantic one (vision Markdown or text layer) is chunked and embedded; the factual one
  (page PNG, text layer, table grids) stays in `<home>/brain/assets/<file key>/pNN.*` and is what `get_evidence`
  returns. Sections link to the file (opened in its app) and carry their page.
- **Dates**: Brain's `exact_date` — the first full date in a document's first 2000 chars (not ≥ today: an auto-updated
  field), else a full date in the file name; transcripts and spreadsheets only by file name; else none.
- Later: the numbers lane (`facts`) for reporting workbooks, supersession by date, recordings (Echo records its own
  calls).

### Phase 5 as built and measured (2026-10-30)
On a sample folder of consulting documents (a hundred-odd files): every reader read every file without an error. Routing,
measured against Brain's own rule on LibreOffice's PDFs of the decks: the rule flags about 40% of slides; a rule
from the slides' XML disagreed on half of them (consulting decks put text in free boxes and draw divider lines), so
slides are routed on the PDF — the XML still gives their exact text and tables. PDFs: about a third of pages flagged.
Spreadsheets: one section per row turned a few dozen books into millions of characters; reporting books (a sheet over
200 rows) now keep their schema and first rows (tens of thousands of characters for the largest). On a test folder of 10 files (a twin, a hidden
file, a lock file, an excluded file): the first sync read 48 of 60 flagged pages with Claude (4 a request, ~4 a minute)
and indexed 6 documents (552 sections) in 13.5 minutes; the second read the other 12 in under 3 minutes; the third
changed nothing (1.3 s). Readings keep tables as exact grids. Removing the folder removed its documents and kept the
page cache (re-adding costs no model).

## The numbers lane (2026-10-30)
Brain's `tabular-semantic-layer` ported (`brain/numbers/`), its config format kept so a Brain `families.<corpus>.json`
imports as is. Checked against a Brain built on a sample set of reporting workbooks: with its own config, Echo's build reproduced its
comparable facts exactly (wide_month, tolerant_long, collisions with weighted means, rollups); the only differences were
files outside the folder and values Brain had from an earlier version of a workbook. Claude's proposal from Echo's
profiles: a first pass recovered a quarter to two thirds of a series (the profile showed one file per series: template
drift unseen); with the series' other files' headers and two audit-driven repair rounds, about 90% overall. Nothing is served until the user accepts the families.
