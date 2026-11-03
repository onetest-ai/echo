// Echo window. Vanilla JS, no dependencies. Data comes from echo:// (`api/*`), actions go to the
// app over `window.ipc.postMessage`. Everything shown is escaped first: chat and transcripts are
// written by other people.
'use strict'

const $ = (sel, root = document) => root.querySelector(sel)
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const send = (msg) => window.ipc && window.ipc.postMessage(JSON.stringify(msg))
const get = async (path) => {
  const r = await fetch(path)
  if (!r.ok) throw new Error(`${r.status} ${path}`)
  return r.json()
}

const S = {
  view: 'calls', // calls | calendar
  cal: { mode: 'week', offset: 0, data: null, selected: null, loading: false },
  calls: null,
  agenda: null,
  state: null,
  selected: null,
  call: null,
  tab: 'minutes',
  query: '',
  find: '',
  jump: null, // seconds to scroll the transcript to
  confirmDelete: false,
  lightbox: null,
  error: null,
  liveDraft: '', // the question being typed in the Live tab (kept across refreshes)
  recordProfile: null, // the AI profile picked for the next recording (null = the default)
  notesProfile: null, // {call, id}: the AI profile picked for Write minutes
  prepOpen: {}, // call → the "Last time" card's open state, once the user toggled it
  autoTab: null, // the call whose tab was last chosen for the user (Live while it records)
  notice: null, // a message for a moment (an action that couldn't be done)
  closing: false, // the window is saving the user's notes before it closes (`prepareClose`)
  closeError: null, // why they couldn't be saved (the window stays open)
  renderOnBlur: false, // a render held back while the user types their notes
  overlay: false, // a narrow window shows the right panel over the page (asked for, never saved)
  stopAsk: false, // Stop was clicked: the pill asks before the recording ends
  listOpen: false, // a narrow window shows the calls list over the call
  menuOpen: false, // the call's ⋯ menu
  notesRaw: false, // My notes shows the file as text (Edit as text) instead of the timeline
  folderProfile: null, // the AI profile picked for Claude · Echo folder (null: the default)
  sessions: null, // Claude conversations that can be resumed (`<profile>:folder`, `<profile>:<stamp>`)
  settings: null, // Settings' snapshot (`api/settings`)
  tk: { data: null, project: '', sort: 'due', more: {} }, // Tasks: across projects
  br: { tab: 'topics', scope: '', data: null, graph: null, node: null, nodeData: null, edit: null }, // the Brain
  pj: { client: null, adding: null, pfEdit: null, menu: false, work: null, node: null, openOnly: true, view: 'tree', kind: '', owner: '', editField: null, found: false, foundData: null, data: null, sel: null, tab: 'brief', detail: null, briefEdit: null, showClosed: false, confirmDelete: false, newClient: false, picker: false }, // Projects
  setSection: 'general', // Settings' open section
  noteDraft: '', // the note being typed in My notes' box
  noteAt: null, // the call's time when it was started (seconds), while the call records
  noteDraftCall: null, // the call the note in the box is for (the one open at its first key)
  noteKind: 'note', // note | question | decision | todo
  selNote: null, // a comment on words selected in the transcript: {t, quote, x, y, open}
  noteEdit: null, // the note being edited in place: {i (its block), text, kind, call}
}

// ── the user's notes (Notes tab): saved over IPC, acknowledged, one save in flight per call ─
// Per call (by its stamp, so a retitle doesn't lose a draft): `draft` = the editor's text,
// `synced` = what the file was last known to hold. A save sends `{text, base: synced}`; the app
// writes it, keeping what tray → Add a note… appended meanwhile, and answers with what the file
// holds (`window.echo.saved`). An unchanged text is never sent: the file's time is the minutes'
// clock. No DOM in here: `scripts/check-mod.sh` drives it from a node test.
const NotesSync = (() => {
  const calls = new Map()
  let seq = 0
  let post = (msg) => send(msg)
  const stamp = (name) => String(name).slice(0, 17)
  const of = (name) => {
    let c = calls.get(stamp(name))
    if (!c) calls.set(stamp(name), (c = { name, draft: null, synced: null, inflight: null, error: null, kept: null, waiters: [], epoch: 0 }))
    c.name = name // the folder as it is named now
    return c
  }
  const settle = (c, err) => {
    const w = c.waiters.splice(0)
    for (const x of w) err ? x.reject(err) : x.resolve()
  }
  const dirty = (c) => c.draft !== null && c.draft !== (c.synced ?? '')
  function save(name) {
    const c = of(name)
    if (c.inflight) return
    if (!dirty(c)) return settle(c)
    c.inflight = { id: ++seq, text: c.draft }
    c.error = null
    post({ cmd: 'save_my_notes', call: c.name, id: c.inflight.id, text: c.draft, base: c.synced ?? '' })
  }
  return {
    setPost(f) { post = f },
    /** Which save answers a fetch saw (`seen` takes the epoch read when the fetch began). */
    epoch(name) { return of(name).epoch },
    /** What the file holds, from the call's details: the editor takes it unless it has unsaved text,
     *  or the fetch began before a save was answered (it may show the file from before). */
    seen(name, text, epoch) {
      const c = of(name)
      if (epoch !== undefined && epoch !== c.epoch) return
      // With unsaved text, `synced` stays what it was based on: the next save keeps what was
      // appended to the file since (the app merges by that base).
      if (c.inflight || (c.draft !== null && dirty(c))) return
      c.draft = text
      c.synced = text
    },
    edit(name, text) { of(name).draft = text },
    save,
    /** The app's answer to a save. */
    saved(r) {
      const c = of(r.call)
      if (!c.inflight || c.inflight.id !== r.id) return
      const sent = c.inflight.text
      c.inflight = null
      c.epoch++
      if (r.error) { c.error = r.error; return settle(c, new Error(r.error)) }
      c.synced = r.stored
      if (r.kept) c.kept = r.kept
      // What the tray appended meanwhile lands at the end, as in the file; typing went on at the caret.
      if (c.draft === sent) c.draft = r.stored
      else if (r.stored.startsWith(sent)) c.draft += r.stored.slice(sent.length)
      if (dirty(c)) save(c.name)
      else settle(c)
    },
    /** Resolves once the file holds the draft (rejects when a save fails). */
    flush(name) {
      const c = of(name)
      if (!c.inflight && !dirty(c)) return Promise.resolve()
      const p = new Promise((resolve, reject) => c.waiters.push({ resolve, reject }))
      save(name)
      return p
    },
    flushAll() { return Promise.all([...calls.values()].filter((c) => c.inflight || dirty(c)).map((c) => this.flush(c.name))) },
    view(name) {
      const c = of(name)
      return { draft: c.draft ?? '', saving: !!c.inflight, dirty: dirty(c), error: c.error, kept: c.kept }
    },
    clearKept(name) { of(name).kept = null },
  }
})()

// ── my notes as moments: each note a markdown line of my-notes.md ──────────────────────────
// `- [12:05] text` (a note at that time of the call), `- [12:05] ? text` (a question), `! ` (a
// decision), `⚑ ` (a flagged moment), `- [ ] [12:22] text` (a to-do; `[x]` done), and the words it
// is about on the next line as `  > "…"`. A line without a time is a general note; anything else
// (headings, paragraphs) stays as written. The file stays the user's plain notes: the tray appends
// `- [mm:ss] …` too, and the minutes run reads it as it is. No DOM in here: `my-notes.test.mjs`.
const NoteDoc = (() => {
  const TIME = /^\[(\d{1,2}):(\d{2})(?::(\d{2}))?\]\s*/
  const BOX = /^\[(\s?|x|X)\]\s*/
  const secs = (m) => (m[3] != null ? Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]) : Number(m[1]) * 60 + Number(m[2]))
  /** `[mm:ss]`'s text: 05:07, 1:02:03. */
  const stamp = (t) => {
    t = Math.max(0, Math.round(t))
    const h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), s = t % 60
    const two = (n) => String(n).padStart(2, '0')
    return h ? `${h}:${two(m)}:${two(s)}` : `${two(m)}:${two(s)}`
  }
  const MARK = { question: '?', decision: '!', flag: '⚑' }
  /** The text as blocks: a note (its line and its quote line) or any other line as it is. */
  function parse(text) {
    const lines = String(text ?? '').split('\n')
    const out = []
    for (let i = 0; i < lines.length; i++) {
      const l = lines[i]
      const m = l.match(/^- (.*)$/)
      if (!m) { out.push({ note: false, raw: l }); continue }
      // A to-do's box before or after the time, written [], [ ] or [x]: `- [ ] [12:22] …`,
      // `- [12:22] [] …`, `- [][12:22] …`.
      let rest = m[1], t = null, box = null
      const takeBox = () => { const b = rest.match(BOX); if (b && box == null) { box = b[1]; rest = rest.slice(b[0].length) } }
      takeBox()
      const tm = rest.match(TIME)
      if (tm) { t = secs(tm); rest = rest.slice(tm[0].length) }
      takeBox()
      let kind = box != null ? 'todo' : 'note'
      if (kind === 'note') {
        for (const [k, c] of Object.entries(MARK)) if (rest === c || rest.startsWith(c + ' ')) { kind = k; rest = rest.slice(c.length).trim(); break }
      }
      const raw = [l]
      let quote = null
      while (i + 1 < lines.length && /^\s{2,}>/.test(lines[i + 1])) {
        const q = lines[++i].replace(/^\s+>\s?/, '').replace(/^"|"$/g, '')
        quote = quote ? `${quote} ${q}` : q
        raw.push(lines[i])
      }
      out.push({ note: true, kind, t, done: box === 'x' || box === 'X', text: rest.trim(), quote, raw: raw.join('\n') })
    }
    return out
  }
  const flat = (s) => String(s ?? '').replace(/\s+/g, ' ').trim()
  /** A note as its markdown line (and quote line). */
  function line(n) {
    const time = n.t != null ? `[${stamp(n.t)}] ` : ''
    const head = n.kind === 'todo' ? `- [${n.done ? 'x' : ' '}] ${time}` : `- ${time}${MARK[n.kind] ? MARK[n.kind] + ' ' : ''}`
    const body = (head + flat(n.text)).trimEnd()
    return n.quote ? `${body}\n  > "${flat(n.quote).replace(/"/g, '”')}"` : body
  }
  /** Add a note: in time order among the timed ones (a comment on an earlier moment goes there),
   *  a general one at the end. */
  function add(text, n) {
    const blocks = parse(text)
    while (blocks.length && !blocks[blocks.length - 1].note && blocks[blocks.length - 1].raw === '') blocks.pop()
    const at = n.t == null ? -1 : blocks.findIndex((b) => b.note && b.t != null && b.t > n.t)
    const nb = { note: true, raw: line(n) }
    if (at < 0) blocks.push(nb); else blocks.splice(at, 0, nb)
    return blocks.map((b) => b.raw).join('\n') + '\n'
  }
  /** Change the note at block `i` (`null`: remove it). */
  function change(text, i, patch) {
    const blocks = parse(text)
    const b = blocks[i]
    if (!b || !b.note) return text
    if (patch === null) blocks.splice(i, 1)
    else blocks[i] = { ...b, raw: line({ ...b, ...patch }) }
    return blocks.map((x) => x.raw).join('\n')
  }
  /** What was typed in the note box: a leading `?`, `!` or `[]` picks the kind, and a `[mm:ss]` before
   *  or after it the time (`t`, else undefined). */
  function typed(s, kind) {
    let v = String(s ?? '').trim(), t
    const time = () => { const m = v.match(TIME); if (m && t === undefined) { t = secs(m); v = v.slice(m[0].length) } }
    time()
    const p = v.match(/^(\?|!|\[\s?\])\s*/)
    if (p) { kind = p[1] === '?' ? 'question' : p[1] === '!' ? 'decision' : 'todo'; v = v.slice(p[0].length) }
    time()
    return t === undefined ? { kind, text: v.trim() } : { kind, text: v.trim(), t }
  }
  /** The note box's text added as Enter adds it — `{text, kind, at}`, `at` the call's time taken at its
   *  first key (null: none) unless the text names one — or null when there is nothing to add. */
  function addTyped(text, p) {
    const n = typed(p.text, p.kind)
    if (!n.text) return null
    const t = n.t !== undefined ? n.t : p.at
    return add(text, { kind: n.kind, t: t == null ? null : Math.floor(t), text: n.text })
  }
  /** A note edited in place (`{i, text, kind}`) saved as Save saves it: its text and kind change, its
   *  time and quote stay; null when emptied (delete is the × beside it). */
  function saveEdit(text, ed) {
    const t = flat(ed.text)
    if (!t && ed.kind !== 'flag') return null
    return change(text, ed.i, { text: t, kind: ed.kind })
  }
  return { parse, line, add, change, typed, stamp, addTyped, saveEdit }
})()
// end of the notes block

// ── projects: how a project is drawn, and its stages (no DOM: `projects.test.mjs`) ────────────
// A project shows as a small tile: its logo if it has one, else a symbol the user picked, else an
// identicon from its id (a 5×5 pattern mirrored left to right, as GitHub draws a user without an
// avatar) — in its colour. The same id always gives the same pattern.
const ProjectArt = (() => {
  /** FNV-1a of a string, as an unsigned 32-bit number. */
  function hash(str) {
    let h = 0x811c9dc5
    for (const ch of String(str)) { h ^= ch.codePointAt(0); h = Math.imul(h, 0x01000193) >>> 0 }
    return h >>> 0
  }
  /** 25 cells, row by row: columns 0–2 from the hash, 3–4 mirror 1–0. Never (nearly) empty or full. */
  function cells(id) {
    const h = hash(id)
    let bits = Array.from({ length: 15 }, (_, i) => (h >>> i) & 1)
    const on = bits.reduce((a, b) => a + b, 0)
    if (on < 4 || on > 12) bits = bits.map((b, i) => (i % 2 ? b : 1 - b))
    const out = []
    for (let r = 0; r < 5; r++) for (let c = 0; c < 5; c++) out.push(bits[r * 3 + (c < 3 ? c : 4 - c)] === 1)
    return out
  }
  const STAGES = [
    ['pursuit', 'Pursuit', [['lead', 'Lead'], ['proposal', 'Proposal'], ['negotiation', 'Negotiation']]],
    ['delivery', 'Delivery', [['kickoff', 'Kickoff'], ['active', 'Active'], ['on_hold', 'On hold'], ['wrap_up', 'Wrap-up']]],
    ['closed', 'Closed', [['done', 'Done'], ['lost', 'Lost'], ['cancelled', 'Cancelled']]],
  ]
  /** A stage's label and its group's ("on_hold" → {label: 'On hold', group: 'Delivery'}). */
  function stage(id) {
    for (const [gid, glabel, xs] of STAGES) for (const [sid, label] of xs) if (sid === id) return { id, label, group: glabel, gid }
    return { id, label: id, group: '', gid: '' }
  }
  return { hash, cells, STAGES, stage }
})()
// end of the projects block

// ── ⌘K palette: ranking what was typed (no DOM: `palette.test.mjs`) ────────────────────────
// Each item: `{label, group, keywords?, disabled?}`. A query matches when its words all appear
// in the label (or keywords), in any order; whole-word and earlier matches rank first, and
// unavailable items after available ones. With no query, the items keep their own order.
const Palette = (() => {
  const norm = (s) => String(s ?? '').toLowerCase().normalize('NFKD').replace(/[\u0300-\u036f]/g, '')
  const atStart = (hay, i) => i === 0 || /[\s·:,/(-]/.test(hay[i - 1])
  // A word anywhere in the label; in the keywords only at a word's start ("min" isn't "terminal").
  function score(item, words) {
    const label = norm(item.label), keys = norm(item.keywords)
    let total = 0
    for (const w of words) {
      const i = label.indexOf(w)
      if (i >= 0) { total += (atStart(label, i) ? 30 : 10) - Math.min(i, 20) * 0.5; continue }
      const k = keys.split(/\s+/).some((x) => x.startsWith(w))
      if (!k) return -1
      total += 5
    }
    return total
  }
  return {
    /** The items for `query`, best first (at most `limit`). */
    rank(items, query, limit = 50) {
      const words = norm(query).split(/\s+/).filter(Boolean)
      if (!words.length) return items.slice(0, limit)
      return items
        .map((it, i) => ({ it, i, s: score(it, words) }))
        .filter((x) => x.s >= 0)
        .sort((a, b) => (!!a.it.disabled - !!b.it.disabled) || (b.s - a.s) || (a.i - b.i))
        .slice(0, limit)
        .map((x) => x.it)
    },
    /** The same items with each group kept together, groups in the order of their best item. */
    grouped(items) {
      const order = [...new Set(items.map((x) => x.group))]
      return order.flatMap((g) => items.filter((x) => x.group === g))
    },
  }
})()
// end of the palette block

// ── workbench: what the window is made of, and where each view goes ────────────────────────
// The window is fixed areas (activity bar, primary view, right and bottom panels) filled from
// these lists; Echo's own views are entries like a plugin's will be. No DOM in here:
// `workbench.test.mjs` drives it from node.
const ICON = (d) => `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`
/** Activities (the activity bar, in order). `layout`: `list-detail` (a list beside the item) or `single`. */
/** The Brain is built in and switched on (Settings → Brain); off, it shows nowhere. */
const brainOn = () => !(typeof S !== 'undefined' && S.state && S.state.brain === false)
/** The activities shown: those whose `when` (if any) holds. */
const shownActivities = () => ACTIVITIES.filter((a) => !a.when || a.when())
const ACTIVITIES = [
  { id: 'calls', label: 'Calls', layout: 'list-detail', icon: ICON('<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>') },
  { id: 'calendar', label: 'Calendar', layout: 'single', icon: ICON('<rect x="3.5" y="5" width="17" height="15" rx="2.5"/><path d="M3.5 10h17M8 3v4M16 3v4"/>') },
  { id: 'projects', label: 'Threads', layout: 'single', icon: ICON('<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/><path d="M8 13h8"/>') },
  { id: 'tasks', label: 'Tasks', layout: 'single', icon: ICON('<rect x="4" y="4" width="16" height="16" rx="3.5"/><path d="M8.5 12.2l2.4 2.4 4.6-4.9"/>') },
  { id: 'brain', label: 'Brain', layout: 'single', when: () => brainOn(), icon: ICON('<path d="M9 4.5a3 3 0 0 0-3 3 3 3 0 0 0-2 5.2A3.2 3.2 0 0 0 7.5 18 2.5 2.5 0 0 0 12 19V6a2 2 0 0 0-3-1.5z"/><path d="M15 4.5a3 3 0 0 1 3 3 3 3 0 0 1 2 5.2A3.2 3.2 0 0 1 16.5 18 2.5 2.5 0 0 1 12 19"/>') },
  { id: 'settings', label: 'Settings', layout: 'single', bottom: true, icon: ICON('<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>') },
]
/** A call's tabs, in order. `x` = what the call has: `{recording, chat, frames, actions}` (counts, or
 *  null for actions not written yet). One Transcript tab: live while the call records (with its live
 *  notes beside it), the final transcript after — and the live notes stay beside it. */
const CALL_TABS = [
  { id: 'minutes', label: 'Minutes' },
  { id: 'transcript', label: 'Transcript' },
  { id: 'chat', label: 'Chat', when: (x) => x.chat > 0, count: (x) => x.chat },
  { id: 'screens', label: 'Screens', when: (x) => x.frames > 0, count: (x) => x.frames },
  { id: 'actions', label: 'Actions', count: (x) => x.actions ?? '' },
]
/** The tabs a call shows: `[id, label, count ('' = none)]`; while it records, Transcript comes first
 *  with a fourth element, `true` (its recording dot). */
const callTabs = (x) => {
  const tabs = CALL_TABS.filter((t) => !t.when || t.when(x)).map((t) => [t.id, t.label, t.count ? t.count(x) : ''])
  if (!x.recording) return tabs
  const i = tabs.findIndex((t) => t[0] === 'transcript')
  return [[...tabs[i], true], ...tabs.filter((_, j) => j !== i)]
}
/** What a calendar event's card offers, first match wins: `e` the event, `st` its timing
 *  (`{past, live, recordable}`). The card draws each kind (`eventDetail`). */
const EVENT_ACTIONS = [
  { kind: 'open-notes', when: (e) => !!e.recorded },
  { kind: 'import', when: (e, st) => st.past && !!e.join_url && !e.cancelled && !e.declined },
  { kind: 'scheduled', when: (e, st) => st.recordable && !!e.scheduled },
  { kind: 'join', when: (e, st) => st.recordable && st.live && !!e.join_url },
  { kind: 'record', when: (e, st) => st.recordable },
]
const eventAction = (e, st) => (EVENT_ACTIONS.find((a) => a.when(e, st)) || {}).kind || null
/** Views that open in the right or bottom panel: `{id, label, area, when(), render()}` (`when`:
 *  shown with the current activity; added where their renderers live: My notes below the Notes code). */
const PANEL_VIEWS = []

/** Where panel views go and which panels are open: `<config dir>/window-layout.json`, per computer,
 *  saved over IPC (`save_layout`). A view the user never moved sits in its own `area`. */
const Layout = (() => {
  const AREAS = ['right', 'bottom']
  const SIZE = { right: 320, bottom: 220 }
  const fresh = () => ({ right: { open: true, size: SIZE.right, active: null }, bottom: { open: false, size: SIZE.bottom, active: null }, views: {} })
  let L = fresh()
  const num = (v, lo, hi, d) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d)
  const areaOf = (v) => (L.views[v.id] && AREAS.includes(L.views[v.id].area) ? L.views[v.id].area : v.area)
  return {
    AREAS,
    /** Take a saved layout; anything malformed falls back to the default. */
    load(o) {
      L = fresh()
      if (!o || typeof o !== 'object') return
      for (const a of AREAS) {
        const s = o[a] || {}
        L[a] = { open: typeof s.open === 'boolean' ? s.open : L[a].open, size: num(s.size, 160, 900, SIZE[a]), active: typeof s.active === 'string' ? s.active : null }
      }
      for (const [id, v] of Object.entries(o.views || {})) if (v && AREAS.includes(v.area)) L.views[id] = { area: v.area }
    },
    toJSON() { return JSON.parse(JSON.stringify(L)) },
    /** The known views in an area, in their own order. */
    views(area, known) { return known.filter((v) => areaOf(v) === area).map((v) => v.id) },
    area(id, known) { const v = known.find((x) => x.id === id); return v ? areaOf(v) : null },
    /** Move a view to the other area and show it there. */
    move(id, area, known) {
      if (!AREAS.includes(area) || !known.some((v) => v.id === id)) return false
      L.views[id] = { area }
      for (const a of AREAS) if (L[a].active === id && a !== area) L[a].active = null
      L[area].open = true
      L[area].active = id
      return true
    },
    toggle(area) { if (AREAS.includes(area)) L[area].open = !L[area].open },
    /** A panel dragged to `size` px (null: back to its default). */
    resize(area, size) { if (AREAS.includes(area)) L[area].size = size == null ? SIZE[area] : num(size, 160, 900, SIZE[area]) },
    show(id, known) { const a = this.area(id, known); if (a) { L[a].open = true; L[a].active = id } },
    /** Forget views that no longer exist (a plugin removed). */
    prune(known) { for (const id of Object.keys(L.views)) if (!known.some((v) => v.id === id)) delete L.views[id] },
    /** What to draw at this window size: an area with no views is not drawn; below 900 px wide the
     *  right panel opens over the page — only when asked for there (`overlay`: this session's
     *  choice, never saved) — and below 600 px high the bottom one is folded to its tab strip. */
    effective(width, height, known, overlay = false) {
      const out = { narrow: width < 900 }
      for (const a of AREAS) {
        const ids = this.views(a, known)
        const active = ids.includes(L[a].active) ? L[a].active : ids[0] || null
        out[a] = { views: ids, active, shown: ids.length > 0 && L[a].open, size: L[a].size }
      }
      if (out.narrow && !overlay) out.right.shown = false
      out.right.overlay = out.right.shown && out.narrow
      out.bottom.folded = out.bottom.shown && height < 600
      return out
    },
  }
})()
// end of the workbench block

// ── formatting ─────────────────────────────────────────────────────────────────────────────
const clock = (s) => {
  s = Math.max(0, Math.round(s))
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60
  const mm = String(m).padStart(h ? 2 : 1, '0'), ss = String(sec).padStart(2, '0')
  return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}
const hm = (d) => d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
const dayLabel = (d) => {
  const today = new Date(); today.setHours(0, 0, 0, 0)
  const that = new Date(d); that.setHours(0, 0, 0, 0)
  const diff = Math.round((today - that) / 86400000)
  if (diff === 0) return 'Today'
  if (diff === 1) return 'Yesterday'
  if (diff < 7) return that.toLocaleDateString([], { weekday: 'long' })
  return that.toLocaleDateString([], { day: 'numeric', month: 'long', year: today.getFullYear() === that.getFullYear() ? undefined : 'numeric' })
}
const STATUS = {
  recording: 'Recording', transcribing: 'Transcribing', notes: 'Minutes', transcript: 'Transcript', failed: 'Failed', audio: 'Audio',
}
const speaker = (seg) => (seg.speaker === 'me' || seg.speaker === 'mic') ? 'Me' : (seg.name || 'Remote')

// ── brand: the onetest logo (the full octopus character on its violet disc) ────────────────
const mark = (cls = '') => `<img class="logo ${cls}" src="assets/logo.png" alt="" aria-hidden="true">`

// ── data ───────────────────────────────────────────────────────────────────────────────────
async function loadState() {
  try { S.state = await get('api/state') } catch { /* app restarting */ }
}
async function loadCalls() {
  try { S.calls = await get('api/calls'); S.error = null } catch (e) { S.error = String(e) }
}
async function loadProjects() {
  try { S.pj.data = await get('api/projects') } catch { /* app restarting */ }
}
async function loadTasks() {
  try { S.tk.data = await get('api/tasks') } catch { /* app restarting */ }
}
/** The work of the project a Tasks card is in (its details panel reads it). */
async function loadWork() {
  const id = S.pj.sel
  if (!id) return
  try { const w = await get('api/project/' + encodeURIComponent(id) + '/work'); if (S.pj.sel === id) S.pj.work = w } catch { /* app restarting */ }
}
/** The People tab's data: the open thread's or client's (`people.rs`). */
async function loadPeople() {
  const url = S.pj.client ? `api/client/${encodeURIComponent(S.pj.client)}/people` : S.pj.sel ? `api/project/${encodeURIComponent(S.pj.sel)}/people` : null
  if (!url) { S.pj.people = null; return }
  try { const d = await get(url); d.url = url; S.pj.people = d } catch { /* app restarting */ }
}
/** The Brain as one thread sees it: its topics and terms (shared layer + its client's), for its
 *  Profile and Brief. */
async function loadThreadBrain() {
  const id = S.pj.sel
  if (!id) return
  try { const d = await get(`api/brain?thread=${encodeURIComponent(id)}`); d.thread = id; if (S.pj.sel === id) S.pj.tbrain = d } catch { /* no Brain in this build */ }
}
async function loadFound() {
  try { S.pj.foundData = await get('api/found') } catch { /* app restarting */ }
}
async function loadProject() {
  if (S.pj.found) return loadFound()
  if (S.pj.client) {
    const id = S.pj.client
    try { const d = await get('api/client/' + encodeURIComponent(id)); if (S.pj.client === id) S.pj.cdata = d } catch { /* app restarting */ }
    if (S.pj.ctab === 'people') await loadPeople()
    return
  }
  const id = S.pj.sel
  if (!id) { S.pj.detail = null; return }
  try { const d = await get('api/project/' + encodeURIComponent(id)); if (S.pj.sel === id) S.pj.detail = d } catch { /* app restarting */ }
  if (S.pj.tab === 'people') await loadPeople()
  if ((S.pj.tab === 'profile' || S.pj.tab === 'brief') && (!S.pj.tbrain || S.pj.tbrain.thread !== S.pj.sel)) await loadThreadBrain()
  if (S.pj.tab === 'work') {
    try { const w = await get('api/project/' + encodeURIComponent(id) + '/work'); if (S.pj.sel === id) S.pj.work = w } catch { /* app restarting */ }
  }
}
async function loadAgenda() {
  try { S.agenda = await get('api/agenda') } catch { S.agenda = [] }
}
async function loadLayout() {
  try { Layout.load(await get('api/layout')); Layout.prune(PANEL_VIEWS) } catch { /* the default layout */ }
}
let callSeq = 0
async function loadCall(name) {
  const mine = ++callSeq, epoch = NotesSync.epoch(name)
  try {
    const c = await get('api/call/' + encodeURIComponent(name))
    // Another call was chosen while this answer came (it is not shown, nor its notes edited as that
    // one), or a newer fetch was started: only the newest answer counts.
    if (S.selected !== name || mine !== callSeq) return
    S.call = c
    // A file that can't be read isn't "empty": the editor stays away from it (`notesTab` says why).
    if (c.my_notes_error == null) NotesSync.seen(S.call.name, S.call.my_notes || '', epoch)
    syncEditor()
  } catch (e) {
    if (S.selected !== name || mine !== callSeq) return
    // Retitled meanwhile (the notes run renames the folder): the same call keeps its stamp.
    const stamp = name.slice(0, 17)
    const moved = (S.calls || []).find((c) => c.name !== name && c.name.slice(0, 17) === stamp)
    if (moved && S.selected === name) { S.selected = moved.name; history.replaceState(null, '', `#call/${moved.name}/${S.tab}`); return loadCall(moved.name) }
    S.call = { error: String(e) }
  }
}

// ── routing: #calls, #call/<name>/<tab>, #today ────────────────────────────────────────────
function route() {
  const h = decodeURIComponent(location.hash.slice(1))
  const cm = h.match(/^calendar(?:\/(day|week))?$/)
  if (cm) { S.view = 'calendar'; if (cm[1]) S.cal.mode = cm[1]; return }
  const sm = h.match(/^settings(?:\/([a-z0-9:-]+))?$/)
  // The vocabulary was a Settings section: its old link opens its panel, over the calls.
  if (sm && sm[1] === 'vocabulary') { S.view = 'calls'; setTimeout(() => { Layout.show('vocabulary', panelViews()); layoutChanged() }); return }
  if (sm) { S.view = 'settings'; if (sm[1]) S.setSection = sm[1]; return }
  if (h === 'tasks') { S.view = 'tasks'; return }
  const bm = h.match(/^brain(?:\/(\w+))?$/)
  if (bm && !brainOn()) { S.view = 'calls'; return }
  if (bm) { S.view = 'brain'; if (bm[1]) S.br.tab = bm[1]; return }
  if (h === 'found') { S.view = 'projects'; S.pj.found = true; S.pj.client = null; S.pj.sel = null; S.pj.detail = null; return }
  const cm2 = h.match(/^client\/([a-z0-9-]+)(?:\/(\w+))?$/)
  if (cm2) {
    S.view = 'projects'; S.pj.found = false
    if (S.pj.client !== cm2[1]) { S.pj.cdata = null; S.pj.briefEdit = null; S.pj.person = null }
    S.pj.client = cm2[1]; S.pj.ctab = cm2[2] || S.pj.ctab || 'overview'; S.pj.sel = null; S.pj.detail = null; S.pj.adding = null; return
  }
  const pm = h.match(/^projects?(?:\/([a-z0-9-]+))?(?:\/(\w+))?$/)
  if (pm) {
    S.view = 'projects'
    S.pj.found = false
    S.pj.client = null
    if (pm[1] && S.pj.sel !== pm[1]) { S.pj.adding = null; S.pj.pfEdit = null; S.pj.menu = false; S.pj.sel = pm[1]; S.pj.detail = null; S.pj.work = null; S.pj.person = null; S.pj.ppAdding = false; S.pj.node = S.pj.wantNode || null; S.pj.wantNode = null; S.pj.briefEdit = null; S.pj.confirmDelete = false }
    if (pm[2]) S.pj.tab = pm[2]
    return
  }
  S.view = 'calls'
  const m = h.match(/^call\/([^/]+)(?:\/(\w+))?$/)
  if (m) {
    // Another call: a note typed or edited on this one goes into its notes first.
    if (S.selected !== m[1]) { commitPendingNotes(); S.selected = m[1]; S.call = null; S.confirmDelete = false; S.find = '' }
    if (m[2]) S.tab = m[2]
  }
}
const go = (hash) => { if (location.hash.slice(1) !== hash) location.hash = hash; else render() }
/** An activity, where the user left it (the open call and tab; the calendar's day or week). */
const goActivity = (id) => go(id === 'calendar' ? `calendar/${S.cal.mode}` : id === 'settings' ? `settings/${S.setSection}`
  : id === 'tasks' ? 'tasks' : id === 'brain' ? `brain/${S.br.tab}`
  : id === 'projects' ? (S.pj.found ? 'found' : S.pj.client ? `client/${S.pj.client}/${S.pj.ctab || 'overview'}` : S.pj.sel ? `project/${S.pj.sel}/${S.pj.tab}` : 'projects') : (S.selected ? `call/${S.selected}/${S.tab}` : 'calls'))

// ── language of the next recording (Record ▾) ──────────────────────────────────────────────
// Per viewer, remembered in this webview; English unless chosen otherwise. '' = auto: Echo
// detects among the configured languages (Settings → Languages).
const LANG_KEY = 'echo.recordLanguage'
function recordLang() {
  try { const v = localStorage.getItem(LANG_KEY); if (v !== null) return v } catch { /* storage off */ }
  return 'en'
}
function setRecordLang(v) { try { localStorage.setItem(LANG_KEY, v) } catch { /* storage off */ } }
const LANG_NAMES = (() => { try { return new Intl.DisplayNames(['en'], { type: 'language' }) } catch { return null } })()
const langName = (c) => { try { return (LANG_NAMES && LANG_NAMES.of(c)) || c.toUpperCase() } catch { return c.toUpperCase() } }
/** The languages on offer: English, then the configured ones; and the current choice. */
function langChoices() {
  const configured = (S.state && S.state.languages) || []
  const opts = [...new Set(['en', ...configured])]
  const cur = recordLang()
  return { configured, opts, cur: cur === '' || opts.includes(cur) ? cur : 'en' }
}
// ── AI profiles: which model (and account) writes a call's live notes and minutes ──────────
const ai = () => (S.state && S.state.ai && S.state.ai.profiles) ? S.state.ai : { profiles: [], default: null, live_notes: false, error: S.state && S.state.ai && S.state.ai.error }
const profileOf = (id) => ai().profiles.find((p) => p.id === id)
const profileName = (id) => { const p = profileOf(id); return p ? p.name : (id || '') }
/** Where a profile's requests go, in words. */
const profileWhere = (p) => !p ? '' : p.kind === 'local' ? (p.host ? `at ${p.host}` : 'on this computer') : p.kind === 'claude' ? `with your Claude account${p.account ? ` (${p.account})` : ''}` : 'as Claude Code is set up'
/** The profile shown for the next recording: the one picked here, else the default. */
function recordProfile() { const p = S.recordProfile; return p && profileOf(p) ? p : ai().default }
/** What a recording is sent with: only a profile picked here (else the app pins its default when
 *  the recording starts, and a broken AI setting never stops it). */
function pickedProfile() { const p = S.recordProfile; S.recordProfile = null; return p && profileOf(p) ? p : undefined }
/** A profile in a menu: a Claude one with the account its folder is signed in to ("— not signed
 *  in"; the recorded one until checked). */
function profileLabel(p) {
  if (!p || p.kind !== 'claude') return p ? p.name : ''
  const s = p.signin || {}
  if (s.state === 'signed_in' || s.state === 'other_account') return `${p.name} — ${s.account}`
  if (s.state === 'signed_out') return `${p.name} — not signed in`
  return p.account ? `${p.name} — ${p.account}` : p.name
}
function profileOptions(cur) {
  return ai().profiles.map((p) => `<option value="${esc(p.id)}"${p.id === cur ? ' selected' : ''}>${esc(profileLabel(p))}</option>`).join('')
}

/** A Claude profile's sign-in and what can be done about it (`signin.rs` → `p.signin`); '' when
 *  there is nothing to say: signed in (not just now), or not known yet and no error. */
function signinNote(p) {
  const s = p && p.kind === 'claude' && p.signin
  if (!s) return ''
  const id = esc(p.id), name = esc(p.name)
  const btn = (attrs, label, cls = 'btn') => `<button class="${cls}" ${attrs}>${esc(label)}</button>`
  const signIn = (label = 'Sign in…') => btn(`data-signin="${id}"`, label, 'btn primary')
  const note = (text, buttons, kind = '', more = '') => `<div class="signin ${kind}"><span class="signin-text">${text}</span>${buttons ? `<span class="signin-acts">${buttons}</span>` : ''}${more ? `<span class="hint">${more}</span>` : ''}</div>`
  const cli = `echoctl ai login ${p.id}`
  switch (s.state) {
    case 'signed_out': return note(`${name} isn't signed in.`, signIn())
    case 'starting': return note(`Starting the sign-in of ${name}…`, '', 'busy')
    case 'checking': return note('Checking the sign-in…', '', 'busy')
    case 'waiting': return note(`Sign in to ${name} in your browser${s.email ? ` (as ${esc(s.email)})` : ''}…`, btn(`data-signin-cancel="${id}"`, 'Cancel', 'btn ghost'), 'busy',
      `Browser didn't open? Cancel, then run <code>${esc(cli)}</code> in Terminal <button class="btn ghost" data-copy="${esc(cli)}">Copy</button>`)
    case 'stopping': return note('Stopping the sign-in…', '', 'busy')
    case 'elsewhere': return note(`${name} is being signed in in a terminal…`, '', 'busy')
    case 'failed': return note(`The sign-in of ${name} didn't finish: ${esc(s.error || '')}`, signIn('Try again'), 'error')
    case 'other_account': return note(`${name} is signed in as <b>${esc(s.account)}</b>, not ${esc(s.expected || 'the account it was set up with')}.`,
      btn(`data-signin-accept="${id}" data-account="${esc(s.account)}"`, `Use ${s.account}`) + signIn('Sign in again…'), 'error')
    case 'signed_in': return s.just ? note(`Signed in as ${esc(s.account)}.`, '', 'ok') : ''
    default: return s.error ? note(`${name}: ${esc(s.error)}`, btn(`data-signin-check="${id}"`, 'Check again', 'btn ghost') + signIn(), 'error') : ''
  }
}
const CARET = '<svg viewBox="0 0 10 6" width="8" height="5" aria-hidden="true"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>'
/** A compact native select over a label (as the language picker): `data-<attr>`. */
function profilePick(attr, cur, cls = 'btn', title = 'Model for this call') {
  if (ai().profiles.length < 2) return ''
  const short = profileName(cur)
  return `<label class="${cls} split-lang pick" title="${esc(title)}: ${esc(profileLabel(profileOf(cur)) || short)}"><span>${esc(short.length > 14 ? short.slice(0, 13) + '…' : short)}</span>${CARET}<select ${attr} aria-label="${esc(title)}">${profileOptions(cur)}</select></label>`
}

/** A record button with the language picker on its right (a native select over the caret), and
 *  the AI profile picker when there is more than one. */
function recordButton(attrs, label, primary = true) {
  const { configured, opts, cur } = langChoices()
  const cls = primary ? 'btn primary' : 'btn'
  const auto = configured.length > 1 ? `Detect (${configured.map((c) => c.toUpperCase()).join(', ')})` : 'Detect the language'
  const options = opts.map((c) => `<option value="${esc(c)}"${c === cur ? ' selected' : ''}>${esc(langName(c))}</option>`).join('')
    + `<option value=""${cur === '' ? ' selected' : ''}>${esc(auto)}</option>`
  const title = cur ? `Transcribed as ${langName(cur)}` : 'Echo detects the language'
  return `<span class="split"><button class="${cls} split-main" ${attrs}>${esc(label)}</button><label class="${cls} split-lang" title="${esc(title)}"><span>${cur ? esc(cur.toUpperCase()) : 'Auto'}</span>${CARET}<select data-reclang aria-label="Language of the call">${options}</select></label>${profilePick('data-recprofile', recordProfile(), cls, 'Model for live notes and minutes')}</span>`
}

// ── views ──────────────────────────────────────────────────────────────────────────────────
const MAC = /Mac/.test(navigator.platform || navigator.userAgent)
const MOD = MAC ? '⌘' : 'Ctrl+'

/** A calendar meeting about to start (3 min before … 4 min after), not recorded or armed yet. */
function meetingSoon() {
  const now = Date.now()
  return (S.agenda || []).find((m) => {
    const s = new Date(m.start).getTime()
    return m.id && m.join_url && now >= s - 3 * 60000 && now <= s + 4 * 60000
  }) || null
}

/** The recording's clock: `data-since` (ms), counted by `tickClocks` between renders. */
const since = (iso) => { const t = iso ? new Date(iso).getTime() : NaN; return Number.isFinite(t) ? t : null }
function tickClocks() {
  for (const el of document.querySelectorAll('[data-since]')) {
    const t = Number(el.dataset.since)
    if (t) el.textContent = clock((Date.now() - t) / 1000)
  }
}

/** Record / Stop at the top centre of the window, in every state; the sign-in a recording's
 *  profile needs, under it. */
function pill() {
  const st = S.state || {}
  if (st.recording) {
    if (S.stopAsk) {
      return `<div class="pill rec"><span class="ptitle">Stop recording?</span><button class="pbtn stop" data-cmd="stop">Stop</button><button class="pbtn ghost" data-cmd="stop-keep">Keep recording</button></div>`
    }
    const name = String(st.recording).split(/[\\/]/).pop()
    const c = (S.calls || []).find((x) => x.name === name)
    const t0 = since(c && c.started_at)
    return `<div class="pill rec" role="group" aria-label="Recording"><span class="dot" aria-hidden="true"></span>${t0 ? `<span class="clock" data-since="${t0}">${clock((Date.now() - t0) / 1000)}</span>` : ''}<button class="ptitle link" data-call="${esc(name)}" title="Open this call">${esc(titleOf(st.recording))}</button><button class="pbtn stop" data-cmd="stop-ask"><svg viewBox="0 0 10 10" width="9" height="9" aria-hidden="true"><rect width="10" height="10" rx="2" fill="currentColor"/></svg>Stop</button></div>`
  }
  if (st.armed) {
    return `<div class="pill armed" role="group" aria-label="Armed"><span class="ring" aria-hidden="true"></span><span class="ptitle">Armed — records the next Teams call</span><button class="pbtn ghost" data-cmd="stop">Cancel</button></div>`
  }
  const soon = !st.in_call && meetingSoon()
  const main = soon
    ? `<div class="pill soon" role="group" aria-label="Meeting soon"><span class="clock">${esc(hm(new Date(soon.start)))}</span><span class="ptitle">${esc(soon.subject || 'Meeting')}</span><button class="pbtn" data-arm="${esc(soon.id)}">Record it</button></div>`
    : `<div class="pill idle">${recordButton('data-cmd="record"', st.in_call ? 'Record this call' : 'Record next call')}</div>`
  // A call being transcribed doesn't take the pill: Record stays one click away.
  const tr = (S.calls || []).find((c) => c.status === 'transcribing')
  const chip = tr ? `<button class="pchip" data-call="${esc(tr.name)}" title="${esc(tr.title)}">Writing the transcript…</button>` : ''
  const sign = signinNote(profileOf(recordProfile()))
  return main + chip + (sign ? `<div class="pill-note">${sign}</div>` : '')
}

function actbar() {
  const btn = (a, i) => {
    const found = a.id === 'projects' && S.pj.data ? S.pj.data.found_count || 0 : 0
    const badge = a.id === 'settings' && settingsWarnings() ? `<span class="act-badge" aria-label="${settingsWarnings()} to fix">${settingsWarnings()}</span>`
      : found ? `<span class="act-badge new" aria-label="${found} found for you">${found}</span>` : ''
    return `<button class="act" data-go="${a.id}"${S.view === a.id ? ' aria-current="page"' : ''} title="${esc(a.label)} (${a.id === 'settings' ? MOD + ',' : MOD + (i + 1)})">${a.icon}<span>${esc(a.label)}</span>${badge}</button>`
  }
  const acts = shownActivities()
  const top = acts.filter((a) => !a.bottom), bottom = acts.filter((a) => a.bottom)
  // Inbox: not an activity — it shows or hides its panel over whatever is open.
  const n = nyOpen().length, urgent = nyOpen().some((i) => i.urgent)
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay)
  const shown = Layout.AREAS.some((a) => eff[a].shown && eff[a].active === 'needs')
  const needs = `<button class="act${shown ? ' pressed' : ''}" data-panel-flip="needs" aria-pressed="${shown}" title="Inbox (${NY_HINT})">${NY_ENTRY}<span>Inbox</span>${n ? `<span class="act-badge${urgent ? '' : ' new'}" aria-label="${n} open">${n}</span>` : ''}</button>`
  return `<nav class="acts" aria-label="Activities">${top.map(btn).join('')}<span class="acts-grow"></span>${needs}${bottom.map((a) => btn(a, acts.indexOf(a))).join('')}</nav>`
}

function statusbar() {
  const a = ai(), p = profileOf(a.default)
  const parts = []
  if (p) parts.push(`<span title="${esc(profileWhere(p))}">AI: ${esc(p.name)}</span>`)
  if (a.live_notes) parts.push('<span>Live notes on</span>')
  // The terminal panel: a toggle here, where VS Code keeps it.
  parts.push(`<button class="sb-btn" data-pal-open title="Actions (${MAC ? '⌘K' : 'Ctrl+K'})">Actions ${MAC ? '⌘K' : 'Ctrl+K'}</button>`)
  const live = [...Terms.entries()].some(([k, e]) => k.startsWith('claude:') && e.id != null && !e.exited)
  parts.push(`<button class="sb-btn" data-cmd="claude-panel" title="Claude (${MOD}\`)">${live ? '● ' : ''}Claude</button>`)
  return `<span class="grow"></span>${parts.join('')}`
}

/** A panel (right or bottom) with its views as tabs; nothing when it has none (`Layout.effective`). */
function panel(area, eff) {
  const p = eff[area]
  if (!p.shown) return ''
  const other = area === 'right' ? 'bottom' : 'right'
  const v = PANEL_VIEWS.find((x) => x.id === p.active)
  // Each view by its icon (its name as the tooltip); one without an icon by its name.
  const tabs = p.views.map((id) => { const x = PANEL_VIEWS.find((y) => y.id === id); return `<button class="tab${x.icon ? ' icon-tab' : ''}" role="tab" data-panel-show="${esc(id)}" aria-selected="${id === p.active}" title="${esc(x.label)}" aria-label="${esc(x.label)}">${x.icon || esc(x.label)}</button>` }).join('')
  const moveIcon = other === 'right'
    ? ICON('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>')
    : ICON('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 14h18"/>')
  return `<div class="panel-resize ${area}" data-resize="${area}" title="Drag to resize · double-click: default size" aria-hidden="true"></div><div class="panel-head"><div class="tabs" role="tablist">${tabs}</div><span class="panel-title">${v ? esc(v.label) : ''}</span>
    <button class="icon-btn" data-panel-move="${esc(p.active)}" data-to="${other}" title="Move to the ${other} panel" aria-label="Move to the ${other} panel">${moveIcon}</button>
    <button class="icon-btn" data-panel-toggle="${area}" title="Close the panel" aria-label="Close the panel">${ICON('<path d="M6 6l12 12M18 6L6 18"/>')}</button></div>
    <div class="panel-body">${v ? v.render() : ''}</div>`
}

/** Show Claude: the call's session on a call (or `view`), else the folder's; again: hide it. */
function claudePanel(view) {
  const known = panelViews()
  const id = view || (S.view === 'calls' && S.selected ? 'claude-call' : 'claude-folder')
  const a = Layout.area(id, known)
  if (!a) return
  const eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (!view && eff[a].shown && eff[a].active === id) Layout.toggle(a); else { if (eff.narrow && a === 'right') S.overlay = true; Layout.show(id, known) }
  layoutChanged()
}

let layoutTimer = null
/** The layout changed: draw it, and save it a moment later (one write for a burst of changes). */
function layoutChanged() {
  clearTimeout(layoutTimer)
  layoutTimer = setTimeout(() => send({ cmd: 'save_layout', layout: Layout.toJSON() }), 500)
  render()
}

function titleOf(dir) {
  if (!dir) return ''
  const name = String(dir).split(/[\\/]/).pop()
  const c = (S.calls || []).find((x) => x.name === name)
  return c ? c.title : name
}

/** A call's status as a small icon left of its title (its name as the tooltip). */
const STATUS_ICON = {
  notes: '<path d="M6 3h8l4 4v14H6z"/><path d="M9 13.5l2 2 4-4.5"/>',
  transcript: '<path d="M5 7h14M5 11h14M5 15h9"/>',
  transcribing: '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>',
  failed: '<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17v.5"/>',
  audio: '<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>',
}
function statusIcon(status) {
  const label = STATUS[status] || status || ''
  const inner = status === 'recording' ? '<span class="rec-dot"></span>'
    : `<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${STATUS_ICON[status] || STATUS_ICON.transcript}</svg>`
  return `<span class="si ${esc(status || '')}" title="${esc(label)}" aria-label="${esc(label)}">${inner}</span>`
}

function list() {
  const q = S.query.trim().toLowerCase()
  const calls = (S.calls || []).filter((c) => !q || c.title.toLowerCase().includes(q) || c.people.some((p) => p.toLowerCase().includes(q)))
  let body
  if (S.calls === null) body = `<div class="skeleton" style="padding:12px">${'<i></i>'.repeat(8)}</div>`
  else if (S.error) body = `<p class="error" style="padding:12px">Can't read the calls: ${esc(S.error)}</p>`
  else if (!calls.length) body = q ? `<p style="padding:14px;color:var(--text-3)">Nothing matches "${esc(S.query)}".</p>` : ''
  else {
    let group = null
    body = calls.map((c) => {
      const d = c.started_at ? new Date(c.started_at) : null
      const g = d ? dayLabel(d) : 'Undated'
      const head = g !== group ? `<div class="group">${esc(g)}</div>` : ''
      group = g
      const meta = [d ? hm(d) : '', c.imported ? 'Teams transcript' : '', c.minutes ? `${c.minutes} min` : '', c.people.length ? c.people.slice(0, 2).join(', ') + (c.people.length > 2 ? ` +${c.people.length - 2}` : '') : ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
      const pjs = ((S.pj.data && S.pj.data.by_call) || {})[c.name] || []
      const icons = pjs.length ? `<span class="pj-icons">${pjs.slice(0, 3).map((p) => projIcon(p, 14)).join('')}</span>` : ''
      return `${head}<button class="row" role="option" data-call="${esc(c.name)}" aria-selected="${c.name === S.selected}">
        ${statusIcon(c.status)}
        <span class="t">${esc(c.title)}</span>
        <span class="m">${icons}${meta}</span>
      </button>`
    }).join('')
  }
  return `<section class="list">
    <div class="list-head"><div class="list-title"><h1>Calls</h1><button class="icon-btn" data-panel-show="vocabulary" title="Vocabulary — names and terms transcription should hear right" aria-label="Vocabulary">${BOOK}</button></div><input class="search" id="q" placeholder="Search titles and people" value="${esc(S.query)}" aria-label="Search calls"></div>
    <div class="rows" role="listbox" aria-label="Calls">${body}</div>
  </section>`
}

/** In a narrow window the list folds away: this opens it over the call (`.list-open`). */
function listToggle() {
  const c = S.selected && (S.calls || []).find((x) => x.name === S.selected)
  const d = c && c.started_at ? new Date(c.started_at) : null
  return `<button class="btn ghost list-toggle" data-cmd="list-toggle" aria-expanded="${S.listOpen}">Calls${d ? ` · ${esc(dayLabel(d))}` : ''} ${CARET}</button>`
}

function emptyMain() {
  if (S.calls && !S.calls.length) {
    return `<div class="empty"><div>${mark()}<h3>No calls yet</h3><p>Use Record at the top of this window or the menu-bar octopus. Echo transcribes each call on this computer when it ends.</p></div></div>`
  }
  return `${listToggle()}<div class="empty"><div>${mark()}<h3>Pick a call</h3><p>Its minutes, your notes, the transcript, chat and shared screens open here.</p></div></div>`
}

/** The call view in regions: `plain` (nothing to show yet), else its header and its pane — on the
 *  Minutes tab with minutes, the pane's bar and its frame apart, so a refresh that changes the bar
 *  never reloads the page in the frame (`render`). */
function doc() {
  if (!S.selected) return { plain: emptyMain() }
  const c = S.call
  if (!c) return { plain: `<div class="pane"><div class="skeleton">${'<i></i>'.repeat(10)}</div></div>` }
  if (c.error) return { plain: `<div class="empty"><div><h3>This call can't be opened</h3><p class="error">${esc(c.error)}</p></div></div>` }
  const d = c.started_at ? new Date(c.started_at) : null
  const segs = Array.isArray(c.segments) ? c.segments : []
  const chat = Array.isArray(c.chat) ? c.chat : []
  const frames = Array.isArray(c.frames) ? c.frames : []
  const actions = Array.isArray(c.actions) ? c.actions : null
  const tabs = callTabs({ recording: c.status === 'recording', chat: chat.length, frames: frames.length, actions: actions ? actions.length : null })
  // While a call records (or is transcribed), open it on its Transcript once; once its minutes are
  // written, on Minutes once. The user's own choice stays. (`live` was the Transcript's live twin.)
  if (S.tab === 'live') S.tab = 'transcript'
  if (S.autoTab !== c.name && (c.status === 'recording' || c.status === 'transcribing')) { S.autoTab = c.name; S.tab = 'transcript' }
  if (S.autoTab === c.name && S.tab === 'transcript' && c.notes && c.notes_run && c.notes_run.state === 'done') { S.autoTab = 'done:' + c.name; S.tab = 'minutes' }
  if (!tabs.some((t) => t[0] === S.tab)) S.tab = 'minutes'
  const people = c.people && c.people.length ? `<span class="people">${esc(c.people.slice(0, 6).join(', '))}${c.people.length > 6 ? ` and ${c.people.length - 6} more` : ''}</span>` : ''
  const head = `${listToggle()}<div class="head-row">
        <div style="min-width:0">
          <h2 class="selectable">${esc(c.title)}</h2>
          <div class="facts">${d ? `<span>${esc(d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }))}, ${esc(hm(d))}</span>` : ''}${c.minutes ? `<span>${c.minutes} min</span>` : ''}${c.imported ? '<span title="Made from the meeting\'s Teams transcript: no audio, no live notes">from Teams transcript</span>' : ''}${people}</div>
          ${callProjects(c)}
        </div>
        <div class="doc-actions">${callMenu(c)}</div>
      </div>
      <div class="tabs" role="tablist">${tabs.map(([id, label, n, rec]) => `<button class="tab" role="tab" data-tab="${id}" aria-selected="${S.tab === id}">${rec ? '<span class="tab-rec" aria-label="Recording"></span>' : ''}${label}${n !== '' ? `<span class="n">${n}</span>` : ''}</button>`).join('')}</div>${notices()}`
  const body = tabBody(c, segs, chat, frames, actions)
  const confirm = S.confirmDelete ? confirmBox(c) : ''
  return typeof body === 'string' ? { head, pane: confirm + body } : { head, bar: confirm + body.bar, frame: body.frame }
}

/** The call's one set of actions: a ⋯ menu (the folder, the minutes page, delete). */
function callMenu(c) {
  const item = (cmd, label, cls = '') => `<button class="menu-item${cls}" role="menuitem" data-cmd="${cmd}">${esc(label)}</button>`
  const items = [
    item('reveal', 'Show folder'),
    c.notes ? item('open-minutes', 'Open minutes in browser') : '',
    c.notes ? item('reveal-minutes', `${SHOW_FILE.replace('Show in', 'Show minutes in')}`) : '',
    c.notes ? item('write-notes', 'Write minutes again') : '',
    item('ask-claude', 'Ask Claude about this call'),
    slotItems('call.menu', { kind: 'call', call: c }, 'menu'),
    '<div class="menu-sep" role="separator"></div>',
    item('ask-delete', 'Delete…', ' danger'),
  ].join('')
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay)
  const a = Layout.area('notes', panelViews())
  const on = !!(a && eff[a].shown && eff[a].active === 'notes')
  const notes = `<button class="icon-btn${on ? ' on' : ''}" data-panel-flip="notes" aria-pressed="${on}" title="My notes" aria-label="My notes">${ICON('<path d="M5 4h10l4 4v12H5z"/><path d="M15 4v4h4M8 12h8M8 16h6"/>')}</button>`
  return notes + `<button class="icon-btn" data-cmd="call-menu" aria-haspopup="menu" aria-expanded="${S.menuOpen}" aria-label="Call actions" title="Call actions">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.menuOpen ? `<div class="menu" role="menu">${items}</div>` : ''}`
}

/** A message for a moment (an action that couldn't be done), and why the window didn't close: under
 *  a call's tabs, or above whatever else the window shows (`noticesBar`). */
/** Messages for a moment float over the window (`toasts`, drawn once in #r-over): nothing in the
 *  page moves when one comes or goes. These stay as empty slots for the places that drew them. */
function notices() { return '' }
function toasts() {
  const n = S.notice ? `<div class="toast"><span class="grow">${esc(S.notice)}</span><button class="btn ghost" data-cmd="notice-ok">OK</button></div>` : ''
  const c = S.closeError ? `<div class="toast error"><span class="grow">Your notes couldn't be saved: ${esc(S.closeError)}</span><button class="btn" data-cmd="close-retry">Try again</button><button class="btn ghost danger" data-cmd="close-now">Close without saving</button></div>` : ''
  return n + c
}

function noticesBar() { return '' }

function confirmBox(c) {
  return `<div class="confirm"><span>Delete "${esc(c.title)}"? It goes to the trash in the data folder for 30 days.</span>
    <button class="btn" data-cmd="cancel-delete">Keep</button><button class="btn primary" style="background:var(--danger);border-color:var(--danger)" data-cmd="delete">Delete</button></div>`
}

/** Transcript turns: consecutive lines of one speaker joined; `q` highlights, `jump` (seconds)
 *  marks the turn to scroll to. */
/** A turn's longest stretch, in seconds, before the same speaker's next line starts a new one. */
const TURN_MAX = 60
function turnsHtml(segs, q, jump, marks = []) {
  // One speaker's lines join a turn while they follow each other (< 8 s apart), each line its own
  // paragraph; a turn runs at most TURN_MAX seconds, then the next starts with its own time — during a
  // call every other voice is one "Remote" track, which would otherwise be one block of the whole call.
  const turns = []
  for (const s of segs) {
    const last = turns[turns.length - 1]
    if (last && last.who === speaker(s) && s.start - last.end < 8 && s.start - last.start < TURN_MAX) { last.text += ' ' + s.text; last.parts.push(s.text); last.end = s.end }
    else turns.push({ who: speaker(s), me: speaker(s) === 'Me', spk: s.speaker_id, start: s.start, end: s.end, lang: s.lang, text: s.text, parts: [s.text] })
  }
  const jumpIdx = jump == null ? -1 : Math.max(0, turns.findIndex((t) => t.end >= jump - 2))
  return `<div class="transcript">${turns.map((t, i) => {
    const hit = (q && t.text.toLowerCase().includes(q)) || i === jumpIdx
    // Detected remote speakers each get a color of their own (speakers.rs numbers them).
    const spk = !t.me && t.spk ? ` spk${((t.spk - 1) % 6) + 1}` : ''
    // My notes at this turn's moment: a dot each in the margin (the note's text on hover).
    const next = turns[i + 1] ? turns[i + 1].start : Infinity
    const mine = marks.filter((m) => m.t >= t.start - 1 && m.t < next - 1)
    const dots = mine.length ? `<span class="nt-marks">${mine.map((m) => `<span class="nt-dot k-${m.kind}" title="${esc((NOTE_LOOK[m.kind] || NOTE_LOOK.note)[1] + (m.text ? ': ' + m.text : ''))}"></span>`).join('')}</span>` : ''
    return `<div class="turn ${t.me ? 'me' : 'remote'}${spk}${hit ? ' hit' : ''}" data-t="${t.start}" ${i === jumpIdx ? 'id="jump"' : ''}>
      <time>${dots}${clock(t.start)}</time>
      <div><div class="who">${esc(t.who)}${t.lang ? `<span class="lang">${esc(t.lang)}</span>` : ''}</div>${t.parts.map((x) => `<p>${esc(x)}</p>`).join('')}</div></div>`
  }).join('')}</div>`
}

function tabBody(c, segs, chat, frames, actions) {
  if (S.tab === 'transcript') {
    // Live notes kept (or being kept): the transcript with them beside it, as during the call.
    if (hasLive(c)) return liveTab(c, segs)
    if (!segs.length) return c.status === 'recording'
      ? `<p style="color:var(--text-3)">Listening. The live transcript appears here as people speak.</p>`
      : `<p style="color:var(--text-3)">${c.status === 'transcribing' ? 'Transcribing on this computer. This takes about a quarter of the call length.' : 'No transcript for this call.'}</p>`
    return `<div class="from-bar"><input class="search" id="find" placeholder="Find in transcript" value="${esc(S.find)}" style="max-width:280px" aria-label="Find in transcript"></div>
      ${turnsHtml(segs, S.find.trim().toLowerCase(), S.jump, noteMarks(c))}`
  }
  if (S.tab === 'chat') {
    return `<div class="chat">${chat.map((m) => `<article class="msg"><header><b>${esc(m.from)}</b><time>${m.t == null ? '' : m.t < 0 ? 'before the call' : clock(m.t)}</time></header>
      <p>${esc(m.text)}</p>${m.files && m.files.length ? `<div class="files">${esc(m.files.join(', '))}</div>` : ''}</article>`).join('')}</div>`
  }
  if (S.tab === 'screens') {
    return `<div class="frames">${frames.map((f) => `<button class="frame" data-frame="${esc(f.url)}"><img src="${esc(f.url)}" alt="Shared screen at ${clock(f.t || 0)}" loading="lazy"><span>${clock(f.t || 0)}</span></button>`).join('')}</div>`
  }
  if (S.tab === 'actions') {
    if (!actions && c.actions_error) return minutesHint(c, "The action items can't be read", `${c.actions_error} — the next minutes replace them (the file is kept beside them).`)
    if (!actions) return minutesHint(c, 'No action items yet', 'Action items come with the minutes.')
    const changed = callChanges(c)
    if (!actions.length) return `${changed}<p style="color:var(--text-3)">No action items in this call.</p>`
    const links = c.action_tasks || {}
    return `${changed}${changed ? '<div class="cc-h">Action items</div>' : ''}<div class="todo">${actions.map((a) => {
      const k = links[a.id], kp = k && projById(k.project)
      const done = a.status === 'done'
      // By its id, with the status wanted: a list rewritten meanwhile can't make it another item.
      return `<div class="item${done ? ' done' : ''}">
        <button class="check" role="checkbox" aria-checked="${done}" data-toggle="${esc(a.id || '')}" data-status="${done ? 'todo' : 'done'}" aria-label="${done ? 'Mark to do' : 'Mark done'}"${a.id ? '' : ' disabled'}></button>
        <div><div class="task">${esc(a.task)}</div><div class="meta">
          <span class="owner${a.owner === 'Me' ? ' me' : ''}">${esc(a.owner || 'Unassigned')}</span>
          ${a.due ? `<span class="due">Due ${esc(a.due)}</span>` : ''}
          ${a.confidence === 'low' ? '<span class="uncertain">Uncertain</span>' : ''}
          ${k ? `<button class="cc-link" data-tk="open" data-project="${esc(k.project)}" data-node="${esc(k.id)}" title="Tracked as ${esc(k.title)}">${k.state === 'done' ? '✓ ' : ''}${kp ? projIcon(kp, 12) : ''}${esc(k.id)}</button>` : ''}
        </div></div>
        ${typeof a.t === 'number' ? `<button class="btn ghost" data-at="${a.t}">${clock(a.t)}</button>` : '<span></span>'}
      </div>`
    }).join('')}</div>`
  }
  return minutesTab(c)
}

/** What this call changed in its threads' work (`ledger::call_changes`): one line per change, in
 *  the order said — what happened, to which item, the words, the verifier's mark. A task an action
 *  item became shows on that item instead (`action_tasks`); changes the verifier called wrong, none. */
const CC_KIND = { story: 'New story', task: 'New task', decision: 'Decision', risk: 'Risk', question: 'Question' }
function ccWhat(x) {
  const n = x.node
  if (x.op === 'create') return [n.kind === 'task' && n.type === 'session' ? 'New session' : n.kind === 'task' && n.type === 'dependency' ? 'New dependency' : CC_KIND[n.kind] || 'New', n.kind]
  if (x.effect) {
    const e = x.effect
    return [e === 'due' ? `Due ${x.due ? fmtDay(x.due) : x.due_text || ''}` : e === 'held' ? 'Held' : e === 'done' ? 'Done' : e === 'progress' ? 'Progress' : e === 'blocked' ? 'Blocked'
      : e === 'cancelled' ? 'Cancelled' : e === 'reopen' ? 'Reopened' : e === 'delayed' ? 'Slipped' : e === 'reassign' ? 'New owner' : e, e === 'done' || e === 'held' ? 'done' : e === 'blocked' || e === 'delayed' ? 'warn' : 'move']
  }
  if (x.op === 'link') return ['Now waits', 'link']
  if (x.op === 'waiting') return [x.what ? 'Waiting' : 'Not waiting', 'warn']
  if (x.op === 'ac_met') return ['Criterion met', 'done']
  if (x.op === 'revise') return ['Revised', 'move']
  if (x.op === 'answer') return [n.kind === 'risk' ? 'Response' : 'Answered', 'done']
  return [x.op, 'move']
}
function callChanges(c) {
  const blocks = (c.changes || []).map((g) => {
    const p = projById(g.project) || { id: g.project, name: g.project }
    const linked = new Set(Object.values(c.action_tasks || {}).filter((k) => k.project === g.project).map((k) => k.id))
    const rows = g.changes.filter((x) => x.verdict !== 'wrong' && !(x.op === 'create' && linked.has(x.node.id)))
    const head = `<div class="cc-th">${projIcon(p, 14)}<b>${esc(p.name)}</b>`
    if (!g.read) {
      // Not read yet: being read now, due at a time (15 min after the call, then the next look), or
      // nothing to read (no transcript).
      const at = g.reads_at && new Date(g.reads_at)
      const why = g.syncing ? 'is reading this call now…'
        : g.queued ? 'hasn\'t read this call yet'
        : at ? (at > new Date() ? `reads this call after ${esc(hm(at))} — 15 minutes after it ended` : 'reads this call within a few minutes')
        : 'has nothing to read in this call (no transcript)'
      const canSync = !g.syncing && (g.queued || at)
      return `${head}<span class="set-sub">${why}</span><span class="grow"></span>${canSync ? `<button class="link-btn" data-cc-sync="${esc(g.project)}">Sync now</button>` : ''}</div>`
    }
    const hidden = linked.size ? ` · ${linked.size} on the action items below` : ''
    if (!rows.length) return `${head}<span class="set-sub">nothing changed${hidden}</span></div>`
    return `${head}<span class="set-sub">${rows.length} change${rows.length === 1 ? '' : 's'}${hidden}</span></div><div class="cc-list">${rows.map((x) => {
      const [what, cls] = ccWhat(x)
      const mark = x.applied === false && x.verdict === 'supports' ? '<span class="wk-v weak" title="You set this yourself; the call\'s words are kept in its history">kept yours</span>'
        : x.verdict === 'weak' ? '<span class="wk-v weak">weak · to review</span>'
        : x.op !== 'create' && x.verdict == null && x.applied === false ? '<span class="wk-v">to check</span>' : ''
      const ts = x.ts ? hms(x.ts) : null
      return `<div class="cc-row"><span class="cc-what ${esc(cls)}">${esc(what)}</span>
        <div class="cc-main"><button class="cc-node" data-tk="open" data-project="${esc(g.project)}" data-node="${esc(x.node.id)}"><span class="wk-id">${esc(x.node.id)}</span> ${esc(x.node.title)}</button>
        ${x.op === 'link' ? `<div class="cc-dep">for <button class="cc-node inline" data-tk="open" data-project="${esc(g.project)}" data-node="${esc(x.depends_on)}"><span class="wk-id">${esc(x.depends_on)}</span> ${esc(x.depends_on_title || '')}</button></div>`
          : x.op === 'waiting' && x.what ? `<div class="cc-dep">on ${esc(x.who || 'someone')} — ${esc(x.what)}</div>` : ''}
        ${x.quote ? `<div class="cc-q">“${esc(x.quote)}”${x.speaker ? ` <span>— ${esc(x.speaker)}</span>` : ''}</div>` : ''}</div>
        ${mark}${ts != null ? `<button class="btn ghost" data-at="${ts}">${clock(ts)}</button>` : '<span></span>'}</div>`
    }).join('')}</div>`
  })
  return blocks.length ? `<section class="cc"><div class="cc-h">What this call changed</div>${blocks.join('')}</section>` : ''
}
const hms = (s) => String(s).split(':').reduce((a, x) => a * 60 + Number(x), 0)

// ── Notes: the user's own notes, an editor over my-notes.md (saved by NotesSync) ─────────────
/** `pending`: a note typed in the box, or edited in place, not added to the notes yet. */
const notesState = (v, pending) => pending ? 'Not saved yet' : v.saving ? 'Saving…' : v.error ? `Couldn't save: ${v.error}` : v.dirty ? 'Not saved yet' : 'Saved'
function notesTab(c) {
  if (c.my_notes_error) {
    return `<div class="mynotes"><div class="notice error"><span class="grow">Your notes for this call (my-notes.md) can't be read: ${esc(c.my_notes_error)}. Fix or replace the file, and they show here again.</span><button class="btn ghost" data-cmd="reveal">Show folder</button></div></div>`
  }
  const v = NotesSync.view(c.name)
  const time = c.status === 'recording' ? '<button class="btn ghost" data-cmd="stamp" title="Insert the time into the call">Time</button>' : ''
  const kept = v.kept ? `<div class="notice"><span class="grow">The file was changed outside Echo while you typed: your text is saved, the other version is in ${esc(v.kept)}.</span><button class="btn ghost" data-cmd="kept-ok">OK</button></div>` : ''
  const err = v.error ? `<div class="notice error"><span class="grow">Your notes couldn't be saved: ${esc(v.error)}</span><button class="btn" data-cmd="notes-retry">Retry</button></div>` : ''
  // (The newline after <textarea> is dropped by the parser, so one that starts the text stays.)
  return `<div class="mynotes">
    <p class="hint"><span>Your notes shape the minutes: they're written around them. A line starting with # becomes a section.</span>${time}</p>
    ${kept}${err}
    <textarea id="my-notes" data-notes-call="${esc(c.name)}" aria-label="Your notes" placeholder="Type your notes for this call…"${S.closing ? ' readonly' : ''}>
${esc(v.draft)}</textarea>
    <div class="saved-state" id="notes-state">${esc(notesState(v))}</div>
  </div>`
}

/** My notes, in a panel beside the call (right by default): the open call's, on every tab. */
function notesPanel() {
  const c = S.call
  if (!S.selected) return '<p class="panel-empty">Open a call to take notes for it.</p>'
  if (!c || c.error) return '<div class="skeleton" style="padding:12px"><i></i><i></i></div>'
  if (S.notesRaw || c.my_notes_error) return notesTab(c).replace('<p class="hint">', `<p class="hint"><button class="btn ghost" data-cmd="notes-timeline">← Timeline</button>`)
  return notesTimeline(c)
}

/** Where the call is now, in seconds from its start (only while it records). */
function callNow(c) {
  const t0 = c && c.status === 'recording' && since(c.started_at)
  return t0 ? (Date.now() - t0) / 1000 : null
}
const NOTE_LOOK = {
  note: ['•', 'Note'], question: ['?', 'Question'], decision: ['!', 'Decision'], todo: ['☐', 'To-do'], flag: ['⚑', 'Flagged'],
}

/** My notes as a timeline of moments (`NoteDoc` over the draft NotesSync holds), a box to add one —
 *  timed from the first key while the call records — and Edit as text for the file itself. */
function notesTimeline(c) {
  const v = NotesSync.view(c.name)
  const blocks = NoteDoc.parse(v.draft)
  const items = blocks.map((b, i) => {
    if (!b.note) {
      const raw = b.raw.trim()
      if (!raw) return ''
      const h = raw.match(/^#{1,6}\s+(.*)$/)
      return h ? `<div class="nt-head">${esc(h[1])}</div>` : `<div class="nt-entry k-general"><span class="nt-icon">¶</span><div class="nt-body"><span class="nt-text">${esc(raw)}</span></div></div>`
    }
    const [icon, label] = NOTE_LOOK[b.kind] || NOTE_LOOK.note
    const mark = b.kind === 'todo'
      ? `<button class="nt-icon nt-check" role="checkbox" aria-checked="${b.done}" data-note-done="${i}" aria-label="${b.done ? 'Mark to do' : 'Mark done'}">${b.done ? '☑' : '☐'}</button>`
      : `<span class="nt-icon" title="${label}">${icon}</span>`
    const at = b.t != null ? `<a class="at" data-at="${b.t}" title="Jump to ${clock(b.t)} in the transcript">${esc(NoteDoc.stamp(b.t))}</a> ` : ''
    const quote = b.quote ? `<blockquote>“${esc(b.quote)}”</blockquote>` : ''
    if (S.noteEdit && S.noteEdit.i === i) {
      const kinds = ['note', 'question', 'decision', 'todo', 'flag'].map((k) => `<button class="nt-kind${S.noteEdit.kind === k ? ' on' : ''}" data-edit-kind="${k}">${k === 'note' ? 'Note' : NOTE_LOOK[k][0] + ' ' + NOTE_LOOK[k][1]}</button>`).join('')
      return `<div class="nt-entry editing k-${S.noteEdit.kind}">${mark}<div class="nt-body"><div>${at}</div>
        <textarea id="note-edit" rows="2" aria-label="Edit this note">${esc(S.noteEdit.text)}</textarea>${quote}
        <div class="nt-kinds">${kinds}</div>
        <div class="nt-edit-acts"><span class="grow">Enter to save · Esc to cancel</span><button class="btn ghost" data-cmd="note-edit-cancel">Cancel</button><button class="btn primary" data-cmd="note-edit-save">Save</button></div></div><span></span></div>`
    }
    const text = b.text ? esc(b.text) : (b.kind === 'flag' ? '<span class="muted">Flagged moment</span>' : '')
    return `<div class="nt-entry k-${b.kind}${b.done ? ' done' : ''}">${mark}<div class="nt-body"><div>${at}<span class="nt-text" data-note-edit="${i}" title="Click to edit">${text}</span></div>${quote}</div><span class="nt-tools"><button class="nt-x" data-note-edit="${i}" title="Edit" aria-label="Edit this note">✎</button><button class="nt-x" data-note-del="${i}" title="Delete" aria-label="Delete this note">×</button></span></div>`
  }).join('')
  const empty = blocks.some((b) => b.note || b.raw.trim()) ? '' : `<p class="panel-empty">${c.status === 'recording' ? 'Notes you add here are tied to the moment of the call. Start with ? for a question, ! for a decision, [] for a to-do.' : 'Add notes about this call. Select words in the transcript to comment on what was said.'}</p>`
  const live = c.status === 'recording'
  const kinds = ['note', 'question', 'decision', 'todo'].map((k) => `<button class="nt-kind${S.noteKind === k ? ' on' : ''}" data-note-kind="${k}">${k === 'note' ? 'Note' : `${NOTE_LOOK[k][0]} ${NOTE_LOOK[k][1]}`}</button>`).join('')
  const at = S.noteAt != null ? S.noteAt : callNow(c)
  const ph = at != null ? `Note at ${NoteDoc.stamp(at)} — Enter to add` : 'Add a note — Enter to add'
  const kept = v.kept ? `<div class="notice"><span class="grow">The file was changed outside Echo while you typed: your text is saved, the other version is in ${esc(v.kept)}.</span><button class="btn ghost" data-cmd="kept-ok">OK</button></div>` : ''
  const err = v.error ? `<div class="notice error"><span class="grow">Your notes couldn't be saved: ${esc(v.error)}</span><button class="btn" data-cmd="notes-retry">Retry</button></div>` : ''
  return `<div class="notes-tl" data-notes-call="${esc(c.name)}">
    <div class="nt-bar"><span class="saved-state" id="notes-state">${esc(notesState(v, pendingNote(c.name)))}</span><button class="icon-btn" data-cmd="notes-raw" title="Edit as text (my-notes.md)" aria-label="Edit as text">${ICON('<path d="M8 7l-5 5 5 5M16 7l5 5-5 5"/>')}</button></div>
    ${kept}${err}
    <div class="nt-list" id="nt-list">${items}${empty}</div>
    <div class="nt-compose">
      <div class="nt-kinds">${kinds}</div>
      <div class="nt-row"><input id="note-input" class="search" autocomplete="off" maxlength="1000" placeholder="${esc(ph)}" value="${esc(S.noteDraft)}" aria-label="New note"${S.closing ? ' readonly' : ''}>${live ? `<button class="btn nt-flag" data-cmd="note-flag" title="Flag this moment — the minutes look here">⚑ Flag</button>` : ''}</div>
      ${live ? '<div class="nt-hint">The time is taken when you start typing.</div>' : ''}
    </div>
  </div>`
}

/** Change the open call's notes (a NoteDoc edit of NotesSync's draft) and save them soon. */
function editNotes(f) {
  const call = S.call && S.call.name
  if (!call || S.closing) return
  NotesSync.edit(call, f(NotesSync.view(call).draft))
  saveNotesSoon(call)
  render()
}

/** Save the note edited in place: its text and kind change, its time and quote stay. */
function saveNoteEdit() {
  if (!S.noteEdit || S.closing) return
  commitPendingEdit() // emptied: dropped (delete is the × beside it)
  render()
}

/** Add what is typed in the note box (or a flag) at its moment. */
function addNote(kindOverride) {
  const c = S.call
  if (!c) return
  if (S.closing) return
  if (kindOverride) {
    const t = callNow(c)
    editNotes((text) => NoteDoc.add(text, { kind: kindOverride, t: t == null ? null : Math.floor(t), text: '' }))
  } else {
    commitNoteDraft()
    render()
  }
  const el = $('#note-input'); if (el) el.focus()
}

// A note typed in the box (`S.noteDraft`, its time `S.noteAt`) and one edited in place (`S.noteEdit`)
// belong to the call they were started on (`S.noteDraftCall`, `S.noteEdit.call`). Enter / Save add them;
// so do closing the window, quitting Echo (anything `prepareClose` saves for), Write minutes and opening
// another call — nothing typed is lost. Escape / Cancel drop them.
const sameCall = (a, b) => !!a && !!b && String(a).slice(0, 17) === String(b).slice(0, 17)
/** The call's folder as it is named now (a retitle keeps the stamp). */
const ownCall = (name) => (S.call && sameCall(S.call.name, name) ? S.call.name : name)
/** Whether the call has a note typed or edited that isn't in its notes yet. */
function pendingNote(call) {
  if (S.noteDraft.trim() && sameCall(S.noteDraftCall, call)) return true
  const ed = S.noteEdit
  if (!ed || !sameCall(ed.call, call)) return false
  const draft = NotesSync.view(call).draft, next = NoteDoc.saveEdit(draft, ed)
  return next != null && next !== draft
}
/** Add the note box's text to its own call's notes, as Enter does; the box is emptied once they hold it.
 *  The call whose notes changed, else null. */
function commitNoteDraft() {
  const own = S.noteDraftCall || (S.call && S.call.name)
  if (!own || !S.noteDraft.trim()) return null
  const call = ownCall(own)
  const at = S.noteAt != null ? S.noteAt : S.call && sameCall(S.call.name, call) ? callNow(S.call) : null
  const next = NoteDoc.addTyped(NotesSync.view(call).draft, { text: S.noteDraft, kind: S.noteKind, at })
  if (next == null) return null
  NotesSync.edit(call, next)
  S.noteDraft = ''; S.noteAt = null; S.noteDraftCall = null
  if (!S.closing) saveNotesSoon(call)
  return call
}
/** Everything typed and not added yet, added to its own call's notes and saved now (no waiting for the
 *  1 s timer): before the window closes or Echo quits, before Write minutes, when another call opens. */
function commitPendingNotes() {
  const changed = new Set([commitPendingEdit(), commitNoteDraft()].filter(Boolean))
  for (const call of changed) NotesSync.save(call)
  return changed.size > 0
}
/** The note edited in place, saved into its own call's notes as Save does (an emptied one is dropped,
 *  as Save drops it). The call whose notes changed, else null. */
function commitPendingEdit() {
  const ed = S.noteEdit
  const call = ed && ownCall(ed.call || (S.call && S.call.name))
  if (!call) return null
  const draft = NotesSync.view(call).draft, next = NoteDoc.saveEdit(draft, ed)
  S.noteEdit = null
  if (next == null || next === draft) return null
  NotesSync.edit(call, next)
  if (!S.closing) saveNotesSoon(call)
  return call
}
PANEL_VIEWS.push({ id: 'notes', label: 'My notes', icon: ICON('<path d="M5 4h10l4 4v12H5z"/><path d="M15 4v4h4M8 12h8M8 16h6"/>'), area: 'right', when: () => S.view === 'calls', render: notesPanel })

/** Also discussed (the Brain): for each moment of the open call, the nearest moments elsewhere — the
 *  same client's first (another client's only when very close) — each opening at its second. */
const also = { name: null, data: null, at: 0 }
async function loadAlso(name) {
  if (also.name === name && Date.now() - also.at < 60000) return
  also.name = name; also.at = Date.now()
  try { also.data = await get('api/brain/also/' + encodeURIComponent(name)) } catch { also.data = { error: 'The Brain isn\'t available' } }
  render()
}
function alsoPanel() {
  if (!S.selected) return '<p class="panel-empty">Open a call.</p>'
  if (also.name !== S.selected) { loadAlso(S.selected); return '<div class="skeleton" style="padding:12px"><i></i><i></i></div>' }
  const d = also.data
  if (!d || d.error) return `<p class="panel-empty">${esc((d && d.error) || '')}</p>`
  const chips = (d.topics || []).map((t) => `<span class="chipt">${esc(t)}</span>`).join('')
  const groups = (d.groups || []).map((g) => `<div class="al-g"><div class="al-a">for ${esc(g.anchor)}</div>${g.related.map((r) => {
    const call = r.link && r.link.startsWith('echo://call/') ? r.link.slice(12).split('?')[0] : ''
    const t = r.link && r.link.includes('?t=') ? Number(r.link.split('?t=')[1]) : null
    return `<div class="al-r"><button class="src" ${call ? `data-br-open="${esc(call)}"${t != null ? ` data-t="${t}"` : ''}` : extAttr(r.link)}>${kindMark(r)} ${esc(r.doc || r.source)}${r.date ? ` · ${esc(relDay(r.date))}` : ''}${t != null ? ` · ${clock(t)}` : ''}</button><div class="qt">${esc(r.text)}</div></div>`
  }).join('')}</div>`).join('')
  return `<div class="al">${chips ? `<div class="al-chips">${chips}</div>` : ''}${groups || '<p class="set-sub">Nothing close elsewhere yet — it fills as more calls (and, later, channels, mail and files) come into the Brain.</p>'}</div>`
}
PANEL_VIEWS.push({ id: 'also', label: 'Also discussed', icon: ICON('<circle cx="7" cy="12" r="3"/><circle cx="17" cy="6" r="3"/><circle cx="17" cy="18" r="3"/><path d="M9.6 10.7 14.4 7.4M9.6 13.3l4.8 3.3"/>'), area: 'right', when: () => brainOn() && S.view === 'calls' && !!S.selected, render: alsoPanel })
// A button that sends one IPC command as it is (`data-cmd-ipc`), then reads the settings again.
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-cmd-ipc]')
  if (!b) return
  b.disabled = true
  send({ cmd: b.dataset.cmdIpc })
  setTimeout(() => loadSettings().then(render), 8000)
})
/** Inbox (`attention.rs`): what waits for the user's decision, from everywhere, in one list — the
 *  activity bar's entry above Settings shows or hides it. Each item is decided here with the same
 *  message its own place sends, or opens that place; decided anywhere, it moves to Done this week. */
const NY = { data: null, at: 0, filter: 'open', busy: new Map(), last: null, loading: null, failed: 0, seq: 0, more: new Set() }
/** How long a decision shows as working with no word from the app: a quick one is gone by then. */
const NY_BUSY_MS = 15000
/** One request at a time; a fresh one (after a decision) wins over any older answer still on its way. */
function loadAttention(fresh) {
  if (NY.loading && !fresh) return NY.loading
  const n = ++NY.seq
  NY.loading = get('api/attention' + (fresh ? '?fresh=1' : '')).then((d) => {
    if (n !== NY.seq) return
    NY.data = d; NY.at = Date.now(); NY.failed = 0
    // A decision still working stays so until its item is gone; the app names the long ones (`busy`).
    const open = new Set((d.open || []).map((i) => i.key))
    // A job that failed gives the buttons back at once, with why (`failed`).
    for (const [k, at] of NY.busy) if (!open.has(k) || (d.failed || {})[k] || (!(d.busy || {})[k] && Date.now() - at > NY_BUSY_MS)) NY.busy.delete(k)
  }).catch(() => { NY.failed = Date.now() }).finally(() => { if (n === NY.seq) NY.loading = null })
  return NY.loading
}
/** Show the list (from ⌘K): over the page in a narrow window, as My notes does. */
function nyShow() {
  const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (eff.narrow && Layout.area('needs', known) === 'right') S.overlay = true
  Layout.show('needs', known)
  layoutChanged()
}
/** Its shortcut: ⇧⌘I on macOS; Ctrl+Shift+U elsewhere (Ctrl+Shift+I opens WebView2's DevTools). */
const NY_KEY = MAC ? 'I' : 'U'
const NY_HINT = MAC ? '⇧⌘I' : 'Ctrl+Shift+U'
/** At most this many of one kind in a group; the rest are counted, opening their place. */
const NY_CAP = 5
const nyOpen = () => (NY.data || {}).open || []
/** What an item is doing now, or '' : the app's word for a background job, else the user's click a moment ago. */
const nyBusy = (key) => ((NY.data || {}).busy || {})[key] || (NY.busy.has(key) ? 'Working…' : '')
const NY_ICON = {
  signin: ICON('<circle cx="12" cy="8" r="3.5"/><path d="M5 20c1.2-3.6 4-5.5 7-5.5s5.8 1.9 7 5.5"/>'),
  minutes: ICON('<path d="M6 3.5h9l4 4v13H6z"/><path d="M9 12h7M9 16h5"/>'),
  sender: ICON('<rect x="3.5" y="5.5" width="17" height="13" rx="2"/><path d="M4 7l8 6 8-6"/>'),
  people: ICON('<circle cx="9" cy="9" r="3"/><circle cx="16" cy="9" r="3"/><path d="M3.5 19c.9-2.8 3-4.3 5.5-4.3M20.5 19c-.9-2.8-3-4.3-5.5-4.3"/>'),
  work: ICON('<rect x="4" y="4" width="16" height="16" rx="3.5"/><path d="M8.5 12.2l2.4 2.4 4.6-4.9"/>'),
  found: ICON('<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/><path d="M12 11v5M9.5 13.5h5"/>'),
  brain: ICON('<path d="M9 4.5a3 3 0 0 0-3 3 3 3 0 0 0-2 5.2A3.2 3.2 0 0 0 7.5 18 2.5 2.5 0 0 0 12 19V6a2 2 0 0 0-3-1.5z"/><path d="M15 4.5a3 3 0 0 1 3 3 3 3 0 0 1 2 5.2A3.2 3.2 0 0 1 16.5 18 2.5 2.5 0 0 1 12 19"/>'),
  health: ICON('<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17v.01"/>'),
  update: ICON('<path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14"/>'),
}
const NY_ENTRY = ICON('<path d="M4 13l3-8h10l3 8v6H4z"/><path d="M4 13h5l1 2h4l1-2h5"/>')
function needsPanel() {
  const d = NY.data
  if (!d) {
    if (NY.failed) return '<p class="panel-empty">The list can\'t be read right now. Echo tries again shortly.</p>'
    if (!NY.loading) loadAttention().then(render)
    return '<div class="skeleton"><i></i><i></i></div>'
  }
  const open = nyOpen(), urgent = open.filter((i) => i.urgent), done = d.done || []
  const chip = (f, label, n) => `<button class="fchip${NY.filter === f ? ' on' : ''}" data-ny-filter="${f}">${label}${n ? ` ${n}` : ''}</button>`
  const head = `<div class="ny-head">${chip('open', 'Open', open.length)}${urgent.length ? chip('blocking', 'Blocking', urgent.length) : ''}${chip('done', 'Done', done.length)}<span class="grow"></span><span class="set-sub">this week</span></div>`
  const row = (i) => {
    const busy = nyBusy(i.key)
    const acts = busy ? `<span class="ny-busy" role="status"><i></i>${esc(busy)}</span>` : (i.actions || []).map((a, k) => `<button class="btn${a.primary ? (i.urgent ? ' warn' : ' primary') : ''}" data-ny-act="${esc(i.key)}" data-k="${k}">${esc(a.label)}</button>`).join('')
    const go = i.go ? `<button class="btn ghost" data-ny-go="${esc(i.go)}">${(i.actions || []).length ? 'Open' : 'Open →'}</button>` : ''
    const failed = !busy && ((NY.data || {}).failed || {})[i.key]
    return `<div class="ny-item${i.urgent ? ' urgent' : ''}${busy ? ' busy' : ''}"><span class="ny-k">${NY_ICON[i.kind] || NY_ICON.work}</span><b>${esc(i.title)}</b>${i.why ? `<span class="ny-why">${esc(i.why)}</span>` : ''}${failed ? `<span class="ny-why ny-err">${esc(failed)}</span>` : ''}${acts || go ? `<div class="ny-acts">${acts}${go}</div>` : ''}</div>`
  }
  const groupName = (g) => g === '-' || g === 'echo' ? 'Everything else' : (d.clients || {})[g] || g
  const groups = (items) => {
    const by = new Map()
    for (const i of items) { const g = i.group === 'echo' ? '-' : i.group; if (!by.has(g)) by.set(g, []); by.get(g).push(i) }
    const keys = [...by.keys()].sort((a, b) => (a === '-') - (b === '-') || groupName(a).localeCompare(groupName(b)))
    const capped = (g, list) => {
      const kinds = new Map()
      for (const i of list) { if (!kinds.has(i.kind)) kinds.set(i.kind, []); kinds.get(i.kind).push(i) }
      return [...kinds.entries()].map(([k, xs]) => {
        const all = NY.more.has(g + ':' + k), shown = all ? xs : xs.slice(0, NY_CAP), rest = xs.length - shown.length
        return shown.map(row).join('') + (rest ? `<button class="btn ghost ny-more" data-ny-more="${esc(g + ':' + k)}">${rest} more like these</button>` : '')
      }).join('')
    }
    return keys.map((g) => `<div class="ny-grp"><span class="cap">${esc(groupName(g))}</span><span class="set-sub">${by.get(g).length}</span></div>${capped(g, by.get(g))}`).join('')
  }
  const doneRow = (x) => `<div class="ny-done"><span class="ok">✓</span><s>${esc(x.title || x.key)}</s><span class="grow"></span><span class="set-sub">${esc(relDay(String(x.at || '').slice(0, 10)))}</span></div>`
  let body
  if (NY.filter === 'done') body = done.length ? done.map(doneRow).join('') : '<p class="panel-empty">Nothing decided this week yet.</p>'
  else if (NY.filter === 'blocking') body = urgent.map(row).join('') || '<p class="panel-empty">Nothing blocks work.</p>'
  else if (!open.length) body = `<div class="ny-empty"><span class="ok">✓</span><div><b>Nothing needs you</b><div class="set-sub">Echo counts it here when something does.</div></div></div>${done.length ? `<div class="ny-grp"><span class="cap">Done this week</span><span class="set-sub">${done.length}</span></div>${done.slice(0, 3).map(doneRow).join('')}` : ''}`
  else body = urgent.map(row).join('') + groups(open.filter((i) => !i.urgent)) + (done.length ? `<div class="ny-grp ny-sep"><span class="ok">✓</span><span class="cap">Done this week</span><span class="set-sub">${done.length}</span><span class="grow"></span><button class="btn ghost" data-ny-filter="done">All ▸</button></div>${done.slice(0, 2).map(doneRow).join('')}` : '')
  return `<div class="ny">${head}${body}</div>`
}
PANEL_VIEWS.push({ id: 'needs', label: 'Inbox', icon: NY_ENTRY, area: 'right', render: needsPanel })
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-ny-filter],[data-ny-act],[data-ny-go],[data-ny-more]')
  if (!t) return
  if (t.dataset.nyFilter) { NY.filter = t.dataset.nyFilter; return render() }
  if (t.dataset.nyMore) { NY.more.add(t.dataset.nyMore); return render() }
  if (t.dataset.nyGo) return go(t.dataset.nyGo)
  const i = nyOpen().find((x) => x.key === t.dataset.nyAct)
  const a = i && (i.actions || [])[Number(t.dataset.k)]
  if (!a) return
  // Change…: decided where it can be edited (a suggestion's own version in the People tab).
  if (a.go) { if (a.edit) S.pj.ppSugEdit = a.edit; return go(a.go) }
  send(a.ipc)
  // A decision shows as working until its item goes (a background job: until it ends); an action
  // that opens a flow (a sign-in, a log) leaves it as it was.
  if (!['sign_in', 'show_log', 'settings_action', 'write_notes'].includes(a.ipc.cmd)) { NY.busy.set(i.key, Date.now()); NY.last = { key: i.key, at: Date.now() } }
  render()
  setTimeout(async () => { await loadAttention(true); await loadProjects(); render() }, 700)
})
/** The panel views of the current activity. */
const panelViews = () => PANEL_VIEWS.filter((v) => !v.when || v.when())

// ── terminals (`terminal.rs`): each session a pseudo-terminal in the app, shown with xterm.js ───
// The app owns the program (closing the window keeps it; quitting Echo ends it). Here each has
// an element made once and kept: render() draws a slot (`data-term`), and the element moves into
// it — out of sight, it waits in a hidden holder, so a re-render, a tab or a panel move never
// loses the scrollback. Output comes by a long poll of `api/pty/<id>?from=<byte>` (all of it after
// a window reopens: the app keeps 2 MB); keys and size go back over IPC.
const Terms = new Map() // key → {id, term, fit, el, from, exited, polling, cols, rows}
const termTheme = () => {
  const css = getComputedStyle(document.documentElement)
  const v = (n) => css.getPropertyValue(n).trim()
  return { background: v('--surface') || '#1b1b21', foreground: v('--text') || '#ececf1', cursor: v('--accent') || '#9b78ff', selectionBackground: v('--accent-soft') || 'rgba(155,120,255,.25)' }
}
function termHolder() {
  let h = $('#term-holder')
  if (!h) { h = document.createElement('div'); h.id = 'term-holder'; h.hidden = true; document.body.appendChild(h) }
  return h
}
/** The element of a terminal (made on first use, then kept). */
function termEntry(key) {
  let e = Terms.get(key)
  if (e) return e
  if (!window.Terminal) return null
  const el = document.createElement('div')
  el.className = 'term'
  const term = new window.Terminal({ fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--mono') || 'Menlo, monospace', fontSize: 12.5, cursorBlink: true, allowProposedApi: false, theme: termTheme(), scrollback: 5000 })
  const fit = new window.FitAddon.FitAddon()
  term.loadAddon(fit)
  term.open(el)
  e = { id: null, term, fit, el, from: 0, exited: false, polling: false, cols: 0, rows: 0 }
  term.onData((d) => { if (e.id != null && !e.exited) send({ cmd: 'pty_input', id: e.id, data: d }) })
  term.attachCustomKeyEventHandler((k) => termKey(e, k))
  Terms.set(key, e)
  termHolder().appendChild(el)
  return e
}
/** Keys xterm.js would get wrong here (false = handled). macOS: the window has no Edit menu, so
 *  ⌘C / ⌘V / ⌘A never reach the webview's own copy and paste — copy the selection, paste through
 *  the app (it reads the clipboard; an image there goes on as Ctrl+V, which Claude Code reads).
 *  Ctrl+Enter and Shift+Enter: a new line in Claude Code's prompt (Option+Enter's ESC CR), not Enter. */
function termKey(e, k) {
  const live = e.id != null && !e.exited
  const cmd = MAC && k.metaKey && !k.ctrlKey && !k.altKey && !k.shiftKey ? k.key.toLowerCase() : ''
  if (cmd === 'c' || cmd === 'v' || cmd === 'a') {
    k.preventDefault()
    if (k.type !== 'keydown') return false
    if (cmd === 'c' && e.term.hasSelection()) send({ cmd: 'copy', text: e.term.getSelection() })
    if (cmd === 'v' && live) send({ cmd: 'pty_paste', id: e.id, bracketed: !!e.term.modes.bracketedPasteMode })
    if (cmd === 'a') e.term.selectAll()
    return false
  }
  if (k.key === 'Enter' && (k.ctrlKey || k.shiftKey) && !k.metaKey && !k.altKey) {
    k.preventDefault()
    if (k.type === 'keydown' && live) send({ cmd: 'pty_input', id: e.id, data: '\x1b\r' })
    return false
  }
  return true
}
/** Follow a terminal's output until its program ends. */
async function termPoll(e) {
  if (e.polling) return
  e.polling = true
  const bytes = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  while (e.id != null) {
    const id = e.id
    let r
    try { r = await get(`api/pty/${id}?from=${e.from}`) } catch { if (e.id === id) { e.exited = true; render() } break }
    if (e.id !== id) break
    if (r.reset) e.term.reset()
    if (r.data) e.term.write(bytes(r.data))
    e.from = r.next
    if (r.exited && !r.data) {
      if (!e.exited) { e.exited = true; try { S.sessions = new Set(await get('api/sessions')) } catch { /* as it was */ } render() }
      break
    }
  }
  e.polling = false
}
/** Fit a shown terminal to its slot, and tell the app its size. */
function termFit(e) {
  if (!e.el.isConnected || e.el.closest('#term-holder')) return
  try { e.fit.fit() } catch { return }
  if (e.id != null && (e.term.cols !== e.cols || e.term.rows !== e.rows)) {
    e.cols = e.term.cols; e.rows = e.term.rows
    send({ cmd: 'pty_resize', id: e.id, cols: e.cols, rows: e.rows })
  }
}
/** After a render: each terminal in its slot, the others in the holder. */
function attachTerms(focusKey) {
  const shown = new Set()
  for (const slot of document.querySelectorAll('.term-slot[data-term]')) {
    const e = Terms.get(slot.dataset.term)
    if (!e) continue
    shown.add(slot.dataset.term)
    if (e.el.parentElement !== slot) slot.appendChild(e.el)
    termFit(e)
  }
  for (const [k, e] of Terms) if (!shown.has(k) && e.el.parentElement !== termHolder()) termHolder().appendChild(e.el)
  if (focusKey && shown.has(focusKey)) Terms.get(focusKey).term.focus()
}
const focusedTerm = () => { for (const [k, e] of Terms) if (e.el.contains(document.activeElement)) return k; return null }
/** Start a terminal of `kind` for `key` (the app answers with `ptyOpened`). */
function termStart(kind, key, extra = {}) {
  const e = termEntry(key)
  if (!e) { S.notice = "The terminal can't be shown (xterm.js didn't load)."; return render() }
  e.exited = false; e.starting = true; e.signin = null; e.error = null; e.from = 0; e.term.reset()
  if (S.termOutput) delete S.termOutput[key]
  if (kind === 'claude-call') for (const [k, o] of Terms) if (k !== key && k.startsWith('claude:call:') && o.id != null) { o.id = null; o.exited = true } // one call session at a time
  const slot = document.querySelector(`.term-slot[data-term="${CSS.escape(key)}"]`)
  const dims = slot ? (() => { slot.appendChild(e.el); try { e.fit.fit() } catch {} return { cols: e.term.cols, rows: e.term.rows } })() : { cols: 100, rows: 30 }
  send({ cmd: 'pty_open', kind, key, ...extra, ...dims })
  render()
}
/** Terminals the app already runs (the window reopened): shown again, scrollback and all. */
async function loadTerms() {
  try { S.sessions = new Set(await get('api/sessions')) } catch { /* none known */ }
  let list = []
  try { list = await get('api/ptys') } catch { return }
  for (const t of list) {
    const e = termEntry(t.key)
    if (!e || e.id === t.id) continue
    e.id = t.id; e.from = 0; e.exited = t.exited
    termPoll(e)
  }
}
new ResizeObserver(() => { for (const e of Terms.values()) termFit(e) }).observe(document.documentElement)

/** A terminal panel view: the slot, or what starts it. */
function termView(key, kind, startLabel, intro = '', extra = {}) {
  const e = Terms.get(key)
  const running = e && e.id != null && !e.exited
  const attrs = `data-term-start="${esc(kind)}" data-term-key="${esc(key)}"${extra.call ? ` data-term-call="${esc(extra.call)}"` : ''}`
  // Ended (or couldn't start): a card like the one before starting — what happened, and Resume —
  // not an empty terminal. What it printed stays one click away (an error is worth reading).
  if (e && e.exited && !(S.termOutput || {})[key]) {
    const why = e.error ? `<p class="term-error">${esc(e.error)}</p>` : ''
    const msg = e.signin ? 'This profile needs a sign-in before Claude can start.' : e.error ? "Claude couldn't start." : 'The session ended. Resume picks up the conversation where it stopped.'
    const signin = e.signin ? `<button class="btn primary" data-signin="${esc(e.signin)}">Sign in…</button>` : ''
    return `<div class="term-empty"><p>${msg}</p>${why}<div class="term-acts">${signin}<button class="btn${e.signin ? '' : ' primary'}" ${attrs}>${e.error || e.signin ? 'Try again' : 'Resume'}</button>${e.error ? '' : `<button class="btn ghost" data-term-output="${esc(key)}">Show output</button>`}</div></div><div class="term-slot" data-term="${esc(key)}" hidden></div>`
  }
  if (!running && !(e && (e.exited || e.starting))) {
    return `<div class="term-empty">${intro}<button class="btn primary" ${attrs}>${esc(startLabel)}</button></div><div class="term-slot" data-term="${esc(key)}" hidden></div>`
  }
  const head = e && e.exited ? `<div class="term-bar"><span class="grow">The session ended — its output.</span><button class="btn ghost" data-term-output="${esc(key)}">Hide</button><button class="btn" ${attrs}>Resume</button></div>`
    : e && e.starting ? '<div class="term-bar"><span class="grow">Starting Claude Code…</span></div>' : ''
  return `<div class="term-wrap">${head}<div class="term-slot" data-term="${esc(key)}"></div></div>`
}
// ── Claude in the window: two sessions (terminal.rs). The call's is guarded and on the call's own
// AI profile; the folder's is the user's own Claude Code on the profile they pick.
const stampOf = (name) => String(name || '').slice(0, 17)
const callKey = (name) => `claude:call:${stampOf(name)}`
/** The call a running call session is on (one at a time), if any. */
function callSessionOn() {
  for (const [k, e] of Terms) if (k.startsWith('claude:call:') && e.id != null && !e.exited) return k.slice('claude:call:'.length)
  return null
}
function claudeCallView() {
  const c = S.call
  if (!S.selected || !c || c.error) return '<p class="panel-empty">Open a call to ask Claude about it.</p>'
  const key = callKey(c.name), p = profileOf(c.live && c.live.profile)
  const pname = p ? p.name : (c.live && c.live.profile) || 'the call\'s profile'
  const banner = `<div class="term-banner"><b>Guarded</b> · reads this call${c.status === 'recording' ? ' (the transcript so far)' : ''} and the earlier minutes of this meeting · can't change or send anything · ${esc(pname)}, this call's profile</div>`
  const on = callSessionOn()
  const elsewhere = on && on !== stampOf(c.name)
    ? `<div class="term-bar"><span class="grow">Claude is on ${esc(titleOf(((S.calls || []).find((x) => stampOf(x.name) === on) || {}).name || on))}.</span></div>` : ''
  const intro = `${elsewhere}<p>Ask Claude about <b>${esc(c.title)}</b>: what was decided, who asked for what, what you missed. It reads the transcript, chat, minutes and your notes of this call and the earlier minutes of the same meeting — it can't change anything.</p><p class="hint">On ${esc(pname)}, the profile this call was recorded with.</p>`
  // A conversation about this call on its profile that can go on: Resume, not a new one.
  const saved = S.sessions && S.sessions.has(`${(c.live && c.live.profile) || 'system'}:${stampOf(c.name)}`)
  return banner + termView(key, 'claude-call', saved ? 'Resume the conversation about this call' : on && on !== stampOf(c.name) ? 'Open Claude on this call' : 'Ask Claude about this call', intro, { call: c.name })
}
function claudeFolderView() {
  const a = ai()
  const pick = a.profiles.length > 1 ? `<label class="term-profile">Profile <select data-term-profile>${profileOptions(S.folderProfile || a.default)}</select></label>` : ''
  const intro = `<p>Your own Claude Code in the Echo data folder — your settings and permissions, every call's files at hand.</p>${pick}`
  const saved = S.sessions && S.sessions.has(`${S.folderProfile || a.default || 'system'}:folder`)
  return termView('claude:folder', 'claude-folder', saved ? 'Resume Claude' : 'Start Claude', intro, {})
}
// Claude's sparkle, in a speech bubble (this call) or on a folder (the Echo folder).
const SPARK = (x, y, r) => `<path d="M${x} ${y - r}l${r * 0.3} ${r * 0.7} ${r * 0.7} ${r * 0.3}-${r * 0.7} ${r * 0.3}-${r * 0.3} ${r * 0.7}-${r * 0.3}-${r * 0.7}-${r * 0.7}-${r * 0.3} ${r * 0.7}-${r * 0.3}z" fill="currentColor" stroke="none"/>`
PANEL_VIEWS.push({ id: 'claude-call', label: 'Claude · this call', icon: ICON(`<path d="M4 5h16v11H10l-5 4v-4H4z"/>${SPARK(12, 10.5, 4)}`), area: 'bottom', when: () => S.view === 'calls' && !!S.selected, render: claudeCallView })
PANEL_VIEWS.push({ id: 'claude-folder', label: 'Claude · Echo folder', icon: ICON(`<path d="M3 6h6l2 2h10v11H3z"/>${SPARK(12, 13.5, 3.6)}`), area: 'bottom', when: () => S.view !== 'settings', render: claudeFolderView })

// ── Minutes: the page as it would be sent, in a sandboxed frame (`/minutes/<call>`) ───────────
const SHOW_FILE = /Windows/.test(navigator.userAgent) ? 'Show in Explorer' : 'Show in Finder'
function minutesTab(c) {
  const run = c.notes_run
  const busy = run && (run.state === 'queued' || run.state === 'running')
  // The first minutes being written: their box until they are done (the file appears mid-run).
  if (!c.notes || (busy && !run.had_minutes)) return minutesHint(c, 'No minutes yet', 'The transcript is ready.')
  const frame = `<iframe id="minutes-frame" sandbox="allow-scripts" referrerpolicy="no-referrer" title="Minutes" src="minutes/${encodeURIComponent(c.name)}?v=${encodeURIComponent(c.minutes_rev || '')}"></iframe>`
  return { bar: minutesBar(c), frame }
}

/** Above the page: an update going on or one that didn't finish, notes changed since, and the file. */
/** Write now: a run waiting for another call's recording or transcript that may start anyway
 *  (`may_start`: Claude Code as configured, not on a model on this computer). */
const notesNowBtn = (run) => {
  return run && run.state === 'queued' && run.wait === 'call' && run.may_start && !run.now ? '<button class="btn ghost" data-cmd="notes-now" title="Start them now; the other call keeps recording, and both share this computer">Write now</button>' : ''
}
function minutesBar(c) {
  const run = c.notes_run
  const log = c.has_log ? '<button class="btn ghost" data-cmd="show-log">Show log</button>' : ''
  let status = ''
  if (run && run.state === 'running') status = `<span class="grow">Updating the minutes…</span>${log}`
  else if (run && run.state === 'queued') status = `<span class="grow">${esc(run.message || 'The update waits its turn.')}</span>${notesNowBtn(run)}`
  else if (run && (run.state === 'failed' || run.state === 'needs_login')) {
    const p = run.state === 'needs_login' && profileOf(run.profile)
    const signin = p && p.kind === 'claude' ? `<button class="btn ghost" data-signin="${esc(p.id)}">Sign in…</button>` : ''
    status = `<span class="grow warn">The last update didn't finish: ${esc(run.message || '')}</span>${signin}<button class="btn ghost" data-cmd="write-notes">Try again</button>${log}`
  } else if (run && run.state === 'skipped') status = `<span class="grow">${esc(run.message || 'The last update wrote nothing.')}</span>`
  // Opening or showing the page is in the call's ⋯ menu: this bar is only for what is going on.
  return status
}

/** No minutes (or no action items) yet: when Echo writes them by itself, or why not, and Write
 *  minutes — Claude Code run from the app (the same process-call skill a session runs); the run's
 *  state and log come with the call. */
function minutesHint(c, heading, lead) {
  const n = (S.state && S.state.notes) || {}
  const run = c.notes_run
  const auto = c.auto || {}
  const cmd = `/echo:process-call ${c.name}`
  const log = c.has_log ? '<button class="btn ghost" data-cmd="show-log">Show log</button>' : ''
  const box = (h, p, buttons, busy) => `<div class="empty" style="height:auto;padding:40px 0"><div>${mark(busy ? 'busy' : '')}<h3>${esc(h)}</h3>
    ${p}${buttons ? `<div class="notes-actions">${buttons}</div>` : ''}</div></div>`
  if (run && run.state === 'queued') {
    return box('Minutes are waiting their turn', `<p>${esc(run.message || 'They start as soon as the model is free.')}</p><p class="alt">With <b>${esc(run.model || '')}</b>. You can close this window.</p>`, notesNowBtn(run), true)
  }
  if (run && run.state === 'running') {
    return box('Writing minutes…', `<p>Claude Code is writing the minutes and action items with <b>${esc(run.model || n.model || 'sonnet')}</b>. It takes a few minutes; you can close this window.</p>`, log, true)
  }
  if (c.status === 'recording' || c.status === 'transcribing') {
    // Promised only when they are due; whether Claude Code is there only once the state says so.
    const known = !!(S.state && S.state.notes)
    const when = known && !n.claude ? "Minutes are written by Claude Code, which isn't installed on this computer: install it from claude.com/claude-code to get them."
      : known && !n.plugin ? "Echo's plugin for Claude Code wasn't found: menu → Claude Code → Connect Claude Code, then write them once the call is transcribed."
      : auto.due === 'error' ? `Automatic minutes can't start${auto.reason ? `: ${auto.reason.replace(/^(the )?automatic minutes (can't|couldn't) start: /i, '')}` : ''}. Write them once the call is transcribed.`
      : auto.due === 'not_ready' && !ai().error ? 'Echo writes them 15 minutes after the call. Add your own notes in the Notes tab meanwhile: the minutes are written around them.'
      : 'Write them once the call is transcribed.'
    return box('Minutes come after the call', `<p>${esc(when)}</p>`, '')
  }
  if (!run && auto.due === 'now') return box('Starting the minutes…', '<p>Echo is starting them now.</p>', '', true)
  // A run refused for a Claude profile's sign-in (the built-in profile signs in in Terminal), and
  // that profile's sign-in now: signed in since (or a model refused it although it looks signed in).
  const signinOf = run && run.state === 'needs_login' && [profileOf(run.profile)].find((x) => x && x.kind === 'claude')
  const now = signinOf && signinOf.signin
  const again = now && now.state === 'signed_in'
  const why = {
    needs_login: again ? ['Ready to try again', run.message ? `The last try said: ${run.message}` : '']
      : [now && now.state === 'other_account' ? 'These minutes need another account' : 'Claude Code needs you to sign in', run && run.message],
    failed: ["Couldn't write the minutes", run && run.message],
    skipped: ['No minutes for this call', run && run.message],
  }[run && run.state]
  let label = 'Write minutes'
  if (why) [heading, lead, label] = [why[0], why[1] || lead, run.state !== 'skipped' ? 'Try again' : label]
  else if (auto.due === 'at' && auto.at) [heading, lead, label] = [`Minutes at ${hm(new Date(auto.at))}`, '15 minutes after the call, or after your last note — your notes in the Notes tab shape them.', 'Write minutes now']
  else if (auto.due === 'gate') [heading, lead] = ['No minutes written automatically', `The call is too thin for them (${auto.reason || 'too short'}). Write them anyway if you want them.`]
  else if (auto.due === 'error') [heading, lead] = ["Automatic minutes can't start", (auto.reason || "Echo's settings can't be read").replace(/^(the )?automatic minutes (can't|couldn't) start: /i, '')]
  const session = `<p class="alt">Or in a Claude Code session: <code>${esc(cmd)}</code> <button class="btn ghost" data-copy="${esc(cmd)}">Copy</button></p>`
  if (!n.claude) {
    return box(heading, `<p>${esc(lead)}</p><p>Minutes and action items are written by Claude Code. Install it from <a href="https://claude.com/claude-code" data-ext="1">claude.com/claude-code</a>, then come back here.</p>`, log)
  }
  if (!n.plugin) {
    return box(heading, `<p>${esc(lead)}</p><p>Connect Claude Code first: menu → Claude Code → Connect Claude Code.</p>`, log)
  }
  // The call's own profile; none when it can't be read (the run would fail: say why, not the default).
  const live = c.live || {}, picked = S.notesProfile && S.notesProfile.call === c.name
  const pid = picked ? S.notesProfile.id : (live.profile || (live.profile_error ? '' : ai().default))
  const p = profileOf(pid)
  // The model that writes them, as Echo decides it (`Profile::notes_model`), for every kind.
  const model = (p && p.notes_model_label) || n.model || 'sonnet'
  // The profile these minutes need signed in: the call's profile now (picked here or in the Live
  // tab); the one the last run was refused for only while it still is (or none can be read).
  // Signed in now: Try again (or sign in again, if a model keeps refusing it).
  const target = signinOf && (!p || p.id === signinOf.id) ? signinOf : p
  const sign = signinNote(target) || (again && target === signinOf
    ? `<div class="signin"><span class="signin-text">${esc(signinOf.name)} is signed in as ${esc(now.account)} now — click Try again.</span><span class="signin-acts"><button class="btn ghost" data-signin="${esc(signinOf.id)}">Sign in again…</button></span></div>` : '')
  const where = ai().error ? `The AI settings can't be read: ${esc(ai().error)}`
    : !p && !picked && live.profile_error ? `This call's model can't be read: ${esc(live.profile_error)}`
    : !p ? (pid === '!unreadable' ? 'No model could be chosen when this call started — pick one.' : `The model this call was set to (${esc(pid || '?')}) no longer exists — pick another.`)
    : p.kind !== 'system'
      ? `Claude Code writes them with <b>${esc(p.name)}</b> (${esc(model)}), ${esc(profileWhere(p))}.`
      : `Claude Code writes them with <b>${esc(model)}</b>, as it is set up (menu → AI for calls → Minutes model, while it is the profile new calls use).`
  const pick = ai().profiles.length > 1 || (ai().profiles.length && !p) ? `<label class="notes-pick">Model <select data-notesprofile aria-label="Model for these minutes">${p ? '' : '<option value="" selected>Choose…</option>'}${profileOptions(pid)}</select></label>` : ''
  return box(heading, `<p>${esc(lead)}</p><p class="alt">${where}</p>${sign}`,
    `${pick}<button class="btn primary" data-cmd="write-notes">${label}</button>${log}`) + session
}

// ── Live tab: live notes, questions, last time (written by the app, `livenotes.rs`) ────────
/** A call has a Live tab when it has live notes or answers, or while it records or is transcribed with live notes on
 * (or the AI settings unreadable). */
function hasLive(c) {
  const l = c.live || {}, st = l.status || {}
  if (l.notes || (l.answers && l.answers.length)) return true // what was written stays readable
  return (!!st.enabled || !!st.settings_error) && (c.status === 'recording' || c.status === 'transcribing')
}

const LIVE_STATE = {
  waiting: 'Waiting for enough to summarise',
  updating: 'Updating…',
  updated: 'Updated',
  failed: 'The last update failed',
  busy: 'Waiting for the model to finish another job',
  signing_in: 'Waiting for the sign-in',
  paused: 'Paused',
  ended: 'The call ended',
}

function ago(iso) {
  if (!iso) return ''
  const s = Math.max(0, Math.round((Date.now() - new Date(iso)) / 1000))
  return s < 60 ? `${s} s ago` : `${Math.round(s / 60)} min ago`
}

const atLink = (t) => typeof t === 'number' ? ` <a class="at" data-at="${t}" title="Jump to ${clock(t)} in the transcript">${clock(t)}</a>` : ''

function liveHead(c, find = '') {
  const l = c.live || {}, st = l.status || {}, n = l.notes
  const state = st.paused ? 'paused' : st.state || (c.status === 'recording' ? 'waiting' : 'ended')
  let line = LIVE_STATE[state] || ''
  if (state === 'updated' && n && n.updated_at) line = `Updated ${ago(n.updated_at)}`
  const p = profileOf(l.profile)
  const model = ai().profiles.length > 1
    ? `<label class="btn split-lang pick live-model" title="Model for this call's live notes and minutes"><span>${esc(profileName(l.profile))}</span>${CARET}<select data-callprofile aria-label="Model for this call">${profileOptions(l.profile)}</select></label>`
    : `<span class="live-model">${esc(profileName(l.profile))}</span>`
  const where = p ? `<span class="hint">${esc(profileWhere(p))}</span>` : ''
  const draftBy = n && n.profile && l.profile && n.profile !== l.profile ? `<span class="hint">Draft by ${esc(profileName(n.profile))}; the next update by ${esc(profileName(l.profile))}</span>` : ''
  const pause = c.status === 'recording'
    ? `<button class="btn ghost" data-cmd="${st.paused ? 'live-resume' : 'live-pause'}">${st.paused ? 'Resume' : 'Pause'}</button>` : ''
  // A sign-in problem (or one going on) for this call's profile: said with what to do about it.
  const failedFor = st.needs_sign_in && profileOf(st.needs_sign_in)
  const sign = failedFor ? (signinNote(failedFor) || `<div class="signin error"><span class="signin-text">${esc(st.error || '')}</span><span class="signin-acts"><button class="btn primary" data-signin="${esc(failedFor.id)}">Sign in…</button></span></div>`)
    : signinNote(p)
  const problem = st.settings_error ? `The AI settings can't be read: ${st.settings_error}` : !st.enabled ? 'Live notes are off (menu → AI for calls → Live notes during calls).' : (!failedFor && st.state === 'failed' && st.error) || ''
  // What the call's live jobs used so far (Claude Code's own count).
  const u = l.usage
  const tok = (x) => x >= 1e6 ? `${(x / 1e6).toFixed(1)}M` : x >= 1e3 ? `${Math.round(x / 1e3)}K` : String(x)
  const used = u && u.jobs ? `<span class="live-usage" title="${esc(`${u.jobs} jobs · input ${u.input_tokens} + cache read ${u.cache_read_tokens} + cache write ${u.cache_write_tokens} · output ${u.output_tokens}${u.cost_usd ? ` · $${u.cost_usd.toFixed(3)}` : ''}`)}">${u.jobs} update${u.jobs === 1 ? '' : 's'} · ${tok(u.input_tokens + u.cache_read_tokens + u.cache_write_tokens)} in · ${tok(u.output_tokens)} out${u.cost_usd ? ` · $${u.cost_usd.toFixed(2)}` : ''}</span>` : ''
  return `<div class="live-head">${find}${model}${where}<span class="live-state ${esc(state)}">${esc(line)}</span>${draftBy}${used}<span class="grow"></span>${pause}</div>
    ${sign}${problem ? `<p class="live-error">${esc(problem)}</p>` : ''}`
}

function liveNotes(c) {
  const l = c.live || {}, n = l.notes, st = l.status || {}
  if (!n) {
    if (c.status !== 'recording') return '<p class="muted">No live notes for this call.</p>'
    // Why there are none yet (the reason itself is in the header above).
    const why = st.paused ? 'Paused — no live notes until you resume.'
      : st.needs_sign_in ? `No live notes until ${profileName(st.needs_sign_in)} can be used (see above).`
      : st.state === 'signing_in' ? 'No live notes until the sign-in is done.'
      : st.state === 'failed' ? 'No live notes yet: the last update failed (see above).'
      : st.state === 'busy' ? 'No live notes yet: the model is finishing another job.'
      : 'The first live notes appear after about 90 seconds of conversation.'
    return `<p class="muted">${esc(why)}</p>`
  }
  const sec = (title, items, f) => items && items.length ? `<h3>${title}</h3><ul>${items.map(f).join('')}</ul>` : ''
  const gaps = (n.gaps || []).map((g) => `${clock(g[0])}–${clock(g[1])}`).join(', ')
  const empty = !n.now && !(n.decisions || []).length && !(n.actions || []).length && !(n.questions || []).length
  return `${sec('Decisions', n.decisions, (x) => `<li>${esc(x.text)}${atLink(x.t)}</li>`)}
    ${sec('Action items', n.actions, (a) => `<li><span class="owner${a.owner === 'Me' ? ' me' : ''}">${esc(a.owner)}</span> ${esc(a.task)}${atLink(a.t)}</li>`)}
    ${sec('Open questions', n.questions, (x) => `<li>${esc(x.text)}${atLink(x.t)}</li>`)}
    ${empty ? '<p class="muted">Nothing decided, owed or open yet.</p>' : ''}
    ${gaps ? `<p class="muted">Not covered: ${esc(gaps)}</p>` : ''}`
}

/** What is going on now, above the live notes (it never scrolls away under them). */
function liveNow(c) {
  const n = (c.live || {}).notes
  return n && n.now ? `<p class="live-now"><b>Now</b> ${esc(n.now)}</p>` : ''
}

function prepCard(c) {
  const p = (c.live || {}).prep
  if (!p || (!(p.open_actions || []).length && !(p.highlights || []).length)) return ''
  const a = p.open_actions || [], h = p.highlights || []
  const mine = S.prepOpen[c.name]
  const open = (mine !== undefined ? mine : c.status === 'recording' && !(c.live.notes && c.live.notes.updated_at)) ? ' open' : ''
  return `<details class="prep" data-prep="${esc(c.name)}"${open}><summary>Last time${p.date ? ` (${esc(p.date)})` : ''}: ${a.length} open action${a.length === 1 ? '' : 's'}, ${h.length} highlight${h.length === 1 ? '' : 's'}</summary>
    ${a.length ? `<h3>Open actions</h3><ul>${a.map((x) => `<li>${x.owner ? `<span class="owner">${esc(x.owner)}</span> ` : ''}${esc(x.task)}${x.due ? ` <span class="muted">due ${esc(x.due)}</span>` : ''}</li>`).join('')}</ul>` : ''}
    ${h.length ? `<h3>Highlights</h3><ul>${h.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>` : ''}
  </details>`
}

/** An answer: escaped, line breaks kept, "* " / "- " bullets as •, [mm:ss] as jumps. */
function answerText(t) {
  return esc(String(t).replace(/^\s*[*-]\s+/gm, '• ')).replace(/\[((?:\d{1,2}:)?\d{1,2}:\d{2})\]/g, (_, x) => {
    const p = x.split(':').map(Number)
    const sec = p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1]
    return `<a class="at" data-at="${sec}" title="Jump to ${x} in the transcript">${x}</a>`
  }).replace(/\n/g, '<br>')
}

const ASK_TITLE = { missed: 'What did I miss?', question: 'Questions you could ask', ask: 'You asked' }
/** What a question put back waits for (`asking.waiting`). */
const ASK_WAIT = { model: 'Waiting for the model to finish another job…', sign_in: 'Waiting for the sign-in to finish…' }

function liveAnswers(c) {
  const l = c.live || {}, st = l.status || {}
  const pending = st.asking
    ? `<div class="answer pending"><b>${esc(ASK_TITLE[st.asking.kind] || '')}</b>${st.asking.text ? ` ${esc(st.asking.text)}` : ''}<p class="muted">${ASK_WAIT[st.asking.waiting] || 'Thinking…'}</p></div>`
    : st.queued > 0 ? `<div class="answer pending"><p class="muted">Your question is next.</p></div>` : ''
  const list = (l.answers || []).slice().reverse().slice(0, 30).map((a) => `<div class="answer">
      <div class="answer-head"><b>${esc(ASK_TITLE[a.kind] || '')}</b>${a.question ? ` <span>${esc(a.question)}</span>` : ''}<span class="muted">${esc(hm(new Date(a.asked_at)))}</span></div>
      ${a.error ? `<p class="error">${esc(a.error)}</p>` : `<p>${answerText(a.answer)}</p>`}</div>`).join('')
  return pending + list
}

function liveTab(c, segs) {
  // After the call: the final transcript, searchable, its live notes still beside it.
  const recording = c.status === 'recording'
  const find = !recording && segs.length ? `<input class="search" id="find" placeholder="Find in transcript" value="${esc(S.find)}" style="max-width:280px" aria-label="Find in transcript">` : ''
  const empty = recording ? 'Listening. The transcript appears here as people speak.' : c.status === 'transcribing' ? 'Transcribing on this computer.' : 'No transcript for this call.'
  return `<div class="live">
    ${liveHead(c, find)}
    <div class="live-grid">
      <section class="live-transcript" id="live-transcript" aria-label="Transcript">${segs.length ? turnsHtml(segs, recording ? '' : S.find.trim().toLowerCase(), recording ? null : S.jump, noteMarks(c)) : `<p class="muted">${empty}</p>`}</section>
      <section class="live-side" aria-label="Live notes">
        ${liveNow(c)}
        <div class="live-scroll">
          <div class="live-notes doc">${liveNotes(c)}</div>
          ${prepCard(c)}
        </div>
        <!-- Questions about the call go to its Claude session (the call's own profile, guarded). -->
        <div class="live-ask"><button class="btn" data-cmd="ask-claude">Ask Claude about this call</button></div>
        <div class="live-answers">${liveAnswers(c)}</div>
      </section>
    </div>
  </div>`
}

// ── calendar: today, or a rolling week (yesterday, today, 5 days ahead) ───────────────────
// Days and hours by the local calendar and clock, never by adding 24 h: a day the clocks change
// isn't 24 hours long (a week across one showed a day twice and drew the later days' events at
// midnight).
const ymd = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
const midnight = (d) => { const x = new Date(d); x.setHours(0, 0, 0, 0); return x }
const addDays = (d, n) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n)
const sameDay = (a, b) => midnight(a).getTime() === midnight(b).getTime()
/** Where time `t` (ms) is on `day`'s clock, in hours: 0 before the day, 24 after it. */
const clockHours = (t, day) => {
  const d = new Date(t)
  return sameDay(d, day) ? d.getHours() + d.getMinutes() / 60 + d.getSeconds() / 3600 : t < day.getTime() ? 0 : 24
}

/** Side-by-side lanes for overlapping blocks in one day column, by their drawn hours `y0`..`y1`. */
function lanes(items) {
  items.sort((a, b) => a.y0 - b.y0 || b.y1 - a.y1)
  const cols = []
  let group = [], groupEnd = 0
  const flush = () => { const n = Math.max(1, ...group.map((x) => x.lane + 1)); group.forEach((x) => (x.lanes = n)); group = [] }
  for (const it of items) {
    if (group.length && it.y0 >= groupEnd) { flush(); cols.length = 0 }
    let lane = cols.findIndex((end) => end <= it.y0)
    if (lane < 0) { lane = cols.length; cols.push(0) }
    cols[lane] = it.y1
    it.lane = lane
    group.push(it)
    groupEnd = Math.max(groupEnd, it.y1)
  }
  if (group.length) flush()
  return items
}
/** One day column's blocks (`s`, `real`: start and end, ms) where they are drawn: `y0`..`y1` in
 *  clock hours on `day`, at least 20 min so a short meeting is still a readable block, in lanes by
 *  those hours (the hour a day the clocks go back repeats would put two meetings an hour apart in
 *  one place). Counted in whole seconds, so a block ending when the next starts never overlaps it. */
const layoutDay = (items, day) => lanes(items.map((x) => {
  const sec = (t) => Math.round(clockHours(t, day) * 3600)
  const s0 = sec(x.s)
  return Object.assign(x, { y0: s0 / 3600, y1: Math.max(sec(x.real), s0 + 1200) / 3600 })
}))
// end of the calendar's helpers (calendar-dates.test.mjs runs app.js from `const ymd` to here)
function calRange() {
  const today = midnight(new Date())
  if (S.cal.mode === 'day') return { start: addDays(today, S.cal.offset), days: 1 }
  return { start: addDays(today, S.cal.offset * 7 - 1), days: 7 }
}
async function loadCalendar(fresh) {
  const { start, days } = calRange()
  S.cal.loading = true
  try {
    S.cal.data = await get(`api/calendar?start=${ymd(start)}&days=${days}${fresh ? '&fresh=1' : ''}`)
  } catch (e) { S.cal.data = { error: String(e), events: [], calls: [] } }
  S.cal.loading = false
}

function eventState(e, now) {
  const s = new Date(e.start), en = new Date(e.end)
  const past = en < now, live = s <= now && en > now
  const recordable = e.online && !e.declined && !e.cancelled && !e.no_notetakers && !past
  return { s, en, past, live, recordable }
}

function calendarView() {
  const d = S.cal.data
  const { start, days } = calRange()
  const now = new Date()
  const dayList = Array.from({ length: days }, (_, i) => addDays(start, i))
  const title = days === 1
    ? start.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })
    : `${dayList[0].toLocaleDateString([], { day: 'numeric', month: 'short' })} to ${dayList[days - 1].toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' })}`
  const head = `<div class="cal-head"><h1>${esc(title)}</h1>
    <div class="seg" role="group" aria-label="View"><button data-calmode="day" aria-pressed="${S.cal.mode === 'day'}">Day</button><button data-calmode="week" aria-pressed="${S.cal.mode === 'week'}">Week</button></div>
    <button class="btn ghost" data-calnav="-1" aria-label="Earlier">‹</button><button class="btn" data-calnav="0">Today</button><button class="btn ghost" data-calnav="1" aria-label="Later">›</button></div>`
  let note = ''
  if (d && d.error) note = `<div class="cal-note error">Can't read the calendar: ${esc(d.error)}</div>`
  else if (d && !d.signed_in) note = `<div class="cal-note">Showing today's meetings only. Connect Microsoft 365 from the menu-bar octopus (Calendar) to see your week.</div>`
  if (!d) return `<div class="cal">${head}${note}<div style="padding:20px"><div class="skeleton">${'<i></i>'.repeat(8)}</div></div></div>`

  // Blocks per day.
  const blocks = dayList.map(() => [])
  const allday = dayList.map(() => [])
  for (const e of d.events || []) {
    const st = eventState(e, now)
    const di = dayList.findIndex((x) => sameDay(x, st.s))
    if (di < 0) continue
    if (e.all_day) { allday[di].push(e); continue }
    blocks[di].push({ kind: 'event', ev: e, st, s: st.s.getTime(), real: st.en.getTime() })
  }
  for (const c of d.calls || []) {
    const s = new Date(c.start), en = c.end ? new Date(c.end) : new Date(s.getTime() + 30 * 60000)
    const di = dayList.findIndex((x) => sameDay(x, s))
    if (di >= 0) blocks[di].push({ kind: 'call', c, s: s.getTime(), real: en.getTime() })
  }
  blocks.forEach((b, di) => layoutDay(b, dayList[di]))

  // The whole day, scrolling; an hour is sized so the working day (08:00–19:00) fills the
  // window (min 40 px).
  const h0 = 0, h1 = 24
  const reserved = 112 + (allday.some((a) => a.length) ? 30 : 0) + (note ? 34 : 0)
  const HOUR = Math.max(40, Math.floor((window.innerHeight - reserved) / 11))
  const cols = `grid-template-columns: repeat(${days}, minmax(0, 1fr))`
  const dayHead = `<div class="days" style="${cols}">${dayList.map((x) => `<div class="${sameDay(x, now) ? 'today' : ''}">${esc(x.toLocaleDateString([], { weekday: 'short' }))}<b>${x.getDate()}</b></div>`).join('')}</div>`
  const alldayRow = allday.some((a) => a.length) ? `<div class="allday" style="${cols}">${allday.map((a) => `<div>${a.map((e) => `<span title="${esc(e.subject)}">${esc(e.subject)}</span>`).join('')}</div>`).join('')}</div>` : ''
  const hourLabels = Array.from({ length: h1 - h0 }, (_, i) => `<span style="top:${i * HOUR}px">${i === 0 ? '' : String(h0 + i).padStart(2, '0') + ':00'}</span>`).join('')
  const top = (hours) => (hours - h0) * HOUR
  const colHtml = dayList.map((day, di) => {
    const lines = Array.from({ length: h1 - h0 }, (_, i) => `<div class="hourline" style="top:${i * HOUR}px"></div>`).join('')
    const nowLine = sameDay(day, now) ? `<div class="now" style="top:${top(clockHours(now.getTime(), day))}px"></div>` : ''
    const items = blocks[di].map((x) => {
      const y = top(x.y0), h = Math.max(22, top(x.y1) - y - 2)
      const w = 100 / x.lanes, left = x.lane * w
      const style = `top:${y}px;height:${h}px;left:calc(${left}% + 2px);width:calc(${w}% - 4px)`
      if (x.kind === 'call') {
        return `<button class="ev call" style="${style}" data-call="${esc(x.c.name)}"><b>${esc(x.c.title)}</b><small>${esc(hm(new Date(x.s)))}</small></button>`
      }
      const { ev: e, st } = x
      const cls = ['ev', e.online ? 'online' : '', e.recorded ? 'recorded' : '', e.scheduled ? 'scheduled' : '', st.past ? 'past' : '', !e.recorded && e.transcript ? 'transcript' : '', e.declined ? 'declined' : '', e.cancelled ? 'cancelled' : ''].filter(Boolean).join(' ')
      // Green already says recorded: only what the colour doesn't.
      const tag = e.recorded ? '' : e.transcript ? '<span class="tag tr" title="Teams has its transcript: click, then Import">Transcript</span>' : e.scheduled ? '<span class="tag will">Will record</span>' : e.no_notetakers ? '<span class="tag no">No notetakers</span>' : ''
      return `<button class="${cls}" style="${style}" data-ev="${esc(e.id || '')}" aria-selected="${S.cal.selected === e.id}"><b>${esc(e.subject)}</b><small>${esc(hm(st.s))}${tag ? ' ' + tag : ''}</small></button>`
    }).join('')
    return `<div class="col${sameDay(day, now) ? ' today' : ''}" style="height:${(h1 - h0) * HOUR}px">${lines}${nowLine}${items}</div>`
  }).join('')
  const empty = !blocks.some((b) => b.length) && !allday.some((a) => a.length)
    ? `<p style="position:absolute;top:40%;left:0;right:0;text-align:center;color:var(--text-3);pointer-events:none">${days === 1 ? 'Nothing on the calendar this day.' : 'Nothing on the calendar these days.'}</p>` : ''
  const grid = `<div class="grid-wrap" id="calgrid" data-hour="${HOUR}"><div class="grid" style="grid-template-columns: 56px repeat(${days}, minmax(0, 1fr))"><div class="hours" style="height:${(h1 - h0) * HOUR}px">${hourLabels}</div>${colHtml}</div>${empty}</div>`
  const sel = S.cal.selected && (d.events || []).find((e) => e.id === S.cal.selected)
  return `<div class="cal">${head}${note}<div style="display:grid;grid-template-rows:auto auto 1fr;min-height:0">${dayHead}${alldayRow}${grid}</div>${sel ? eventDetail(sel, now) : ''}</div>`
}

function eventDetail(e, now) {
  const st = eventState(e, now)
  const when = `${st.s.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })}, ${hm(st.s)} to ${hm(st.en)}`
  const ACTS = {
    'open-notes': () => `<button class="btn primary" data-call="${esc(e.recorded.name)}">Open notes</button>`,
    scheduled: () => `<button class="btn" data-unschedule="${esc(e.id)}">Don't record</button><p class="why">Echo arms itself a minute before it starts${e.scheduled_languages ? `, transcribed as ${esc(e.scheduled_languages.map(langName).join(', '))}` : ''}.</p>`,
    join: () => `${recordButton(`data-join="${esc(e.id)}"`, 'Join and record')}<button class="btn" data-schedule="${esc(e.id)}">Record it</button>`,
    record: () => recordButton(`data-schedule="${esc(e.id)}"`, st.live ? 'Record it' : 'Record when it starts'),
    import: () => e.import && e.import.busy ? '<button class="btn" disabled>Importing the Teams transcript…</button>'
      : `<button class="btn primary" data-cal-import="${esc(e.id)}">Import Teams transcript</button>`,
  }
  const kind = eventAction(e, st)
  const acts = kind ? ACTS[kind]() : ''
  const why = e.recorded ? '' : e.import && e.import.error ? e.import.error : kind === 'import' ? (e.transcript ? 'Not recorded, but Teams transcribed it' : e.transcript === false ? 'Not recorded, and Teams has no transcript of it (yet)' : 'Not recorded. If Teams transcribed it') + ' — Echo makes a call of its transcript and chat; minutes, threads and the Brain follow.' : e.no_notetakers ? 'The organizer asked for no notetakers.' : e.declined ? 'You declined this meeting.' : e.cancelled ? 'This meeting was cancelled.' : !e.online ? 'Not a Teams meeting.' : st.past ? 'This meeting is over and was not recorded.' : ''
  return `<aside class="detail">
    <button class="btn ghost close" data-calclose="1" aria-label="Close">Close</button>
    <h2 class="selectable">${esc(e.subject)}</h2><div class="when">${esc(when)}</div>
    <dl>${e.organizer ? `<dt>Organizer</dt><dd>${esc(e.organizer)}</dd>` : ''}${e.people ? `<dt>People</dt><dd>${e.people}</dd>` : ''}${e.location ? `<dt>Where</dt><dd>${esc(e.location)}</dd>` : ''}${e.recorded ? `<dt>${e.recorded.imported ? 'Call' : 'Recording'}</dt><dd>${e.recorded.imported ? 'from the Teams transcript' : STATUS[e.recorded.status] || esc(e.recorded.status)}${e.recorded.minutes ? `, ${e.recorded.minutes} min` : ''}</dd>` : ''}</dl>
    ${acts || slotItems('calendar.event', { kind: 'event', event: e }) ? `<div class="acts">${acts}${slotItems('calendar.event', { kind: 'event', event: e })}</div>` : ''}${why ? `<p class="why">${esc(why)}</p>` : ''}
    ${eventProjects(e)}
    ${e.recap && e.recap.length ? `<div><b style="font-size:12px">Last time</b><ul>${e.recap.map((r) => `<li>${esc(r)}</li>`).join('')}</ul></div>` : ''}
  </aside>`
}

/** A calendar event's projects on its card: chips (× on one added by hand to this event), and
 *  + Project — add it to a project (its call will belong), always its meeting series, or a new
 *  project from it. A recorded meeting's projects are its call's. */
function eventProjects(e) {
  if (!e.id) return ''
  const mine = e.projects || []
  const ids = new Set(mine.map((p) => p.id))
  const WHYS = { series: 'its meeting series', people: "the client's people", chat: 'its Teams chat', subject: 'its subject', pinned: 'added by hand' }
  const chips = mine.map((p) => `<span class="pj-chip" title="${esc(WHYS[p.why] || p.why)}">${projIcon(p, 14)}<button class="link-btn" data-pj="goto" data-id="${esc(p.id)}">${esc(p.name)}</button>${p.why === 'pinned' && !e.recorded ? `<button class="pj-x" data-pj="event-member" data-id="${esc(p.id)}" data-event="${esc(e.id)}" data-how="remove" aria-label="Take it out of ${esc(p.name)}">×</button>` : ''}</span>`).join('')
  const all = ((S.pj.data && S.pj.data.projects) || []).filter((p) => p.group !== 'closed' && !ids.has(p.id))
  const picker = S.pj.evPicker === e.id ? `<div class="menu pj-menu" role="menu">${all.map((p) => `<button class="menu-item" role="menuitem" data-pj="event-member" data-id="${esc(p.id)}" data-event="${esc(e.id)}" data-how="include">${projIcon(p, 14)} ${esc(p.name)}</button>`).join('') || '<div class="menu-item set-sub">No other threads</div>'}<div class="menu-sep" role="separator"></div>${all.length ? `<label class="menu-item pj-series" title="Every occurrence of this meeting joins the thread">Every occurrence → <select data-ev-series="${esc(e.subject || '')}"><option value=""></option>${all.concat(mine.map((m) => ((S.pj.data && S.pj.data.projects) || []).find((x) => x.id === m.id)).filter(Boolean)).map((p) => `<option value="${esc(p.id)}">${esc(p.name)}</option>`).join('')}</select></label>` : ''}<button class="menu-item" role="menuitem" data-pj="event-new" data-event="${esc(e.id)}" data-subject="${esc(e.subject || '')}">New thread from this meeting…</button></div>` : ''
  if (e.recorded && !mine.length) return ''
  return `<div class="pj-chips ev-pj">${chips}${e.recorded ? '' : `<span class="pj-add-wrap"><button class="pj-chip add" data-pj="ev-picker" data-event="${esc(e.id)}" aria-haspopup="menu" aria-expanded="${S.pj.evPicker === e.id}">+ Thread</button>${picker}</span>`}</div>`
}

// ── Settings: every setting in one place (the tray keeps quick toggles). Plain values save at once
// (`settings_set`, the value shown, never a toggle of a stale one); flows with a dialog, a sign-in or
// a download run as the tray's own actions (`settings_action`). ────────────────────────────────
const SETTINGS_SECTIONS = [
  ['general', 'General'], ['recording', 'Recording'], ['transcription', 'Transcription'], ['ai', 'AI & minutes'],
  ['calendar', 'Calendar & chat'], ['brain', 'Brain'], ['me', 'About me'], ['deliver', 'Sends to you'], ['plugins', 'Plugins'], ['layout', 'Layout'], ['health', 'Health'],
]
/** Save the profile as edited; shown rendered again once the app has it. */
function saveMeProfile() {
  if (S.meEdit == null) return
  const text = S.meEdit
  send({ cmd: 'settings_set', key: 'me_profile', value: text })
  if (S.settings) S.settings.me_profile = { text, html: S.settings.me_profile ? S.settings.me_profile.html : '' }
  S.meEdit = null
  render()
  setTimeout(() => loadSettings().then(render), 400)
}
async function loadSettings() {
  try { S.settings = await get('api/settings') } catch { /* app restarting */ }
}
/** Required permissions not granted: Settings' badge. */
const settingsWarnings = () => ((S.settings && S.settings.health) || []).filter((c) => c.required && c.status !== 'granted').length
const setRow = (title, sub, control, id = '') => `<div class="set-row"${id ? ` id="set-${id}"` : ''}><div><b>${title}</b>${sub ? `<div class="set-sub">${sub}</div>` : ''}</div><div class="set-ctl">${control}</div></div>`
const setCard = (head, rows) => `<section class="set-sec">${head ? `<h3>${esc(head)}</h3>` : ''}<div class="set-card">${rows.join('')}</div></section>`
const sw = (key, on, label) => `<button class="sw${on ? ' on' : ''}" role="switch" aria-checked="${!!on}" aria-label="${esc(label)}" data-set="${key}" data-val="${!on}"></button>`
const seg = (key, cur, opts) => `<div class="seg" role="group">${opts.map(([v, l]) => `<button data-set="${key}" data-val='${esc(JSON.stringify(v))}' aria-pressed="${cur === v}">${esc(l)}</button>`).join('')}</div>`
const actBtn = (action, label, arg = '', primary = false) => `<button class="btn${primary ? ' primary' : ''}" data-act="${action}"${arg ? ` data-arg="${esc(arg)}"` : ''}>${esc(label)}</button>`
const LANG_LIST = ['en', 'uk', 'ru', 'es', 'de', 'fr', 'it', 'pl', 'pt', 'nl', 'cs', 'ro', 'tr', 'he', 'ar', 'hi', 'ja', 'zh', 'ko']

function settingsView() {
  const st = S.settings
  const own = Plugins.sections.find((x) => x.id === S.setSection)
  const sec = own ? own.id : SETTINGS_SECTIONS.some(([id]) => id === S.setSection) ? S.setSection : 'general'
  const q = (S.setQuery || '').trim().toLowerCase()
  const nav = SETTINGS_SECTIONS.filter(([, l]) => !q || l.toLowerCase().includes(q)).map(([id, label]) => {
    const warn = id === 'health' && settingsWarnings() ? `<span class="set-warn">⚠ ${settingsWarnings()}</span>` : ''
    return `<button class="set-nav${id === sec ? ' on' : ''}" data-setsec="${id}">${esc(label)}${warn}</button>`
  }).join('')
  // Plugins' own sections (`ctx.settings.section`), under their own heading.
  const mine = Plugins.sections.filter((x) => !q || x.label.toLowerCase().includes(q))
  const pnav = mine.length ? `<div class="set-grp">From plugins</div>` + mine.map((x) => {
    let badge = ''
    try { badge = x.badge ? x.badge() : '' } catch { /* none */ }
    return `<button class="set-nav${x.id === sec ? ' on' : ''}" data-setsec="${esc(x.id)}">${x.icon || ''}${esc(x.label)}${badge !== '' && badge != null ? `<span class="set-n">${esc(String(badge))}</span>` : ''}</button>`
  }).join('') : ''
  const body = own ? (() => { try { return own.render() } catch (e) { return `<p class="error">${esc(own.label)}: ${esc(String((e && e.message) || e))}</p>` } })()
    : !st ? `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
    : st.config_error ? `<p class="error">Echo's settings (config.json) can't be read: ${esc(st.config_error)}</p>`
    // A section that fails says so in its place; the rest of the window keeps working.
    : (() => { try { return (SETTINGS_BODY[sec] || SETTINGS_BODY.general)(st) } catch (e) { console.error(e); return `<p class="error">This section couldn't be shown: ${esc(String((e && e.message) || e))}</p>` } })()
  if (own) return `<div class="settings"><aside class="set-side"><h1>Settings</h1><input class="search" id="set-q" placeholder="Search settings" value="${esc(S.setQuery || '')}" aria-label="Search settings">${nav}${pnav}</aside>
    <div class="set-main plug-main">${body}</div></div>`
  return `<div class="settings"><aside class="set-side"><h1>Settings</h1><input class="search" id="set-q" placeholder="Search settings" value="${esc(S.setQuery || '')}" aria-label="Search settings">${nav}${pnav}</aside>
    <div class="set-main${sec === 'plugins' ? ' pl-main' : ''}">${body}<p class="set-foot">Saved as you change it.</p></div></div>`
}

/** The profile the minutes read (me.md): shown rendered (the minutes' own safe Markdown), edited in place. */
function meProfileCard(mp) {
  const head = (ctl) => `<div class="set-row"><div><b>Your profile</b><div class="set-sub">Who you are, the people and terms around you, how you work and like notes written. Sections by heading: “People…”, “Terms…”, “How I work…”, “How I like notes…”.</div></div><div class="set-ctl">${ctl}</div></div>`
  // Who reads which part (`calls::me_parts`): the readers of other people's words get the least.
  const readers = `<div class="set-sub me-readers"><b>Who reads it</b> · minutes, the threads' sync (work and people) and Claude in the window: all of it — your patterns are their defaults, what was said wins · live notes and questions: who you are, people and terms · the Brain's mail and channel reader: people and terms only, for spelling</div>`
  if (S.meEdit != null) {
    return `<section class="set-sec"><div class="set-card">${head('<button class="btn ghost" data-me-cancel>Cancel</button><button class="btn primary" data-me-save>Save</button>')}${readers}
      <div class="me-edit"><textarea id="me-profile" spellcheck="true" aria-label="Your profile (Markdown)">${esc(S.meEdit)}</textarea><div class="set-sub">Markdown: # for headings, - for lists. ⌘S / Ctrl+S saves, Esc cancels.</div></div></div></section>`
  }
  const body = mp && mp.text.trim() ? `<div class="doc me-doc">${mp.html}</div>` : '<div class="me-doc set-sub">Nothing yet — tell the minutes who you are.</div>'
  return `<section class="set-sec"><div class="set-card">${head('<button class="btn" data-me-edit>Edit</button>')}${readers}${body}</div></section>`
}

/** The terms the minutes learned: each with the meetings and people it is used again for. */
/** The vocabulary (a panel view, opened from the book beside Calls): terms the transcription should hear
 *  right — the user's own, used in every call, and those the minutes learned, each scoped to its meetings
 *  and people. Saved through `settings_set` as Settings' values are. */
function vocabularyPanel() {
  const st = S.settings
  if (!st) { loadSettings().then(render); return '<div class="skeleton"><i></i><i></i></div>' }
  const v = st.vocabulary || []
  const chips = v.map((t) => `<span class="chip">${esc(t)}<button data-vocab-del="${esc(t)}" aria-label="Remove ${esc(t)}">×</button></span>`).join('')
  const learned = st.learned || []
  const q = (S.learnedQuery || '').trim().toLowerCase()
  const shown = learned.filter((x) => !q || x.term.toLowerCase().includes(q) || x.subjects.some((s) => s.includes(q)) || x.people.some((p) => p.includes(q)))
    .sort((a, b) => a.term.localeCompare(b.term))
  const list = (xs, n = 2) => xs.length > n ? `${xs.slice(0, n).map(esc).join(', ')} +${xs.length - n}` : xs.map(esc).join(', ')
  const rows = shown.map((x) => `<div class="voc-row"><div class="voc-t"><b>${esc(x.term)}</b><span class="grow"></span>
      <button class="link-btn" data-set="learned_always" data-val='${esc(JSON.stringify(x.term))}' title="Use it in every call">Always</button>
      <button class="link-btn" data-set="learned_remove" data-val='${esc(JSON.stringify(x.term))}' title="Forget it">Remove</button></div>
    <div class="set-sub">${!x.subjects.length && !x.people.length ? 'Only when a meeting\'s invite names it'
      : [x.subjects.length ? `Meetings: ${list(x.subjects)}` : '', x.people.length ? `People: ${list(x.people)}` : '', x.calls ? `${x.calls} call${x.calls === 1 ? '' : 's'}` : ''].filter(Boolean).join(' · ')}</div></div>`).join('')
  return `<div class="voc">
    <p class="set-sub">Names and terms the transcription should hear right.</p>
    <h4 class="voc-h">Every call</h4>
    <div class="chips">${chips || '<span class="set-sub">None yet.</span>'}</div>
    <form data-vocab-form class="vocab-add"><input class="search" id="vocab-new" placeholder="Add a term" maxlength="50" aria-label="Add a term"><button class="btn" type="submit">Add</button></form>
    <h4 class="voc-h">Learned from calls <span class="set-sub">${learned.length}</span></h4>
    <p class="set-sub">Each used again only in calls of the same meeting, or with mostly the same people.</p>
    ${learned.length > 6 ? `<input class="search" id="learned-q" placeholder="Filter terms, meetings, people" value="${esc(S.learnedQuery || '')}" aria-label="Filter learned terms">` : ''}
    ${rows || (learned.length ? '<p class="set-sub">Nothing matches.</p>' : '<p class="set-sub">None yet — the minutes add the names they had to fix.</p>')}
  </div>`
}
const BOOK = ICON('<path d="M5 4.5h9.5A2.5 2.5 0 0 1 17 7v13H7.5A2.5 2.5 0 0 1 5 17.5z"/><path d="M5 17.5A2.5 2.5 0 0 1 7.5 15H17M19 7v13"/>')
PANEL_VIEWS.push({ id: 'vocabulary', label: 'Vocabulary', icon: BOOK, area: 'right', render: vocabularyPanel })

/** Settings → Brain: the embedding server Echo asks (no model runs in Echo), what the Brain holds,
 *  and whether the app keeps it current. */
/** Settings → Brain → Documents: who reads the pages of thread folders that their text misses
 *  (diagrams, timelines, title-only slides), and what lays documents out here. */
function documentsSettings(b, s, field) {
  const d = b.documents || {}
  const pages = s.pages || 'claude'
  const who = `<select class="set-in" data-brain-set="brain_pages" aria-label="Page reader"><option value="claude"${pages === 'claude' ? ' selected' : ''}>Claude (the default AI profile)</option><option value="endpoint"${pages === 'endpoint' ? ' selected' : ''}>A vision endpoint</option><option value="off"${pages === 'off' ? ' selected' : ''}>Off — their text only</option></select>`
  const vapi = `<select class="set-in" data-brain-set="brain_vision_api" aria-label="Vision API"><option value="ollama"${s.vision_api !== 'openai' ? ' selected' : ''}>Ollama</option><option value="openai"${s.vision_api === 'openai' ? ' selected' : ''}>OpenAI-compatible</option></select>`
  const tools = [d.pdfium ? 'PDF pages: ready' : 'PDF pages: <span class="error">PDFium missing</span>', d.libreoffice ? 'decks and Word layouts: LibreOffice' : 'decks and Word layouts: <span class="error">no LibreOffice</span> — slides are indexed from their text'].join(' · ')
  const rows = [
    setRow('Page reader', `Each page is read from its text unless its text misses the meaning — a diagram, a timeline, a title-only slide. Those pages are read from their image, once (cached), while Echo isn't recording.${d.pending ? ` ${d.pending} page${d.pending === 1 ? '' : 's'} waiting.` : ''}`, who),
  ]
  if (pages === 'endpoint') {
    rows.push(setRow('Vision server', 'A vision model on Ollama or an OpenAI-compatible server.', `${vapi}${field('brain_vision_endpoint', s.vision_endpoint, 'http://localhost:11434', 'Vision server address')}`))
    rows.push(setRow('Vision model', 'One that reads images (e.g. qwen2.5vl, llama3.2-vision).', field('brain_vision_model', s.vision_model, 'qwen2.5vl', 'Vision model')))
    rows.push(setRow('Key', 'Only for a server that wants one.', field('brain_vision_key', '', s.vision_key ? '•••• set — type to replace' : 'none', 'Vision key', 'password')))
  }
  rows.push(setRow('Laying out', tools, ''))
  return setCard('Documents (thread folders)', rows)
}

function brainSettings(st) {
  const b = st.brain
  if (!b) return '<p class="set-sub">This build of Echo has no Brain.</p>'
  const onRow = setRow('Use the Brain', 'Cross-call memory: your calls, channels, mail and folders indexed for search, topics, insights and Claude. Off: nothing is read, indexed or offered to Claude, and it leaves the window — what it holds stays on disk for when you turn it back on.', sw('brain_enabled', b.enabled !== false, 'Use the Brain'))
  if (b.enabled === false) return setCard('', [onRow]) + `<p class="set-foot">Kept in <code>${esc(b.path || 'brain/')}</code>.</p>`
  const s = b.settings || {}
  const field = (key, val, ph, label, type = 'text') => `<input class="set-in" type="${type}" data-brain-set="${key}" value="${esc(val || '')}" placeholder="${esc(ph)}" aria-label="${esc(label)}" spellcheck="false">`
  const api = `<select class="set-in" data-brain-set="brain_api" aria-label="API"><option value="ollama"${s.api !== 'openai' ? ' selected' : ''}>Ollama</option><option value="openai"${s.api === 'openai' ? ' selected' : ''}>OpenAI-compatible</option></select>`
  const kinds = Object.entries(b.by_kind || {}).map(([k, n]) => `${n} ${k}${n === 1 ? '' : 's'}`).join(', ')
  const held = b.exists ? `${kinds || 'nothing yet'} · ${b.chunks} sections · ${b.embedded} with meaning${b.embed_model ? ` (${esc(b.embed_model)}${b.embed_dim ? `, ${b.embed_dim}-d` : ''})` : ''} · ${(b.bytes / 1048576).toFixed(1)} MB` : 'Not built yet'
  const last = b.last ? (b.last.error ? `<span class="error">${esc(b.last.error)}</span>` : `Last sync: ${b.last.added || 0} added, ${b.last.changed || 0} changed, ${b.last.removed || 0} removed, ${b.last.embedded || 0} embedded${b.last.embed_error ? ` · <span class="error">${esc(b.last.embed_error)}</span>` : ''}`) : (b.built_at ? `Updated ${esc(relDay(String(b.built_at).slice(0, 10)))}` : '')
  const u = b.usage || {}, by = u.by || {}
  const sum = (o) => Object.values(o || {}).reduce((n, k) => n + k, 0)
  const asked = `${u.claude || 0} time${u.claude === 1 ? '' : 's'} in the last ${u.days || 7} days` + (u.claude ? ` — ${[['folder', 'the Echo folder session'], ['call', 'call sessions'], ['routine', 'routines']].filter(([k]) => sum(by[k])).map(([k, l]) => `${l} ${sum(by[k])}`).join(' · ')}` : '')
  const seen = by.window ? ` · in the window: Also discussed ${by.window.also_discussed || 0}, Mentioned also in ${by.window.mentioned_also_in || 0}` : ''
  const usageRow = setRow('How much it is used', `Claude asked it ${asked}${seen}. Claude sees what it holds and when to ask it at the start of every Echo session.`, '')
  return setCard('', [
    onRow,
    usageRow,
    setRow('What it holds', `${held}${last ? `<br>${last}` : ''}`, b.syncing ? '<span class="set-sub">Syncing…</span>' : '<button class="btn" data-brain-sync>Sync now</button>'),
    setRow('Keep it current', 'Echo adds new and changed calls, channel posts, mail and folder files by itself (checked every 5 minutes).', sw('brain_auto', b.auto, 'Keep the Brain current')),
  ]) + setCard('Meaning (embeddings)', [
    setRow('Server', 'Echo runs no model: it asks yours. Ollama (e.g. http://localhost:11434) or any OpenAI-compatible server. Without one, the Brain searches by words only.', `${api}${field('brain_endpoint', s.endpoint, 'http://localhost:11434', 'Server address')}`),
    setRow('Model', 'An embedding model that knows your languages (e.g. qwen3-embedding, bge-m3). Another model embeds everything again.', field('brain_model', s.model, 'qwen3-embedding', 'Model')),
    setRow('Key', 'Only for a server that wants one (sent as a bearer token; kept on this computer).', field('brain_key', '', s.key ? '•••• set — type to replace' : 'none', 'Key', 'password')),
  ]) + documentsSettings(b, s, field) + `<p class="set-foot">Sources come from your threads — their calls, Teams channels, mail and folders, set on each thread's Profile. Stored in <code>${esc(b.path || 'brain/')}</code>.</p>`
}
document.addEventListener('change', (e) => {
  const t = e.target
  if (!t.matches || !t.matches('[data-brain-set]')) return
  send({ cmd: 'settings_set', key: t.dataset.brainSet, value: t.value })
  if (t.type === 'password') t.value = ''
  setTimeout(() => loadSettings().then(render), 300)
})
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-brain-sync]')
  if (!b) return
  send({ cmd: 'brain_sync' })
  b.disabled = true
  b.textContent = 'Syncing…'
  setTimeout(() => loadSettings().then(render), 1000)
})
// ── Settings → Plugins: installed, offered by the marketplaces, the marketplaces themselves. Every change
// runs in the app (IPC `plugin_op`); installs and fetches in the background, shown from `busy`.
async function loadPluginsView() {
  try { S.plugins = await get('api/plugins') } catch { /* app restarting */ }
  const p = S.plugins
  // What is installed and on may have changed: the window's plugin parts follow.
  if (p && !p.error) loadPluginClients(p)
  // While something runs (a fetch, an install), read again until it ends.
  if (p && p.busy && Object.keys(p.busy).length) setTimeout(() => { if (S.view === 'settings' && S.setSection === 'plugins') loadPluginsView().then(render) }, 1000)
}
function pluginOp(op, body = {}) {
  send({ cmd: 'plugin_op', op, ...body })
  setTimeout(() => loadPluginsView().then(render), 300)
}
/** One of a plugin's own settings (its manifest's `settings`): a switch, or a box saved when it changes; a
 *  secret is never shown back, only whether it is set (Forget empties it). */
function plField(id, d) {
  const at = `data-pl-set="${esc(id)}" data-key="${esc(d.key)}"`
  const box = `id="pl-${esc(id)}-${esc(d.key)}" ${at} aria-label="${esc(d.label)}" spellcheck="false"`
  if (d.type === 'bool') {
    const on = d.value === true || d.value === 'true'
    return `<button class="sw${on ? ' on' : ''}" role="switch" aria-checked="${on}" aria-label="${esc(d.label)}" ${at} data-val="${!on}"></button>`
  }
  if (d.type === 'secret') return `<input class="set-in" type="password" ${box} placeholder="${d.set ? '•••• set — type to replace' : 'not set'}" autocomplete="off">${d.set ? `<button class="btn ghost" ${at} data-forget>Forget</button>` : ''}`
  return `<input class="set-in" ${box} value="${esc(d.value == null ? '' : String(d.value))}" placeholder="${d.type === 'folder' ? '/path/to/folder' : ''}">`
}
/** A plugin's setting being typed (not saved yet): renders wait until the box is left, so the text isn't lost. */
const plTyping = () => { const a = document.activeElement; return !!(a && a.matches && a.matches('input[data-pl-set]') && a.value !== a.defaultValue) }
document.addEventListener('change', (e) => {
  const t = e.target
  if (!t.matches || !t.matches('input[data-pl-set]')) return
  pluginOp('set', { id: t.dataset.plSet, key: t.dataset.key, value: t.value })
  if (t.type === 'password') t.value = ''
  t.defaultValue = t.value
})
document.addEventListener('focusout', (e) => {
  if (e.target.matches && e.target.matches('input[data-pl-set]') && S.renderOnBlur) { S.renderOnBlur = false; setTimeout(render) }
})
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('button[data-pl-set]')
  if (!b) return
  if ('forget' in b.dataset) pluginOp('set', { id: b.dataset.plSet, key: b.dataset.key, value: '' })
  else pluginOp('set', { id: b.dataset.plSet, key: b.dataset.key, value: b.dataset.val === 'true' })
})
/** Settings → Plugins (designed on the Workbench canvas): Installed — a row each, the chosen one on the right;
 *  Discover — categories, featured, cards, the install sheet over the page; Marketplaces — where plugins come from. */
const PL_COLORS = ['#9b78ff', '#4fb3d9', '#3a9cf0', '#e0a96d', '#3ccf91', '#e07fa3', '#d9a84f', '#7fd0c0']
const plColor = (id) => PL_COLORS[[...String(id)].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 7) % PL_COLORS.length]
/** A plugin's tile: the icon of its own Settings section when it has one, else its initial, in its colour. */
function plTile(x, big = false) {
  const sec = Plugins.sections.find((s) => s.plugin === x.id)
  const inner = sec && sec.icon ? sec.icon : esc((x.name || x.id || '?').trim().charAt(0).toUpperCase())
  return `<span class="pl-tile${big ? ' big' : ''}" style="--c:${plColor(x.id)}">${inner}</span>`
}
const plMatch = (x) => { const q = (S.plQuery || '').trim().toLowerCase(); return !q || `${x.name} ${x.summary || ''} ${x.id} ${(x.categories || []).join(' ')}`.toLowerCase().includes(q) }
function pluginsSettings() {
  const p = S.plugins
  if (!p) { if (!S.pluginsLoading) { S.pluginsLoading = true; loadPluginsView().then(() => { S.pluginsLoading = false; render() }) } return '<h2>Plugins</h2><div class="skeleton" style="padding:20px"><i></i><i></i><i></i></div>' }
  if (p.error) return `<h2>Plugins</h2><p class="error">${esc(p.error)}</p>`
  const tab = S.plTab || ((p.installed || []).length ? 'installed' : 'discover')
  const tabs = [['installed', 'Installed', p.installed.length || ''], ['discover', 'Discover', ''], ['markets', 'Marketplaces', p.markets.length]]
  const head = `<div class="pl-head"><h2>Plugins</h2>${tab === 'markets' ? '' : `<input class="search pl-q" id="pl-q" placeholder="Search plugins" value="${esc(S.plQuery || '')}" aria-label="Search plugins">`}</div>`
  const bar = `<div class="tabs pl-tabs" role="tablist">${tabs.map(([id, l, n]) => `<button class="tab" role="tab" data-pl-tab="${id}" aria-selected="${tab === id}">${esc(l)}${n !== '' ? `<span class="n">${esc(String(n))}</span>` : ''}</button>`).join('')}</div>`
  const body = tab === 'installed' ? plInstalled(p) : tab === 'discover' ? plDiscover(p) : plMarkets(p)
  const sheet = S.plSheet ? `<div class="pl-scrim" data-pl-sheet-cancel>${plSheet(S.plSheet)}</div>` : ''
  return `<div class="pl-page">${head}${bar}${body}${sheet}</div>`
}
function plInstalled(p) {
  if (!p.installed.length) return '<p class="set-sub">No plugins yet — Discover lists what your marketplaces offer.</p>'
  const list = p.installed.filter(plMatch).sort((a, b) => a.name.localeCompare(b.name))
  const sel = p.installed.find((x) => x.id === S.plSel) || list[0] || p.installed[0]
  const row = (x) => {
    const pill = x.update && !x.update.blocked ? `<span class="pl-pill up">Update to ${esc(x.update.version)}</span>`
      : Plugins.errors[x.id] || p.errors && p.errors[`plugin:${x.id}`] ? '<span class="pl-pill bad">Has a problem</span>' : '<span></span>'
    return `<div class="pl-row${x.id === sel.id ? ' on' : ''}${x.enabled ? '' : ' off'}" data-pl-sel="${esc(x.id)}">${plTile(x)}<div class="pl-txt"><div class="pl-n">${esc(x.name)} <span class="pl-v">${esc(x.version)}</span></div><div class="pl-s">${esc(x.summary || '')}</div></div>${pill}<button class="sw${x.enabled ? ' on' : ''}" role="switch" aria-checked="${!!x.enabled}" aria-label="${esc(x.name)} on" data-pl-enable="${esc(x.id)}" data-on="${!x.enabled}"></button></div>`
  }
  const rows = list.length ? list.map(row).join('') : '<p class="set-sub" style="padding:12px 14px">Nothing matches.</p>'
  const note = '<p class="set-sub">Where Echo sends you things — Teams, mail and the ways plugins add — is in <a href="#settings/deliver">Sends to you</a>.</p>'
  return `<div class="pl-split"><div><section class="pl-list">${rows}</section>${note}</div>${plDetail(p, sel)}</div>`
}
/** The chosen installed plugin: where it comes from, what it may do, its settings, update and remove. */
function plDetail(p, x) {
  const busy = plBusy(p, `plugin:${x.id}`)
  const sec = Plugins.sections.find((s) => s.plugin === x.id)
  const upd = x.update ? (x.update.blocked ? `<div class="set-sub">${esc(x.update.version)}: ${esc(x.update.blocked)}</div>` : `<button class="btn" data-pl-update="${esc(x.id)}"${busy ? ' disabled' : ''}>${busy ? esc(busy) : `Update to ${esc(x.update.version)}`}</button>`) : ''
  const open = sec && x.enabled ? `<button class="btn primary" data-pl-open="${esc(sec.id)}">Open its settings</button>` : ''
  const rm = S.plRemove === x.id ? `<div class="confirm"><span>Remove ${esc(x.name)}? Its settings and data can stay for when you install it again.</span>
    <button class="btn" data-pl-remove-cancel>Cancel</button><button class="btn" data-pl-remove="${esc(x.id)}" data-keep="1">Remove, keep data</button><button class="btn primary" style="background:var(--danger);border-color:var(--danger)" data-pl-remove="${esc(x.id)}" data-keep="0">Remove with data</button></div>` : ''
  const fields = (x.settings || []).map((d) => setRow(esc(d.label), d.help ? esc(d.help) : '', plField(x.id, d)))
  let own = ''
  const card = Plugins.settings[x.id]
  if (card && x.enabled && !sec) { try { own = card.render() } catch (e) { own = `<p class="error">${esc(String(e))}</p>` } }
  const broke = Plugins.errors[x.id] ? `<p class="error">Its window part didn't load: ${esc(Plugins.errors[x.id])}</p>` : ''
  return `<section class="pl-detail">
    <div class="pl-dhead">${plTile(x, true)}<div style="flex:1;min-width:0"><div class="pl-dn">${esc(x.name)}</div><div class="set-sub">${esc(x.version)} · from <b>${esc(x.market)}</b>${x.market_gone ? ' (removed — no updates)' : ''} · commit <code>${esc((x.commit || '').slice(0, 10))}</code></div></div>
      <button class="sw${x.enabled ? ' on' : ''}" role="switch" aria-checked="${!!x.enabled}" aria-label="${esc(x.name)} on" data-pl-enable="${esc(x.id)}" data-on="${!x.enabled}"></button></div>
    ${x.summary ? `<p class="pl-sum">${esc(x.summary)}</p>` : ''}
    <div class="pl-acts">${open}${upd}<span class="grow"></span><button class="btn ghost danger" data-pl-ask-remove="${esc(x.id)}">Remove…</button></div>
    ${rm}${plErr(p, `plugin:${x.id}`)}${broke}
    ${fields.length ? setCard('Its settings', fields) : ''}${own}
    <div class="pl-h4">What it can do</div>${plUses(x.uses)}
    <p class="set-sub">You agreed to these when you installed it. An update that asks for more asks you again.</p>
  </section>`
}
function plDiscover(p) {
  const all = (p.discover || []).filter(plMatch)
  const cats = [...new Set((p.discover || []).flatMap((x) => x.categories || []))].sort()
  const cat = cats.includes(S.plCat) ? S.plCat : ''
  const list = all.filter((x) => !cat || (x.categories || []).includes(cat))
    .sort((a, b) => (b.featured - a.featured) || (b.built_in - a.built_in) || a.name.localeCompare(b.name))
  const chips = cats.length ? `<div class="pl-cats">${['', ...cats].map((k) => `<button class="pl-cat${k === cat ? ' on' : ''}" data-pl-cat="${esc(k)}">${esc(k ? k[0].toUpperCase() + k.slice(1) : 'All')}</button>`).join('')}</div>` : ''
  const reading = plBusy(p, 'markets') ? '<p class="set-sub">Reading marketplaces…</p>' : ''
  if (!list.length) return chips + reading + `<p class="set-sub">${S.plQuery || cat ? 'Nothing matches.' : 'Your marketplaces offer nothing yet' + ((p.markets || []).some((m) => m.error) ? ' — one can\'t be read (Marketplaces).' : '.')}</p>`
  const card = (x) => {
    const busy = plBusy(p, `plugin:${x.id}`)
    const act = x.installed === true ? '<span class="set-ok">Installed ✓</span>'
      : x.installed === false ? '<span class="set-sub">Installed from another marketplace</span>'
      : x.why_not ? `<span class="tag" title="${esc(x.why_not)}">${!x.version ? "Can't be read" : 'Needs a newer Echo'}</span>`
      : `<button class="btn primary" data-pl-review="${esc(x.id)}" data-market="${esc(x.market)}"${busy ? ' disabled' : ''}>${busy ? esc(busy) : 'Install'}</button>`
    const cats2 = (x.categories || []).map((k) => k[0].toUpperCase() + k.slice(1)).join(' · ')
    return `<div class="pl-card">${''}<div class="pl-ctop">${plTile(x)}<div style="flex:1;min-width:0"><div class="pl-n">${esc(x.name)}</div><div class="pl-cat2">${esc(cats2)}</div></div><span class="tag${x.built_in ? ' rec' : ''}">${esc(x.market)}</span></div>
      <div class="pl-cs">${esc(x.summary || '')}</div><div class="pl-cact">${act}${x.version ? `<span class="pl-v">${esc(x.version)}</span>` : ''}</div>${plErr(p, `plugin:${x.id}`)}</div>`
  }
  const featured = !S.plQuery && !cat ? list.filter((x) => x.featured) : []
  const rest = list.filter((x) => !featured.includes(x))
  return chips + reading + (featured.length ? `<div class="pl-h4">Featured</div><div class="pl-grid two">${featured.map(card).join('')}</div>` : '') +
    (rest.length ? `${featured.length ? '<div class="pl-h4">All plugins</div>' : ''}<div class="pl-grid">${rest.map(card).join('')}</div>` : '')
}
/** The permission sheet, over the page: what the plugin will be able to do, in Echo's words, before anything
 *  is installed — and who listed it when Echo didn't. */
function plSheet(sh) {
  const r = sh.review
  const box = (inner) => `<div class="pl-sheet" role="dialog" aria-modal="true">${inner}</div>`
  if (!r) return box('<span class="set-sub">Reading what it uses…</span>')
  if (r.error) return box(`<span class="error">${esc(r.error)}</span><div class="pl-sheet-btns"><button class="btn" data-pl-sheet-cancel>Close</button></div>`)
  const lines = sh.update ? r.new_uses : r.uses
  const outside = S.plugins && !(S.plugins.markets || []).some((m) => m.name === r.market && m.built_in)
  const who = outside ? `<div class="pl-warn"><b>!</b><span>Listed by ${esc(r.market)} — Echo hasn’t reviewed it. Install it only if you trust who made it.</span></div>` : ''
  const go = sh.update ? `Update to ${esc(r.version)}` : 'Install'
  return box(`<div class="pl-dhead">${plTile({ id: r.id, name: r.name })}<div style="flex:1"><div class="pl-dn">${sh.update ? `${esc(r.name)} ${esc(r.version)} asks for more` : `Install ${esc(r.name)} ${esc(r.version)}?`}</div>
      <div class="set-sub">from <b>${esc(r.market)}</b> · commit <code>${esc((r.commit || '').slice(0, 10))}</code></div></div></div>
    ${who}<div class="pl-h4">It will be able to</div>${plUses(lines)}
    ${r.blocked ? `<div class="error">${esc(r.blocked)}</div>` : ''}
    <div class="pl-sheet-btns"><button class="btn${outside ? ' primary' : ''}" data-pl-sheet-cancel${outside ? ' autofocus' : ''}>Cancel</button>${r.blocked ? '' : `<button class="btn${outside ? '' : ' primary'}" data-pl-sheet-go>${go}</button>`}</div>`)
}
function plMarkets(p) {
  const rows = p.markets.map((m) => {
    const busy = plBusy(p, `market:${m.name}`)
    const state = busy ? esc(busy) : m.error ? `<span class="error">Can’t be read: ${esc(m.error)}</span>` : m.fetched_at ? `${m.count} plugin${m.count === 1 ? '' : 's'} · read ${esc(relDay(String(m.fetched_at).slice(0, 10)))}` : 'Not read yet'
    const tags = `${m.built_in ? ' <span class="tag rec">built in</span>' : ''}${m.hidden ? ' <span class="tag">hidden</span>' : ''}`
    const menu = S.plMenu === m.name ? `<div class="pl-pop"><button data-pl-mhide="${esc(m.name)}" data-hidden="${!m.hidden}">${m.hidden ? 'Show its plugins in Discover' : 'Hide its plugins from Discover'}</button>${m.built_in ? '' : `<button class="danger" data-pl-mremove="${esc(m.name)}">Remove — installed plugins stay, without updates</button>`}</div>` : ''
    return `<div class="pl-mrow">${plTile({ id: m.name, name: m.name })}<div class="pl-txt"><div class="pl-n">${esc(m.name)}${m.title && m.title !== m.name ? ` <span class="set-sub">(${esc(m.title)})</span>` : ''}${tags}</div><div class="pl-s"><code>${esc(m.source)}</code></div><div class="pl-s">${state}</div></div>
      <button class="btn" data-pl-mrefresh="${esc(m.name)}"${busy ? ' disabled' : ''}>Read again</button><span class="pl-menu"><button class="btn pl-ib" data-pl-mmenu="${esc(m.name)}" aria-label="More">⋯</button>${menu}</span></div>`
  })
  const add = `<section class="pl-add"><b>Add a marketplace</b>
    <div class="pl-addrow"><input class="set-in" id="pl-msrc" placeholder="owner/repo, a git address, or a folder" aria-label="Marketplace" spellcheck="false"><input class="set-in" id="pl-mname" placeholder="Name (optional)" aria-label="Name" spellcheck="false"><button class="btn primary" data-pl-madd>Add</button></div>
    <div class="set-sub">It needs <code>echo-marketplace.json</code> at its root. Private repositories use your own git sign-in. Plugins from it say who listed them when you install.</div></section>`
  return `<p class="set-sub">Where plugins come from. Echo’s own is built in; add your team’s or company’s catalog — a GitHub repository, any git address, or a folder.</p>
    <section class="pl-list pl-mlist">${rows.join('')}</section>${Object.keys(p.errors || {}).filter((k) => k.startsWith('market:')).map((k) => plErr(p, k)).join('')}${add}`
}
// The page's own controls: tabs, install / update (through the sheet), switch, remove, marketplaces, search.
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-pl-tab],[data-pl-enable],[data-pl-update],[data-pl-review],[data-pl-sheet-go],[data-pl-sheet-cancel],[data-pl-ask-remove],[data-pl-remove],[data-pl-remove-cancel],[data-pl-mrefresh],[data-pl-mhide],[data-pl-mremove],[data-pl-madd],[data-pl-sel],[data-pl-cat],[data-pl-mmenu],[data-pl-open]')
  if (!t) { if (S.plMenu) { S.plMenu = null; render() } return }
  const d = t.dataset
  // The sheet's backdrop closes it; a click inside the sheet doesn't.
  if ('plSheetCancel' in d && t.classList.contains('pl-scrim') && e.target.closest('.pl-sheet')) return
  if (d.plTab) { S.plTab = d.plTab; S.plSheet = null; S.plMenu = null; return render() }
  if (d.plEnable) { e.stopPropagation(); return pluginOp('enable', { id: d.plEnable, on: d.on === 'true' }) }
  if (d.plSel) { S.plSel = d.plSel; S.plRemove = null; return render() }
  if (d.plCat != null && t.classList.contains('pl-cat')) { S.plCat = d.plCat; return render() }
  if (d.plOpen) return go(`settings/${d.plOpen}`)
  if (d.plMmenu) { S.plMenu = S.plMenu === d.plMmenu ? null : d.plMmenu; return render() }
  if (d.plMhide || d.plMremove) S.plMenu = null
  if (d.plReview || d.plUpdate) {
    const id = d.plReview || d.plUpdate, market = d.market || ''
    S.plSheet = d.plUpdate ? { update: id, review: (S.plugins.installed.find((x) => x.id === id) || {}).update } : { install: id, market, review: null }
    render()
    if (d.plReview) get(`api/plugins/review?id=${encodeURIComponent(id)}&market=${encodeURIComponent(market)}`).then((r) => { if (S.plSheet && S.plSheet.install === id) { S.plSheet.review = r; render() } })
    return
  }
  if ('plSheetCancel' in d) { S.plSheet = null; return render() }
  if ('plSheetGo' in d) {
    const sh = S.plSheet
    S.plSheet = null
    if (sh && sh.review) pluginOp(sh.update ? 'update' : 'install', { id: sh.review.id, market: sh.review.market, agreed: sh.update ? sh.review.new_uses : sh.review.uses })
    return render()
  }
  if (d.plAskRemove) { S.plRemove = d.plAskRemove; return render() }
  if ('plRemoveCancel' in d) { S.plRemove = null; return render() }
  if (d.plRemove) { S.plRemove = null; return pluginOp('remove', { id: d.plRemove, keep_data: d.keep === '1' }) }
  if (d.plMrefresh) return pluginOp('market_refresh', { name: d.plMrefresh })
  if (d.plMhide) return pluginOp('market_hide', { name: d.plMhide, hidden: d.hidden === 'true' })
  if (d.plMremove) return pluginOp('market_remove', { name: d.plMremove })
  if ('plMadd' in d) {
    const source = ($('#pl-msrc') || {}).value || '', name = ($('#pl-mname') || {}).value || ''
    if (source.trim()) pluginOp('market_add', { source: source.trim(), name: name.trim() })
  }
})
document.addEventListener('input', (e) => {
  if (e.target.id !== 'pl-q') return
  S.plQuery = e.target.value
  const pos = e.target.selectionStart
  render()
  const q = $('#pl-q')
  if (q) { q.focus(); q.setSelectionRange(pos, pos) }
})
const plUses = (uses) => (uses || []).length ? `<ul class="pl-uses">${uses.map((u) => `<li>${esc(u)}</li>`).join('')}</ul>` : '<div class="set-sub">Uses nothing of Echo\'s.</div>'
const plBusy = (p, key) => p.busy && p.busy[key]
const plErr = (p, key) => p.errors && p.errors[key] ? `<div class="error">${esc(p.errors[key])}</div>` : ''
async function loadDeliver() {
  try { S.deliver = await get('api/deliver') } catch { /* app restarting */ }
}
function deliverOp(op, body = {}) {
  send({ cmd: 'deliver_op', op, ...body })
  setTimeout(() => loadDeliver().then(render), 500)
}
/** Where Echo sends things to you: your Teams chat with yourself, your mailbox — and the ways plugins add
 *  (Telegram…), each set up in its own plugin's section. */
function deliverCard(d) {
  const test = (k) => `<button class="btn ghost" data-dl-test="${esc(k)}">Send a test</button>`
  const ms = d.account ? `As ${esc(d.account)}.` : 'Sign in to Microsoft 365 first (Settings → Calendar &amp; chat).'
  const own = (d.provided || []).map((x) => {
    const sec = Plugins.sections.find((s) => s.plugin === x.plugin)
    return setRow(esc(x.label), x.ready ? `Ready — from the ${esc(x.plugin)} plugin.` : `${esc(x.why || 'Not set up yet')} — in its plugin’s section.`,
      (x.ready ? test(x.kind) : '') + (sec ? `<button class="btn${x.ready ? ' ghost' : ' primary'}" data-dl-go="${esc(sec.id)}">${x.ready ? 'Settings' : 'Set up…'}</button>` : ''))
  })
  return setCard('', [
    setRow('Teams', `Your chat with yourself. ${ms}`, d.teams ? test('teams') : ''),
    setRow('Mail', `Your own mailbox — or the addresses a plugin was given. ${ms}`, d.mail ? test('mail') : ''),
    ...own,
  ])
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-dl-test],[data-dl-go]')
  if (!t) return
  if (t.dataset.dlGo) return go(`settings/${t.dataset.dlGo}`)
  if (t.dataset.dlTest) return deliverOp('test', { kind: t.dataset.dlTest })
})

// ── Plugins' client halves (docs/plugins-design.md, v4) ──
// Each enabled plugin's `client` module is loaded into this page (trusted, as in dsh-desktop) and its
// `apply(ctx)` adds to Echo: buttons in Echo's own places (slots), panel views, its card in Settings →
// Plugins, ⌘K actions. `ctx.call` runs a command of its host half (`/api/plugin/<id>/call/<command>`).
// Everything a plugin added goes when it is switched off or removed (`unloadPlugin`).
const Plugins = {
  /** slot name → [{plugin, id, label, icon, when(target), run(target)}] */
  slots: {},
  /** plugin id → {render() → html} */
  settings: {},
  /** Plugins' own Settings sections: [{plugin, id, label, icon, badge(), render()}] */
  sections: [],
  /** [{plugin, label, run, group, keywords}] */
  palette: [],
  /** plugin id → its document listeners and style elements (taken out when it unloads). */
  owned: {},
  loaded: {},
  errors: {},
  /** What each slot was last drawn for: its buttons run on that. */
  targets: {},
}
const SLOTS = ['call.menu', 'calendar.event', 'task.actions']
function pluginCtx(id) {
  const add = (list, item) => { list.push({ ...item, plugin: id }); render() }
  const owned = Plugins.owned[id] = Plugins.owned[id] || { listeners: [], styles: [] }
  return {
    id,
    /** A document listener that goes when the plugin is switched off, updated or removed. */
    listen: (type, fn, opts) => { document.addEventListener(type, fn, opts); owned.listeners.push([type, fn, opts]) },
    call: async (command, args = {}) => {
      const r = await get(`api/plugin/${encodeURIComponent(id)}/call/${encodeURIComponent(command)}?args=${encodeURIComponent(JSON.stringify(args))}`)
      if (r && r.error) throw new Error(r.error)
      return r ? r.value : null
    },
    slots: {
      list: () => SLOTS.slice(),
      add: (slot, item) => { if (!SLOTS.includes(slot)) throw new Error(`Echo has no slot “${slot}” (${SLOTS.join(', ')})`); add(Plugins.slots[slot] = Plugins.slots[slot] || [], item) },
    },
    panels: { add: (v) => { PANEL_VIEWS.push({ area: 'right', ...v, id: `${id}:${v.id}`, plugin: id }); render() } },
    settings: {
      /** Its card in Settings → Plugins. */
      add: (v) => { Plugins.settings[id] = v; render() },
      /** A section of its own in Settings ("From plugins"); its id is `<plugin>:<id>` in the hash. */
      section: (v) => { Plugins.sections.push({ ...v, id: `${id}:${v.id || 'main'}`, plugin: id }); render() },
    },
    /** Open one of its panel views (`ctx.panels.add`). */
    showPanel: (view) => { Layout.show(`${id}:${view}`, panelViews()); layoutChanged() },
    /** Open one of Echo's own panel views, e.g. `claude-folder` (the Claude · Echo folder session). */
    openEchoPanel: (view) => { if (view.includes(':')) return; go('calls'); setTimeout(() => { Layout.show(view, panelViews()); layoutChanged() }, 50) },
    /** Where the window is: the activity and the Settings section (`#settings/<plugin>:<id>`). */
    where: () => ({ view: S.view, section: S.setSection }),
    go: (hash) => go(hash),
    palette: { add: (item) => add(Plugins.palette, item) },
    /** Echo's own pieces, so a plugin looks like Echo. */
    ui: {
      esc, setRow, setCard, sw, ICON, relDay, ago, notice: (text) => { S.notice = text; render() },
      /** Its CSS in the page, taken out when it unloads. */
      style: (css) => { const el = document.createElement('style'); el.dataset.plugin = id; el.textContent = String(css); document.head.appendChild(el); owned.styles.push(el) },
    },
    render: () => render(),
  }
}
/** Load the client halves of the enabled plugins not loaded yet (or at another version). */
async function loadPluginClients(snapshot) {
  let p = snapshot
  if (!p) try { p = await get('api/plugins') } catch { return }
  const want = (p.installed || []).filter((x) => x.enabled && x.client)
  for (const id of Object.keys(Plugins.loaded)) if (!want.some((x) => x.id === id && x.version === Plugins.loaded[id])) unloadPlugin(id)
  for (const x of want) {
    if (Plugins.loaded[x.id]) continue
    Plugins.loaded[x.id] = x.version
    try {
      const m = await import(new URL(`plugin/${encodeURIComponent(x.id)}/${x.client}?v=${encodeURIComponent(x.version)}`, location.href).href)
      if (typeof m.apply !== 'function') throw new Error(`${x.client} exports no apply(ctx)`)
      await m.apply(pluginCtx(x.id))
      delete Plugins.errors[x.id]
    } catch (e) { Plugins.errors[x.id] = String((e && e.message) || e) }
  }
  render()
}
function unloadPlugin(id) {
  const owned = Plugins.owned[id]
  if (owned) {
    for (const [type, fn, opts] of owned.listeners) document.removeEventListener(type, fn, opts)
    for (const el of owned.styles) el.remove()
    delete Plugins.owned[id]
  }
  for (const k of Object.keys(Plugins.slots)) Plugins.slots[k] = Plugins.slots[k].filter((x) => x.plugin !== id)
  Plugins.palette = Plugins.palette.filter((x) => x.plugin !== id)
  Plugins.sections = Plugins.sections.filter((x) => x.plugin !== id)
  delete Plugins.settings[id]
  for (let i = PANEL_VIEWS.length - 1; i >= 0; i--) if (PANEL_VIEWS[i].plugin === id) PANEL_VIEWS.splice(i, 1)
  delete Plugins.loaded[id]
}
/** A slot's buttons for `target`: `menu` (menu items) or `buttons`. */
function slotItems(slot, target, style = 'buttons') {
  const items = (Plugins.slots[slot] || []).filter((x) => { try { return !x.when || x.when(target) } catch { return false } })
  if (!items.length) return ''
  Plugins.targets[slot] = target
  return items.map((x, i) => style === 'menu'
    ? `<button class="menu-item" role="menuitem" data-plug-slot="${slot}" data-plug-i="${i}">${x.icon || ''}${esc(x.label)}</button>`
    : `<button class="btn" data-plug-slot="${slot}" data-plug-i="${i}" title="${esc(x.title || x.label)}">${x.icon || ''}${esc(x.label)}</button>`).join('')
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-plug-slot]')
  if (!t) return
  const slot = t.dataset.plugSlot, target = Plugins.targets[slot]
  const items = (Plugins.slots[slot] || []).filter((x) => { try { return !x.when || x.when(target) } catch { return false } })
  const item = items[+t.dataset.plugI]
  S.menuOpen = false
  if (item) Promise.resolve().then(() => item.run(target)).catch((err) => { S.notice = `${item.label}: ${(err && err.message) || err}`; render() })
})

/** The transcription server row: an Echo, or an OpenAI-compatible server (Ollama with an audio
 *  model, Speaches, vLLM, LocalAI, parakeet-api…) and what it was found to give. */
function serverLine(srv) {
  const i = srv.info
  if (!srv.client || !i) return "Send each utterance to another Echo on your network, or to an OpenAI-compatible speech-to-text server (Ollama with an audio model, Speaches, vLLM, LocalAI…); this computer's model takes over when it's down."
  if (i.api === 'openai') return `Transcribing on <code>${esc(i.url)}</code> with <b>${esc(i.model || '')}</b> (OpenAI-compatible)${i.words === false ? ' — it gives no word timings: speaker turns split by sentence only' : ''}. This computer's model takes over when it's down.`
  return `Connected to ${esc(srv.client)}; this computer's model takes over when it's down.`
}
function sttForm(srv) {
  const c = srv.connecting || {}
  const f = S.sttForm
  const msg = c.busy ? '<span class="set-sub">Checking the server — its models, then a test transcription…</span>' : c.error ? `<span class="error">${esc(c.error)}</span>` : ''
  return setRow('OpenAI-compatible server', `Its address (it must answer <code>/v1/models</code> and <code>/v1/audio/transcriptions</code>), the model to ask for (empty: the first speech model it lists) and a key only if it wants one.` +
    `<div class="stt-form"><input class="set-in" id="stt-url" value="${esc(f.url)}" placeholder="http://localhost:11434" aria-label="Server address" spellcheck="false">` +
    `<input class="set-in" id="stt-model" value="${esc(f.model)}" placeholder="model — e.g. lfm3-audio, whisper-1" aria-label="Model" spellcheck="false">` +
    `<input class="set-in" id="stt-key" type="password" placeholder="key (optional)" aria-label="Key">` +
    `<button class="btn primary" data-stt-connect${c.busy ? ' disabled' : ''}>Connect</button></div>${msg ? `<div>${msg}</div>` : ''}`, '')
}
document.addEventListener('click', (e) => {
  if (e.target.closest && e.target.closest('[data-stt-form]')) { S.sttForm = S.sttForm ? null : { url: 'http://localhost:11434', model: '' }; return render() }
  if (!(e.target.closest && e.target.closest('[data-stt-connect]'))) return
  const url = ($('#stt-url') || {}).value || '', model = ($('#stt-model') || {}).value || '', key = ($('#stt-key') || {}).value || ''
  S.sttForm = { url, model }
  send({ cmd: 'server_openai', url, model, key })
  const poll = () => loadSettings().then(() => {
    const c = (S.settings && S.settings.server && S.settings.server.connecting) || {}
    if (c.busy) return setTimeout(poll, 1000)
    if (c.ok) S.sttForm = null
    render()
  })
  setTimeout(poll, 500)
})

const SETTINGS_BODY = {
  brain: brainSettings,
  plugins: pluginsSettings,
  general(st) {
    const upd = st.update ? actBtn('update', `Update to ${st.update}…`, '', true) : actBtn('check_updates', 'Check for updates')
    const plugin = (S.state && S.state.notes) || {}
    return `<h2>General</h2>` + setCard('', [
      setRow('Data folder', `<code>${esc(st.home)}</code> — calls, transcripts and minutes live here.`, actBtn('change_home', 'Change…')),
      setRow('Start at login', 'Echo runs in the menu bar from when you log in.', sw('autostart', st.autostart, 'Start at login')),
      setRow('Command line', st.cli ? '<code>echoctl</code> is on your PATH.' : 'Install <code>echoctl</code> so scripts and Claude Code can drive Echo.', st.cli ? '<span class="set-ok">Installed ✓</span>' : actBtn('install_cli', 'Install…')),
      setRow('Claude Code plugin', "Echo's skills in your own Claude Code: record, minutes, doctor.", plugin.plugin ? '<span class="set-ok">Installed ✓</span>' : actBtn('connect_claude', 'Connect')),
      setRow(`Echo ${esc(st.version)}`, st.update ? `Version ${esc(st.update)} is available.` : 'Echo checks for updates every 6 hours.', upd),
    ])
  },
  recording(st) {
    const langs = st.languages || []
    const chips = langs.map((l) => `<span class="chip">${esc(langName(l))}<button data-lang-del="${esc(l)}" aria-label="Remove ${esc(langName(l))}">×</button></span>`).join('')
    const add = `<label class="btn split-lang pick">+ Add<select data-lang-add aria-label="Add a language"><option value=""></option>${LANG_LIST.filter((l) => !langs.includes(l)).map((l) => `<option value="${l}">${esc(langName(l))}</option>`).join('')}</select></label>`
    const cur = recordLang()
    const frames = st.capture_frames && st.frames_permitted === false ? ` <a href="#" data-act="ask_permissions">Grant…</a>` : ''
    return `<h2>Recording</h2><p class="set-lead">Echo records only the calls you choose, and keeps only text.</p>` +
      setCard('When to record', [
        setRow('Calendar meetings', 'Ask: Echo asks when one starts. Manual: you press Record (or “Record when it starts” on the calendar).', seg('scheduled', st.scheduled, [['ask', 'Ask'], ['manual', 'Manual']])),
        setRow('Other Teams calls', 'Calls not on your calendar.', seg('unscheduled', st.unscheduled, [['ask', 'Ask'], ['manual', 'Manual']])),
      ]) +
      setCard('Languages', [
        setRow('Languages your calls are in', `Detection picks only among these — it keeps Ukrainian and Russian apart.<div class="chips">${chips}${add}</div>`, ''),
        setRow('New recordings', "The Record button's language unless you pick another.", `<div class="seg" role="group"><button data-reclang-set="en" aria-pressed="${cur === 'en'}">English</button><button data-reclang-set="" aria-pressed="${cur === ''}">Detect</button></div>`),
      ]) +
      setCard('During a call', [
        setRow("Follow Teams' mute", "Your track records silence while you're muted in Teams.", sw('follow_app_mute', st.follow_app_mute, "Follow Teams' mute")),
        setRow('Names from the Teams window', 'Who is speaking, read from the call window (Accessibility).', sw('capture_ui', st.capture_ui, 'Names from the Teams window')),
        setRow('Meeting chat', "The meeting's Teams chat, with Microsoft 365 connected.", sw('capture_chat', st.capture_chat, 'Meeting chat')),
        setRow('Shared screens', `Key frames while someone else shares (needs Screen Recording).${frames}`, sw('capture_frames', st.capture_frames, 'Shared screens')),
      ])
  },
  transcription(st) {
    const m = st.models || { models: [] }
    const dl = st.downloading
    const rows = (m.models || []).map((x) => {
      const tags = [x.active ? '<span class="tag use">In use</span>' : '', x.recommended ? '<span class="tag rec">Recommended for this computer</span>' : '', x.runnable === false ? '<span class="tag">Not in this build</span>' : ''].join('')
      let ctl
      if (dl && dl.name === x.name) ctl = `<span class="set-sub">Downloading… ${dl.pct}%</span>`
      else if (x.downloaded) ctl = (x.active || x.runnable === false ? '' : actBtn('use_model', 'Use', x.name)) + (x.active ? '' : actBtn('remove_model', 'Remove', x.name))
      else if (x.runnable === false) ctl = ''
      else ctl = dl ? '<span class="set-sub">Another download is running</span>' : actBtn('download_model', `Download (${x.size_mb} MB)`, x.name)
      const bar = dl && dl.name === x.name ? `<div class="bar"><span style="width:${dl.pct}%"></span></div>` : ''
      return setRow(`${esc(x.name)} ${tags}`, `${esc(x.summary || '')} · ${x.size_mb} MB${bar}`, ctl)
    })
    const srv = st.server || null
    return `<h2>Transcription</h2><p class="set-lead">On this computer — or on one you connect to yourself. Never a cloud service.${st.engine ? ` Speech engine: ${esc(st.engine)}.` : ''}</p>` +
      setCard('Speech models', rows) +
      (srv ? setCard('Transcription server', [
        setRow('Use a server', serverLine(srv), srv.client ? actBtn('disconnect_server', 'Disconnect') : actBtn('connect_server', 'Echo server…') + `<button class="btn" data-stt-form>OpenAI-compatible…</button>`),
        ...(!srv.client && S.sttForm ? [sttForm(srv)] : []),
        setRow('Transcribe for other computers', srv.serving ? `Shared at <code>${esc(srv.url || '')}</code> — key <code>${esc(srv.key || '')}</code>` : "Share this computer's model on your network.", `<button class="sw${srv.serving ? ' on' : ''}" role="switch" aria-checked="${!!srv.serving}" aria-label="Transcribe for other computers" data-act="toggle_serving"></button>`),
      ]) : '') +
      setCard('Audio', [
        setRow('Echo cancellation on speakers', "Removes the others' voices from your mic when you don't wear headphones.", sw('echo_cancel', st.echo_cancel, 'Echo cancellation')),
        setRow('Keep audio files', 'Off: the recordings are deleted once the transcript is written — text only.', sw('keep_audio', st.keep_audio, 'Keep audio files')),
      ])
  },
  ai(st) {
    const a = ai()
    const kind = (p) => p.kind === 'claude' ? 'Claude account' : p.kind === 'local' ? `Model · ${esc(p.host || 'this computer')}` : 'Built in'
    const rows = a.profiles.map((p) => {
      const s = p.signin || {}
      const out = p.kind === 'claude' && s.state === 'signed_out'
      const who = p.kind === 'claude' ? (s.account ? `Signed in as ${esc(s.account)}` : out ? '<span class="warn">Not signed in — its calls wait for it</span>' : esc(p.account || '')) : esc(profileWhere(p))
      const isDef = p.id === a.default
      const ctl = (out ? `<button class="btn primary" data-signin="${esc(p.id)}">Sign in…</button>` : '') + (isDef ? '' : `<button class="btn" data-set="default_profile" data-val='${esc(JSON.stringify(p.id))}'>Make default</button>`)
      return setRow(`${esc(p.name)} ${isDef ? '<span class="tag rec">Default for new calls</span>' : ''}<span class="tag">${kind(p)}</span>`, `${who}${p.notes_model ? ` · minutes: ${esc(p.notes_model)}` : ''}${p.live_model ? ` · live notes: ${esc(p.live_model)}` : ''}`, ctl)
    })
    const add = '<div class="set-sub" style="padding:10px 14px">Add a profile: <code>echoctl ai add-claude &lt;name&gt;</code> (a Claude account) or <code>echoctl ai add-local &lt;name&gt; --url …</code> (a model on this computer) — or ask Claude in the Echo folder session.</div>'
    return `<h2>AI &amp; minutes</h2><p class="set-lead">Which model and account writes each call's live notes and minutes. A call keeps the profile it was recorded with.</p>` +
      `<section class="set-sec"><h3>Profiles</h3><div class="set-card">${rows.join('')}${add}</div></section>` +
      setCard('Minutes', [
        setRow('Write minutes automatically', "15 min after a call ends (or after your last note), with the call's profile.", sw('auto_minutes', st.auto_minutes, 'Write minutes automatically')),
        setRow("Keep threads' work current", "After a thread's calls, Claude reads them into its stories, tasks, decisions, risks and questions — with the thread's AI profile.", sw('project_sync', st.project_sync, "Keep threads' work current")),
        setRow('Live notes during calls', "A running summary and the Live tab's questions, with the call's profile.", sw('live_notes', a.live_notes, 'Live notes')),
        setRow('Minutes model', 'The model that writes minutes for the default profile.', actBtn('notes_model', 'Change…')),
        setRow('Where Claude Code runs for minutes', st.notes_in ? `<code>${esc(st.notes_in)}</code> — its .claude/settings.json applies.` : 'The data folder.', actBtn('notes_choose', 'Choose…') + (st.notes_in ? actBtn('notes_anywhere', 'Use the data folder') : '')),
        setRow('Also write a minutes page file', "An HTML file in the call's folder, for sharing.", sw('minutes_page', st.minutes_page, 'Minutes page file')),
      ]) +
      setCard('Claude in the window', [
        setRow('Echo folder session: no permission prompts', `Starts it with <code>--dangerously-skip-permissions</code>: Claude runs any command and tool without asking — it can talk to your other sessions and close your tasks in other systems. ${st.folder_skip_permissions ? '<span class="warn">On.</span> ' : ''}Takes effect the next time the session starts. A call's session stays guarded.`, sw('folder_skip_permissions', st.folder_skip_permissions, 'Echo folder session: no permission prompts')),
      ])
  },
  calendar(st) {
    return `<h2>Calendar &amp; chat</h2><p class="set-lead">Read-only, through Echo's own Microsoft 365 sign-in.</p>` + setCard('Microsoft 365', [
      setRow('Account', st.calendar ? `Signed in as ${esc(st.calendar)}` : 'Not connected: the calendar shows only what your tray knows about today.', st.calendar ? actBtn('calendar_sync', 'Sync now') + actBtn('calendar_logout', 'Sign out') : actBtn('calendar_login', 'Sign in…', '', true)),
      setRow('Meeting chat', "Each recorded meeting's Teams chat, kept with the call.", sw('capture_chat', st.capture_chat, 'Meeting chat')),
      setRow('App registration', st.graph_custom ? "Your organisation's own app." : 'The default (CodeMie). Your IT can register another.', actBtn('calendar_client_id', 'Change…')),
    ])
  },
  deliver() {
    if (!S.deliver) { if (!S.deliverLoading) { S.deliverLoading = true; loadDeliver().then(() => { S.deliverLoading = false; render() }) } return '<h2>Sends to you</h2><div class="skeleton" style="padding:20px"><i></i><i></i></div>' }
    return `<h2>Sends to you</h2><p class="set-lead">Where Echo sends you things — routines’ answers, tests. Only to you: your own Teams chat, your mailbox, and the ways plugins add.</p>${deliverCard(S.deliver)}`
  },
  me(st) {
    const me = st.me || {}
    return `<h2>About me</h2><p class="set-lead">So Echo knows which voice is you, whose work is whose, and how to write to you.</p>` + setCard('', [
      setRow('Name', '', `<input class="search set-input" id="me-name" value="${esc(me.name || '')}" placeholder="Your name" aria-label="Your name">`),
      setRow('Email', 'Tells your colleagues from external people.', `<input class="search set-input" id="me-email" value="${esc(me.email || '')}" placeholder="you@company.com" aria-label="Your email">`),
    ]) + meProfileCard(st.me_profile)
  },
  layout() {
    const known = PANEL_VIEWS
    const rows = known.filter((v) => v.id !== 'shell').map((v) => {
      const area = Layout.area(v.id, known)
      return setRow(`${v.icon || ''} ${esc(v.label)}`, '', `<div class="seg" role="group">${['right', 'bottom'].map((a) => `<button data-layout-move="${v.id}" data-to="${a}" aria-pressed="${area === a}">${a === 'right' ? 'Right' : 'Bottom'}</button>`).join('')}</div>`)
    })
    return `<h2>Layout</h2><p class="set-lead">Where each view opens. Also from a panel's own move button; drag a panel's edge to resize it.</p>` + setCard('Panel views', rows) +
      setCard('', [setRow('Reset the layout', 'Every view back to its own place and size.', '<button class="btn" data-layout-reset>Reset</button>')])
  },
  health(st) {
    const rows = (st.health || []).map((c) => {
      const ok = c.status === 'granted'
      const mark = ok ? '<span class="set-ok">✓</span>' : c.status === 'denied' ? '<span class="warn">✗</span>' : '<span class="set-sub">?</span>'
      return setRow(`${mark} ${esc(c.name)}`, ok ? esc(c.why) : `${esc(c.why)}<div class="set-sub">${esc(c.how)}</div>`, ok || !c.url ? '' : actBtn('open_settings_url', 'Open settings', c.url))
    })
    const stale = st.stale_plugins || []
    const plugin = stale.length ? setCard('Claude Code', [setRow('<span class="warn">✗</span> An older Echo plugin', `${stale.map((s) => `${esc(s.version)} (${esc(s.scope)}${s.project ? ', ' + esc(String(s.project).split(/[\\/]/).pop()) : ''})`).join(', ')} — its terminal pane can hold calls back from Write minutes. Restart Claude Code after updating.`, '<button class="btn" data-cmd-ipc="update_plugins">Update it</button>')]) : ''
    return `<h2>Health</h2><p class="set-lead">What Echo needs from this computer.</p>` + plugin + setCard('Permissions', rows) +
      setCard('', [setRow('Ask again', 'macOS asks for each permission Echo is missing.', actBtn('ask_permissions', 'Ask for permissions') + actBtn('recheck', 'Check again'))])
  },
}

// ── Projects (`projects.rs`, docs/projects-design.md) ─────────────────────────────────────────
/** Symbols a project can wear instead of its identicon (24-unit strokes, drawn in its colour). */
const PJ_SYMBOLS = {
  rocket: '<path d="M12 3c3 2 5 5.5 5 9.5l-2.5 3h-5L7 12.5C7 8.5 9 5 12 3z"/><circle cx="12" cy="10" r="1.6"/><path d="M9.5 15.5 7 19l3-.8M14.5 15.5 17 19l-3-.8"/>',
  database: '<ellipse cx="12" cy="6" rx="7" ry="2.5"/><path d="M5 6v12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5V6M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5"/>',
  shield: '<path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="M9 12l2 2 4-4"/>',
  people: '<circle cx="9" cy="9" r="3"/><path d="M3.5 19c.6-3 2.8-5 5.5-5s4.9 2 5.5 5"/><circle cx="17" cy="8" r="2.4"/><path d="M15.5 13.3c2.4.2 4.3 2 4.8 4.7"/>',
  chart: '<path d="M4 20V4M4 20h16"/><path d="M8 16v-4M12 16V8M16 16v-6"/>',
  cloud: '<path d="M7 18h10a4 4 0 0 0 .5-8 5.5 5.5 0 0 0-10.6 1.5A3.3 3.3 0 0 0 7 18z"/>',
  code: '<path d="M9 8l-4 4 4 4M15 8l4 4-4 4M13 6l-2 12"/>',
  briefcase: '<rect x="3.5" y="7.5" width="17" height="12" rx="2"/><path d="M9 7.5V6a1.5 1.5 0 0 1 1.5-1.5h3A1.5 1.5 0 0 1 15 6v1.5M3.5 12.5h17"/>',
  flask: '<path d="M10 3.5h4M10.5 3.5v6L5 18.5a1.3 1.3 0 0 0 1.1 2h11.8a1.3 1.3 0 0 0 1.1-2l-5.5-9v-6"/><path d="M7.5 15h9"/>',
  globe: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.4 2.4 3.6 5.2 3.6 8.5s-1.2 6.1-3.6 8.5c-2.4-2.4-3.6-5.2-3.6-8.5S9.6 5.9 12 3.5z"/>',
  cart: '<path d="M3.5 4.5h2.5l2 11h10l2-8H7"/><circle cx="9.5" cy="19" r="1.3"/><circle cx="16.5" cy="19" r="1.3"/>',
  book: '<path d="M5 4.5h10.5a2.5 2.5 0 0 1 2.5 2.5v13H7.5A2.5 2.5 0 0 1 5 17.5z"/><path d="M5 17.5A2.5 2.5 0 0 1 7.5 15H18"/>',
}
const PJ_COLORS = ['#9b78ff', '#5aa9ff', '#3ccf91', '#f0a24a', '#e8603c', '#d36bd1', '#4cc3c8', '#c9b458']
/** A project's tile: its logo, else its symbol, else its identicon — `size` px square. */
function projIcon(p, size = 18) {
  const color = /^#[0-9a-f]{6}$/i.test(p.color || '') ? p.color : PJ_COLORS[0]
  const t = `class="pj-ic" style="--pj:${color};width:${size}px;height:${size}px" title="${esc(p.name || '')}"`
  if (p.has_logo) return `<img ${t} src="api/project/${encodeURIComponent(p.id)}/logo?v=${esc(p.logo_v || 0)}" alt="">`
  if (p.icon === 'client' && p.client && p.client_logo_v) return `<img ${t} src="api/client/${encodeURIComponent(p.client)}/logo?v=${esc(p.client_logo_v)}" alt="">`
  const sym = PJ_SYMBOLS[p.icon]
  if (sym) return `<span ${t} aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${sym}</svg></span>`
  const cells = ProjectArt.cells(p.id || p.name || '')
  const rects = cells.map((on, i) => on ? `<rect x="${i % 5}" y="${Math.floor(i / 5)}" width="1" height="1"/>` : '').join('')
  return `<span ${t} aria-hidden="true"><svg viewBox="-0.75 -0.75 6.5 6.5" fill="currentColor" shape-rendering="crispEdges">${rects}</svg></span>`
}
/** A client's logo (nothing when it has none). */
const clientIcon = (c, size = 16) => c && c.logo_v ? `<img class="pj-ic" style="width:${size}px;height:${size}px" src="api/client/${encodeURIComponent(c.id)}/logo?v=${esc(c.logo_v)}" alt="" title="${esc(c.name)}">` : ''
/** A logo file the user chose or dropped → the app (as base64), after a size check here. */
function sendLogo(file, op, id) {
  if (!file) return
  if (!/^image\/(png|jpeg|webp)$/.test(file.type)) return window.echo.notice('A logo is a PNG, JPEG or WebP image.')
  if (file.size > 512 * 1024) return window.echo.notice('A logo is at most 512 KB.')
  const r = new FileReader()
  r.onload = () => send({ cmd: 'project', op, id, data: String(r.result).replace(/^data:[^,]*,/, '') })
  r.readAsDataURL(file)
}
const logoPicker = (op, id, label) => `<label class="btn pj-file">${esc(label)}<input type="file" accept="image/png,image/jpeg,image/webp" data-logo-op="${op}" data-id="${esc(id)}" aria-label="${esc(label)}"></label>`
const stageTag = (id, long = false) => { const st = ProjectArt.stage(id); return `<span class="pj-stage ${esc(st.gid)}">${esc(long && st.group ? `${st.group} · ${st.label}` : st.label)}</span>` }
const projClient = (id) => ((S.pj.data && S.pj.data.clients) || []).find((c) => c.id === id)

function projectsView() {
  const data = S.pj.data
  if (!data) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  return `<div class="projects"><aside class="pj-side">${projectList(data)}</aside><div class="pj-main">${projectMain(data)}</div></div>`
}

/** The Threads list's clients folded by the user (an accordion each; remembered in the webview). */
const PJ_FOLD_KEY = 'echo.threads.folded'
const pjFolded = (() => { try { return new Set(JSON.parse(localStorage.getItem(PJ_FOLD_KEY) || '[]')) } catch { return new Set() } })()
function pjFold(id) {
  if (pjFolded.has(id)) pjFolded.delete(id); else pjFolded.add(id)
  try { localStorage.setItem(PJ_FOLD_KEY, JSON.stringify([...pjFolded])) } catch { /* storage off */ }
}
function projectList(data) {
  const ps = data.projects || []
  const clients = data.clients || []
  const row = (p) => {
    const last = p.last_call ? dayLabel(new Date(p.last_call)) : ''
    return `<button class="pj-row${p.id === S.pj.sel ? ' on' : ''}" data-pj="open" data-id="${esc(p.id)}">${projIcon(p, 20)}<span class="pj-row-t"><b>${esc(p.name)}</b><span class="pj-row-m">${p.call_count} call${p.call_count === 1 ? '' : 's'}${last ? ` · ${esc(last)}` : ''}</span></span>${p.group !== 'closed' ? stageTag(p.stage) : ''}</button>`
  }
  const open = ps.filter((p) => p.group !== 'closed'), closed = ps.filter((p) => p.group === 'closed')
  const groups = []
  // Each client an accordion: the chevron folds it (the open thread stays in view), the name opens its page.
  const fold = (id, xs, name) => {
    const folded = pjFolded.has(id), shown = folded ? xs.filter((p) => p.id === S.pj.sel) : xs
    // Nothing to fold: a spacer keeps the names in line.
    const chev = !xs.length ? '<span class="pj-chev" aria-hidden="true"></span>' : `<button class="pj-chev" data-pj="fold" data-id="${esc(id)}" aria-expanded="${!folded}" title="${folded ? 'Show' : 'Hide'} its threads" aria-label="${folded ? 'Show' : 'Hide'} its threads">${ICON(folded ? '<path d="M9 6l6 6-6 6"/>' : '<path d="M6 9l6 6 6-6"/>')}</button>`
    return `<div class="pj-ghead${folded ? ' folded' : ''}">${chev}${name}${folded && xs.length ? `<span class="pj-gn">${xs.length}</span>` : ''}</div>${shown.map(row).join('')}`
  }
  for (const c of [...clients.filter((c) => !c.internal), ...clients.filter((c) => c.internal)]) {
    const xs = open.filter((p) => p.client === c.id)
    groups.push(fold(c.id, xs, `<button class="pj-group pj-client${S.pj.client === c.id ? ' on' : ''}" data-pj="client" data-id="${esc(c.id)}" title="${esc(c.name)}: its details">${clientIcon(c, 14)}<span class="pj-cn">${esc(c.internal ? `${c.name} · your company` : c.name)}</span></button>`))
  }
  // Folds of clients that are gone are forgotten.
  if (clients.length && !data.error) for (const k of [...pjFolded]) if (k !== '-' && !clients.some((c) => c.id === k)) pjFold(k)
  const none = open.filter((p) => !p.client || !clients.some((c) => c.id === p.client))
  if (none.length) groups.push(clients.length ? fold('-', none, '<div class="pj-group">No client</div>') : `<div class="pj-group">Threads</div>${none.map(row).join('')}`)
  const closedRows = closed.length ? `<button class="pj-group pj-fold" data-pj="toggle-closed" aria-expanded="${S.pj.showClosed}">${S.pj.showClosed ? '▾' : '▸'} Closed · ${closed.length}</button>${S.pj.showClosed ? closed.map(row).join('') : ''}` : ''
  const err = data.error ? `<p class="error" style="padding:10px">${esc(data.error)}</p>` : ''
  const fc = data.found_count || 0
  const found = `<button class="pj-row pj-found${S.pj.found ? ' on' : ''}${fc ? '' : ' quiet'}" data-pj="found"><span class="pj-spark" aria-hidden="true">✦</span><b class="grow">Found for you</b>${fc ? `<span class="pj-badge">${fc}</span>` : ''}</button>`
  return `<div class="pj-side-head"><h1>Threads</h1><button class="icon-btn" data-pj="new" title="New thread" aria-label="New thread">${ICON('<path d="M12 5v14M5 12h14"/>')}</button></div>${found}
    ${err}<div class="pj-rows">${groups.join('')}${closedRows}</div>`
}

function projectMain(data) {
  if (S.pj.found) return foundView(data)
  if (S.pj.client) return clientView(data)
  if (!S.pj.sel) {
    const n = (data.projects || []).length
    return `<div class="empty"><div>${mark()}<h3>${n ? 'Pick a thread' : 'No threads yet'}</h3><p>A thread gathers the calls of one piece of work — a client engagement, an initiative, hiring — by its meeting series, its people or its chat. Its glossary helps transcription, and the minutes see what was said in its other calls.</p><p>Name one in the box on the left to start.</p></div></div>`
  }
  const p = S.pj.detail
  if (!p) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(8)}</div>`
  if (p.error) return `<div class="empty"><div><h3>This thread can't be opened</h3><p class="error">${esc(p.error)}</p></div></div>`
  const open = workOpenCount()
  const tabs = [['brief', 'Brief'], ['work', `Work${open ? `<span class="n">${open}</span>` : ''}`], ['calls', `Calls<span class="n">${p.call_count}</span>`], ['people', `People${peopleCount() ? `<span class="n">${peopleCount()}</span>` : ''}`], ['profile', 'Profile']]
  const tab = tabs.some(([t]) => t === S.pj.tab) ? S.pj.tab : 'brief'
  const client = p.client_name ? `<span class="tag pj-client-tag">${clientIcon(projClient(p.client), 14)}${esc(p.client_name)}</span>` : ''
  const facts = [`${p.call_count} call${p.call_count === 1 ? '' : 's'}`, p.since ? `since ${esc(p.since)}` : '', p.last_call ? `last call ${esc(dayLabel(new Date(p.last_call)).toLowerCase())}` : ''].filter(Boolean).join(' · ')
  const menu = `<span class="pj-add-wrap pj-more"><button class="icon-btn" data-pj="menu" aria-haspopup="menu" aria-expanded="${S.pj.menu}" aria-label="Thread actions" title="Thread actions">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.pj.menu ? `<div class="menu" role="menu">${p.client ? `<button class="menu-item" role="menuitem" data-pj="client" data-id="${esc(p.client)}">Open ${esc(p.client_name || 'the client')}</button>` : ''}<button class="menu-item" role="menuitem" data-claude-about>Claude about this thread</button><button class="menu-item" role="menuitem" data-pj="sync">Sync now</button><div class="menu-sep" role="separator"></div><button class="menu-item danger" role="menuitem" data-pj="delete-ask">Delete…</button></div>` : ''}</span>`
  const confirmDel = S.pj.confirmDelete ? `<div class="notice"><span class="grow">Delete <b>${esc(p.name)}</b>? Its brief, profile and work go to the trash; its calls stay.</span><button class="btn ghost" data-pj="delete-cancel">Cancel</button><button class="btn danger" data-pj="delete">Move to trash</button></div>` : ''
  const head = `<header class="pj-head"><div class="pj-title">${projIcon(p, 34)}<div style="min-width:0"><div class="pj-name"><h2>${esc(p.name)}</h2>${client}${stageTag(p.stage, true)}</div><div class="facts"><span>${facts}</span></div></div><span class="grow"></span>${menu}</div>${confirmDel}
    <div class="tabs" role="tablist">${tabs.map(([id, label]) => `<button class="tab" role="tab" data-pj="tab" data-tab-id="${id}" aria-selected="${tab === id}">${label}</button>`).join('')}</div>${notices()}</header>`
  const body = tab === 'calls' ? projectCalls(p) : tab === 'profile' ? projectProfile(p, data) : tab === 'work' ? projectWork(p) : tab === 'people' ? peopleView() : projectBrief(p)
  return `${head}<div class="pane pj-pane${tab === 'work' || tab === 'people' ? ' wide' : ''}">${body}</div>`
}

// ── the Work tab: the project's ledger (`ledger.rs`) ────────────────────────────────────────
const CLOSED_STATES = ['done', 'cancelled', 'merged', 'deleted']
const STATE_LABEL = { open: 'to do', in_progress: 'in progress', blocked: 'blocked', done: 'done', cancelled: 'cancelled', merged: 'merged', deleted: 'deleted' }
const KIND_LABEL = { session: 'session', dependency: 'dependency', decision: 'decision', risk: 'risk', question: 'question' }
/** What each kind holds in its Markdown (`description`): the work's description, a question's answer,
 *  the decision itself, what is done about a risk. */
const DESC_PLACEHOLDER = 'What it is, why, what done looks like — Markdown'
const RECORD_TEXT = {
  story: { word: 'description', label: '', empty: 'No description', placeholder: DESC_PLACEHOLDER },
  task: { word: 'description', label: '', empty: 'No description', placeholder: DESC_PLACEHOLDER },
  question: { word: 'answer', label: 'Answer', empty: 'No answer yet', placeholder: 'The answer, and who gave it — Markdown' },
  decision: { word: 'decision', label: 'Decision', empty: 'Not written down', placeholder: 'What was decided, the options weighed, why this one — Markdown' },
  risk: { word: 'response', label: 'Response', empty: 'No response yet', placeholder: 'What we do about it: mitigation, fallback, who watches it — Markdown' },
}
const LEVEL_LABEL = { low: 'low', med: 'medium', high: 'high' }
function workOpenCount() {
  const w = S.pj.work
  if (!w || !w.nodes) return 0
  return Object.values(w.nodes).filter((n) => (n.kind === 'story' || n.kind === 'task') && !CLOSED_STATES.includes(n.state)).length
}
/** What waits for the user: suggested moves and merges, and "done" claims the verifier only half-believed. */
function workToReview(w) {
  const out = []
  for (const n of Object.values(w.nodes || {})) {
    for (const s of n.suggestions || []) if (s.status === 'pending') out.push({ n, s })
    const last = (n.history || []).filter((h) => h.type === 'agent' && h.effect).slice(-1)[0]
    if (last && (last.effect === 'done' || last.effect === 'held') && last.verdict === 'weak' && !CLOSED_STATES.includes(n.state)) out.push({ n, weak: last })
  }
  return out
}
const fmtDay = (d) => d ? new Date(d + 'T12:00').toLocaleDateString([], { weekday: d && (new Date(d + 'T12:00') - Date.now()) / 864e5 < 7 && (new Date(d + 'T12:00') - Date.now()) > -864e5 ? 'short' : undefined, day: 'numeric', month: 'short' }) : ''
const relDay = (d) => { if (!d) return ''; const t = new Date(); const iso = (x) => x.toISOString().slice(0, 10); if (d === iso(t)) return 'today'; t.setDate(t.getDate() - 1); if (d === iso(t)) return 'yesterday'; return fmtDay(d) }
/** A work item's status as the tree and the board show it: its word and its colour. */
function workStatus(n) {
  if (n.kind === 'question') return n.description || n.state === 'done' ? ['answered', 'done'] : CLOSED_STATES.includes(n.state) ? [STATE_LABEL[n.state] || n.state, n.state] : ['open', 'open']
  if (n.kind === 'risk') return CLOSED_STATES.includes(n.state) ? [STATE_LABEL[n.state] || n.state, n.state] : n.description ? ['handled', 'done'] : ['no response', 'waiting']
  if (n.state === 'done' && n.kind === 'task' && n.type === 'session') return ['held', 'done']
  if (n.waiting_for && !CLOSED_STATES.includes(n.state)) return ['waiting', 'waiting']
  return [STATE_LABEL[n.state] || n.state, n.state]
}
/** Where a record came from: its day and the call ("today · weekly 12:05"). */
function workOrigin(w, n) {
  const h = (n.history || []).find((x) => x.op === 'create')
  const item = h && h.evidence && (w.evidence || {})[h.evidence]
  const call = item ? String(item.title || '').split(/[—–-]/)[0].trim().split(/\s+/).slice(0, 2).join(' ').toLowerCase() : ''
  return [n.kind === 'risk' ? `raised ${relDay(n.created)}` : relDay(n.created), call ? `${call}${h.ts ? ' ' + String(h.ts).replace(/^00:/, '').slice(0, 5) : ''}` : ''].filter(Boolean).join(' · ')
}
function workRow(w, n, level) {
  const kind = n.kind === 'task' ? (n.type !== 'action' ? KIND_LABEL[n.type] : '') : KIND_LABEL[n.kind] || ''
  const meta = n.kind === 'story' ? [n.ac.length ? `criteria ${n.ac.filter((a) => a.met).length} / ${n.ac.length}` : '', n.due ? `due ${fmtDay(n.due)}` : ''].filter(Boolean).join(' · ')
    : n.kind === 'task' ? [n.state === 'done' && n.type === 'session' ? `held ${relDay((n.history || []).filter((h) => h.effect === 'held').map((h) => h.date).pop() || n.created)}` : (n.internal_owner || n.client_owner || 'nobody'), n.state !== 'done' && n.due ? fmtDay(n.due) : ''].filter(Boolean).join(' · ')
    : workOrigin(w, n)
  const [word, cls] = workStatus(n)
  const st = n.kind === 'story' || n.kind === 'task' || n.kind === 'question' || n.kind === 'risk' ? `<span class="wk-st ${esc(cls)}">● ${esc(word)}</span>` : ''
  const tags = [n.pinned && n.pinned.length ? `<span class="tag">pinned: ${esc(n.pinned.join(', '))}</span>` : '', n.blocks && n.blocks.length && !CLOSED_STATES.includes(n.state) ? `<span class="tag">holds up ${n.blocks.length}</span>` : ''].join('')
  return `<button class="wk-row${S.pj.node === n.id ? ' on' : ''}${CLOSED_STATES.includes(n.state) ? ' closed' : ''}" style="padding-left:${12 + level * 18}px" data-pj="node" data-node="${esc(n.id)}"><span class="wk-id">${esc(n.id)}</span>${kind ? `<span class="wk-k ${esc(n.kind === 'task' ? n.type : n.kind)}">${esc(kind)}</span>` : ''}<span class="wk-t${n.kind === 'story' ? ' story' : ''}">${esc(n.title)}</span>${tags}<span class="grow"></span><span class="wk-meta">${esc(meta)}</span>${st}</button>`
}
/** Initials for an avatar ("Pat Novak" → "PN"), with a colour of its own. */
function avatar(name, owed) {
  if (!name) return '<span class="bd-av none" title="Nobody named">?</span>'
  const ini = name.split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('')
  const hue = ProjectArt.hash(name) % 360
  return `<span class="bd-av${owed ? ' owed' : ''}" style="--h:${hue}" title="${esc(name)}${owed ? ' (the client owes it)' : ''}">${esc(ini)}</span>`
}
/** The board, as GitHub Projects lays one out: columns of a fixed width (the board scrolls sideways,
 *  cards are never squeezed), each a project's own Status option in a state category; cards with
 *  where they belong on top, the title, then the fields the project shows; optional lanes. */
const DEFAULT_COLUMNS = [
  { id: 'todo', name: 'To do', color: '#8a8a95', state: 'open' },
  { id: 'doing', name: 'In progress', color: '#9b78ff', state: 'in_progress' },
  { id: 'waiting', name: 'Waiting', color: '#f0a24a', state: 'waiting' },
  { id: 'blocked', name: 'Blocked', color: '#e8603c', state: 'blocked' },
  { id: 'done', name: 'Done', color: '#3ccf91', state: 'done' },
]
const CARD_FIELDS = [['story', 'Story'], ['kind', 'Kind'], ['owner', 'Owner'], ['due', 'Due date'], ['waiting', 'Waiting on'], ['holds_up', 'Holds up'], ['source', 'Last call']]
const DEFAULT_FIELDS = ['story', 'kind', 'owner', 'due', 'waiting', 'holds_up']
const STATE_CATS = [['open', 'To do'], ['in_progress', 'In progress'], ['waiting', 'Waiting on someone'], ['blocked', 'Blocked'], ['done', 'Done']]
const COL_COLORS = ['#8a8a95', '#9b78ff', '#5aa9ff', '#4cc3c8', '#3ccf91', '#c9b458', '#f0a24a', '#e8603c', '#d36bd1']
function boardOf() {
  const b = (S.pj.detail && S.pj.detail.board) || {}
  return { columns: b.columns && b.columns.length ? b.columns : DEFAULT_COLUMNS, fields: b.fields && b.fields.length ? b.fields : DEFAULT_FIELDS, group: b.group || 'none', views: b.views && b.views.length ? b.views : DEFAULT_VIEWS }
}
/** Saved views of the Work tab, as GitHub Projects keeps them: each its layout, filters, grouping,
 *  card fields and hidden columns; the columns themselves are the project's. */
const DEFAULT_VIEWS = [{ id: 'board', name: 'Board', layout: 'board' }, { id: 'tree', name: 'Tree', layout: 'tree' }]
const viewKey = () => `echo.pj.view.${S.pj.sel}`
function activeViewId() {
  const views = boardOf().views
  let id = S.pj.viewIds && S.pj.viewIds[S.pj.sel]
  if (!id) { try { id = localStorage.getItem(viewKey()) } catch { /* no storage */ } }
  return views.some((v) => v.id === id) ? id : views[0].id
}
/** The active view, its settings filled in. */
function wv() {
  const b = boardOf()
  const v = b.views.find((x) => x.id === activeViewId()) || b.views[0]
  return { id: v.id, name: v.name || 'View', layout: v.layout === 'tree' ? 'tree' : 'board', kind: v.kind || '', owner: v.owner || '', open: v.open !== false, group: v.group || b.group || 'none', fields: v.fields && v.fields.length ? v.fields : b.fields, hidden: v.hidden || [] }
}
function setView(patch) {
  const b = boardOf()
  const id = activeViewId()
  b.views = b.views.map((v) => v.id === id ? { ...v, ...patch } : v)
  saveBoard(b)
}
function chooseView(id) {
  S.pj.viewIds = { ...(S.pj.viewIds || {}), [S.pj.sel]: id }
  try { localStorage.setItem(viewKey(), id) } catch { /* no storage */ }
  S.pj.viewMenu = false
  render()
}
function viewTabs() {
  const b = boardOf(), cur = activeViewId()
  const icon = (v) => v.layout === 'tree' ? ICON('<path d="M5 5h6M5 12h6M5 19h6M14 5h5M14 12h5M14 19h5"/>') : ICON('<rect x="4" y="5" width="4.5" height="14" rx="1"/><rect x="10" y="5" width="4.5" height="9" rx="1"/><rect x="16" y="5" width="4" height="11" rx="1"/>')
  const tab = (v) => {
    const on = v.id === cur
    const menu = on && S.pj.viewMenu ? `<div class="menu bd-menu wv-menu" role="menu"><div class="bd-menu-row"><input data-view-name value="${esc(v.name || 'View')}" aria-label="View name" autofocus></div><button class="menu-item" data-pj="view-dup">Duplicate view</button>${b.views.length > 1 ? '<button class="menu-item danger" data-pj="view-del">Delete view</button>' : ''}</div>` : ''
    return `<span class="wv-tab${on ? ' on' : ''}"><button class="wv-b" data-pj="${on ? 'view-menu' : 'view-pick'}" data-v="${esc(v.id)}"${on ? ' title="Rename, duplicate or delete this view" aria-haspopup="menu"' : ''}>${icon(v)}${esc(v.name || 'View')}</button>${on ? `<span class="pj-add-wrap"><button class="icon-btn wv-more" data-pj="view-menu" aria-label="View options" aria-haspopup="menu" aria-expanded="${!!S.pj.viewMenu}">${ICON('<path d="M7 10l5 5 5-5"/>')}</button>${menu}</span>` : ''}</span>`
  }
  return `<div class="wv-tabs">${b.views.map(tab).join('')}<button class="wv-add" data-pj="view-new">+ New view</button></div>`
}

function saveBoard(b) {
  if (S.pj.detail) S.pj.detail.board = b
  send({ cmd: 'project', op: 'board', id: S.pj.sel, board: b })
  render()
}
/** A card's category, and the visible column it sits in (the one the user put it in, if it still fits). */
function cardCategory(n) {
  if (n.state === 'done') return 'done'
  if (n.state === 'blocked') return 'blocked'
  if (n.waiting_for) return 'waiting'
  return n.state === 'in_progress' ? 'in_progress' : 'open'
}
function cardColumn(n, cols) {
  const cat = cardCategory(n)
  const own = cols.find((c) => c.id === n.column && c.state === cat)
  return own || cols.find((c) => c.state === cat) || (cat === 'waiting' ? cols.find((c) => c.state === 'open') : null)
}
function workBoard(w, pass) {
  const N = w.nodes || {}
  const b = { ...boardOf(), ...(() => { const v = wv(); return { group: v.group, fields: v.fields, hide: v.hidden } })() }
  const cols = b.columns.filter((c) => !c.hidden && !b.hide.includes(c.id))
  const show = new Set(b.fields)
  const today = new Date().toISOString().slice(0, 10)
  const items = Object.values(N).filter((n) => (n.kind === 'task' || n.kind === 'question') && n.state !== 'merged' && n.state !== 'deleted' && n.state !== 'cancelled' && (pass(n) || n.state === 'done'))
  const placed = items.map((n) => [n, cardColumn(n, cols)]).filter(([, c]) => c)
  const card = (n) => {
    const story = N[n.parent] && N[n.parent].kind === 'story' ? N[n.parent] : null
    const kind = n.kind === 'question' ? 'question' : n.type !== 'action' ? n.type : ''
    const owner = n.internal_owner || n.client_owner
    const late = n.due && n.due < today && n.state !== 'done'
    const last = (n.history || []).filter((h) => h.evidence).pop()
    const item = last && (w.evidence || {})[last.evidence]
    const chips = [
      show.has('kind') && kind ? `<span class="wk-k ${esc(kind)}">${esc(kind)}</span>` : '',
      show.has('due') && n.due ? `<span class="bd-chip${late ? ' late' : ''}" title="${late ? 'Overdue' : 'Due'}">${ICON('<rect x="4" y="5" width="16" height="15" rx="2.5"/><path d="M4 10h16M9 3v4M15 3v4"/>')}${esc(fmtDay(n.due))}</span>` : '',
      show.has('waiting') && n.waiting_for ? `<span class="bd-chip warn" title="${esc(n.waiting_for.what || '')}">⏳ ${esc(n.waiting_for.who || 'someone')}</span>` : '',
      show.has('holds_up') && (n.blocks || []).length && n.state !== 'done' ? `<span class="bd-chip" title="Holds up: ${esc(n.blocks.map((x) => (N[x] || {}).title || x).join('; '))}">holds up ${n.blocks.length}</span>` : '',
      show.has('source') && item ? `<span class="bd-chip" title="${esc(item.title || '')}">▮ ${esc(relDay(item.date))}</span>` : '',
    ].filter(Boolean).join('')
    return `<div class="bd-card${S.pj.node === n.id ? ' on' : ''}${n.state === 'done' ? ' done' : ''}" draggable="true" data-pj="node" data-node="${esc(n.id)}" data-drag="${esc(n.id)}" tabindex="0">
      <div class="bd-top"><span class="bd-ctx"><span class="wk-id">${esc(n.id)}</span>${show.has('story') && story && b.group !== 'story' ? ` · ${esc(story.title)}` : ''}</span>${show.has('owner') && n.kind === 'task' ? avatar(owner, !!n.client_owner && !n.internal_owner) : ''}</div>
      <div class="bd-t">${esc(n.title)}</div>${chips ? `<div class="bd-f">${chips}</div>` : ''}
    </div>`
  }
  // Lanes (Group by): none, each story, or each owner.
  let lanes
  if (b.group === 'story') {
    const keyOf = (n) => (N[n.parent] && N[n.parent].kind === 'story' ? n.parent : '')
    const keys = [...new Set(placed.map(([n]) => keyOf(n)))].sort((x, y) => (x === '') - (y === '') || x.localeCompare(y, undefined, { numeric: true }))
    lanes = keys.map((k) => ({ title: k ? `<button class="bd-lane-t" data-pj="node" data-node="${esc(k)}"><span class="wk-id">${esc(k)}</span><b>${esc(N[k].title)}</b></button>${N[k].ac.length ? `<span class="bd-prog"><span style="width:${Math.round(100 * N[k].ac.filter((a) => a.met).length / N[k].ac.length)}%"></span></span><span class="bd-meta">${N[k].ac.filter((a) => a.met).length}/${N[k].ac.length}</span>` : ''}` : '<b class="bd-lane-loose">Not in a story</b>', rows: placed.filter(([n]) => keyOf(n) === k) }))
  } else if (b.group === 'owner') {
    const keyOf = (n) => n.internal_owner || n.client_owner || ''
    const keys = [...new Set(placed.map(([n]) => keyOf(n)))].sort((x, y) => (x === '') - (y === '') || x.localeCompare(y))
    lanes = keys.map((k) => ({ title: k ? `${avatar(k)}<b>${esc(k)}</b>` : '<b class="bd-lane-loose">Nobody named</b>', rows: placed.filter(([n]) => keyOf(n) === k) }))
  } else lanes = [{ title: '', rows: placed }]
  const count = (c) => placed.filter(([, x]) => x.id === c.id).length
  const head = (c, i) => {
    const n = count(c), over = c.limit && n > c.limit
    const menu = S.pj.colMenu === c.id ? `<div class="menu bd-menu" role="menu">
        <div class="bd-menu-row"><input id="bd-col-name" data-col-name="${esc(c.id)}" value="${esc(c.name)}" aria-label="Column name"></div>
        <div class="bd-menu-row bd-colors">${COL_COLORS.map((x) => `<button class="pj-swatch${c.color === x ? ' on' : ''}" style="background:${x}" data-pj="col-set" data-col="${esc(c.id)}" data-k="color" data-v="${x}" aria-label="Colour ${x}"></button>`).join('')}</div>
        <label class="bd-menu-row">Cards in it are <select data-col-state="${esc(c.id)}">${STATE_CATS.map(([k, l]) => `<option value="${k}"${c.state === k ? ' selected' : ''}>${esc(l.toLowerCase())}</option>`).join('')}</select></label>
        <label class="bd-menu-row">Limit <input type="number" min="0" data-col-limit="${esc(c.id)}" value="${c.limit || ''}" placeholder="none" aria-label="Limit"></label>
        <div class="menu-sep" role="separator"></div>
        ${i > 0 ? `<button class="menu-item" data-pj="col-move" data-col="${esc(c.id)}" data-d="-1">Move left</button>` : ''}${i < cols.length - 1 ? `<button class="menu-item" data-pj="col-move" data-col="${esc(c.id)}" data-d="1">Move right</button>` : ''}
        <button class="menu-item" data-pj="col-set" data-col="${esc(c.id)}" data-k="hidden" data-v="true">Hide</button>
        <button class="menu-item danger" data-pj="col-delete" data-col="${esc(c.id)}">Delete column</button>
      </div>` : ''
    return `<div class="bd-col-h${n ? '' : ' empty'}" title="${esc(c.name)}"><span class="bd-dot" style="background:${esc(c.color)}"></span><b class="bd-hl">${esc(c.name)}</b><span class="bd-n${over ? ' over' : ''}">${n}${c.limit ? ` / ${c.limit}` : ''}</span><span class="grow"></span><span class="pj-add-wrap"><button class="icon-btn bd-more" data-pj="col-menu" data-col="${esc(c.id)}" aria-label="Column options" title="Column options">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${menu}</span></div>`
  }
  const hiddenCols = b.columns.filter((c) => c.hidden || b.hide.includes(c.id))
  // Every column a card's width: the board scrolls sideways when there are many.
  const grid = `grid-template-columns: repeat(${cols.length}, var(--bd-w)) var(--bd-add)`
  const header = `<div class="bd-grid bd-heads" style="${grid}">${cols.map(head).join('')}<div class="bd-col-h add"><button class="tok add" data-pj="col-add">+ Column</button>${hiddenCols.length ? `<button class="link-btn" data-pj="col-unhide">${hiddenCols.length} hidden</button>` : ''}</div></div>`
  const laneHtml = (l) => `${l.title ? `<div class="bd-lane-h">${l.title}<span class="bd-meta">${l.rows.length}</span></div>` : ''}<div class="bd-grid" style="${grid}">${cols.map((c) => `<div class="bd-col" data-drop="${c.state === 'waiting' ? '' : esc(c.id)}">${l.rows.filter(([, x]) => x.id === c.id).map(([n]) => card(n)).join('') || '<div class="bd-empty"></div>'}</div>`).join('')}<div></div></div>`
  if (!placed.length) return `<div class="wk-empty"><b>Nothing here</b><p>No tasks match the filters.</p></div>`
  return `<div class="bd${b.group !== 'none' ? ' grouped' : ''}">${header}${lanes.map(laneHtml).join('')}</div>`
}
// Drag a card to another column: its state, as the user set it (pinned, undoable).
document.addEventListener('dragstart', (e) => {
  const c = e.target.closest && e.target.closest('[data-drag]')
  if (!c) return
  e.dataTransfer.setData('text/x-echo-node', c.dataset.drag)
  e.dataTransfer.effectAllowed = 'move'
  c.classList.add('dragging')
})
document.addEventListener('dragend', (e) => { const c = e.target.closest && e.target.closest('[data-drag]'); if (c) c.classList.remove('dragging'); document.querySelectorAll('.bd-col.over').forEach((x) => x.classList.remove('over')) })
document.addEventListener('dragover', (e) => {
  const cell = e.target.closest && e.target.closest('.bd-col')
  if (!cell || !cell.dataset.drop || !e.dataTransfer.types.includes('text/x-echo-node')) return
  e.preventDefault()
  document.querySelectorAll('.bd-col.over').forEach((x) => x !== cell && x.classList.remove('over'))
  cell.classList.add('over')
})
document.addEventListener('drop', (e) => {
  const cell = e.target.closest && e.target.closest('.bd-col')
  const id = e.dataTransfer && e.dataTransfer.getData('text/x-echo-node')
  if (!cell || !cell.dataset.drop || !id) return
  e.preventDefault()
  cell.classList.remove('over')
  const n = S.pj.work && S.pj.work.nodes[id]
  const col = boardOf().columns.find((c) => c.id === cell.dataset.drop)
  if (!n || !col || n.column === col.id && cardCategory(n) === col.state) return
  const set = { column: col.id }
  if (cardCategory(n) !== col.state) set.state = col.state
  Object.assign(n, set) // shown at once; the ledger answers on the next load
  ledgerEdit({ op: 'edit', node: id, set })
  render()
})

function boardChips() {
  const b = wv()
  const groups = [['none', 'No grouping'], ['story', 'Story'], ['owner', 'Owner']]
  const group = `<label class="fchip${b.group !== 'none' ? ' on' : ''}">${b.group === 'none' ? 'Group' : `Group: ${esc((groups.find(([k]) => k === b.group) || [])[1])}`} ▾<select data-board-group aria-label="Group by">${groups.map(([k, l]) => `<option value="${k}"${b.group === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  const fields = `<span class="pj-add-wrap"><button class="fchip" data-pj="fields-menu">Fields ▾</button>${S.pj.fieldsMenu ? `<div class="menu bd-menu" role="menu">${CARD_FIELDS.map(([k, l]) => `<label class="menu-item bd-check"><input type="checkbox" data-board-field="${k}" ${b.fields.includes(k) ? 'checked' : ''}> ${esc(l)}</label>`).join('')}</div>` : ''}</span>`
  return group + fields
}
const KIND_FILTERS = [['', 'All kinds'], ['story', 'Stories'], ['task', 'Tasks'], ['decision', 'Decisions'], ['risk', 'Risks'], ['question', 'Questions']]
function projectWork(p) {
  const w = S.pj.work
  if (!w) return `<div class="skeleton">${'<i></i>'.repeat(8)}</div>`
  if (w.error) return `<p class="error">${esc(w.error)}</p>`
  const N = w.nodes || {}
  const all = Object.values(N).filter((n) => n.kind !== 'epic' && n.state !== 'merged' && n.state !== 'deleted')
  const sync = w.sync || {}
  const review = workToReview(w)
  const syncText = sync.state === 'running' ? `${esc((sync.message || 'reading calls').replace(/^reading/, 'Reading'))}…`
    : sync.state === 'failed' ? `<span class="error" title="${esc(sync.message || '')}">Sync failed</span>`
    : [sync.at ? `Synced ${esc(hm(new Date(sync.finished_at || sync.at)))}` : 'Not synced yet', w.pending ? `${w.pending} call${w.pending === 1 ? '' : 's'} to read` : '', review.length ? `<span class="wk-warn">${review.length} to review</span>` : ''].filter(Boolean).join(' · ')
  const owners = [...new Set(all.map((n) => n.internal_owner || n.client_owner).filter(Boolean))].sort()
  const counts = Object.fromEntries(KIND_FILTERS.map(([k]) => [k, all.filter((n) => !k || n.kind === k).length]))
  const V = wv()
  const layoutChip = `<label class="fchip">${V.layout === 'tree' ? 'Tree' : 'Board'} ▾<select data-view-layout aria-label="Layout"><option value="board"${V.layout === 'board' ? ' selected' : ''}>Board</option><option value="tree"${V.layout === 'tree' ? ' selected' : ''}>Tree</option></select></label>`
  const kindChip = `<label class="fchip${V.kind ? ' on' : ''}">${esc((KIND_FILTERS.find(([k]) => k === V.kind) || KIND_FILTERS[0])[1])} ▾<select data-pj-filter="kind" aria-label="Kind">${KIND_FILTERS.map(([k, l]) => `<option value="${k}"${V.kind === k ? ' selected' : ''}>${esc(l)} · ${counts[k]}</option>`).join('')}</select></label>`
  const ownerChip = `<label class="fchip${V.owner ? ' on' : ''}">${esc(V.owner === '-' ? 'Nobody named' : V.owner || 'Owner')} ▾<select data-pj-filter="owner" aria-label="Owner"><option value="">Anyone</option><option value="-"${V.owner === '-' ? ' selected' : ''}>Nobody named</option>${owners.map((o) => `<option${V.owner === o ? ' selected' : ''}>${esc(o)}</option>`).join('')}</select></label>`
  const head = `${viewTabs()}<div class="wk-bar">${layoutChip}${kindChip}${ownerChip}<button class="fchip${V.open ? ' on' : ''}" data-pj="open-only">Open only</button>${V.layout === 'board' ? boardChips() : ''}<span class="grow"></span><span class="wk-sync">${syncText}</span><button class="link-btn" data-pj="sync"${sync.state === 'running' ? ' disabled' : ''}>Sync now</button></div>`
  const pass = (n) => (!V.open || !CLOSED_STATES.includes(n.state)) && (!V.kind || n.kind === V.kind)
    && (!V.owner || (V.owner === '-' ? !(n.internal_owner || n.client_owner) && n.kind === 'task' : (n.internal_owner || n.client_owner) === V.owner))
  if (!all.length) {
    return `${head}<div class="wk-empty"><b>No work yet</b><p>${w.pending ? `Sync reads this thread's calls — ${w.pending} so far — into stories, tasks, decisions, risks and questions, each with the words it came from.` : 'Once this thread has calls, Sync reads them into stories, tasks, decisions, risks and questions.'} It uses the thread's AI profile.</p><button class="btn primary" data-pj="sync">Sync now</button></div>`
  }
  const reviewList = review.length ? `<div class="wk-review">${review.map((r) => r.s
    ? `<div class="wk-rv"><span class="wk-warn">To review</span><span class="grow">${esc(r.s.op === 'suggest_merge' ? 'Same work as' : 'Belongs under')} <b>${esc((N[r.s.to] || {}).title || r.s.to)}</b>? <span class="set-sub">${esc(r.n.title)} — “${esc(r.s.quote || '')}”</span></span><button class="btn primary" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'accept', target: r.s.mid }))}'>Yes</button><button class="btn ghost" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'reject', target: r.s.mid }))}'>No</button></div>`
    : `<div class="wk-rv"><span class="wk-warn">To review</span><span class="grow">Is <b>${esc(r.n.title)}</b> done? <span class="set-sub">“${esc(r.weak.quote || '')}”</span></span><button class="btn primary" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'edit', node: r.n.id, set: { state: 'done' } }))}'>Done</button><button class="btn ghost" data-pj="node" data-node="${esc(r.n.id)}">Look</button></div>`).join('')}</div>` : ''
  if (V.layout === 'board') return `${head}${reviewList}${workBoard(w, pass)}`
  const rows = []
  const walk = (id, level) => {
    const n = N[id]
    if (!n || n.state === 'merged' || n.state === 'deleted') return
    const order = ['story', 'task', 'question', 'decision', 'risk']
    const kids = (n.children || []).filter((c) => N[c]).sort((a, b) => order.indexOf(N[a].kind) - order.indexOf(N[b].kind) || a.localeCompare(b))
    if (n.kind === 'epic') {
      const before = rows.length
      rows.push('')
      for (const c of kids) walk(c, 0)
      if (rows.length > before + 1) rows[before] = `<div class="wk-ws">${esc(n.title)}<span> · ${n.plan === 'Off-plan' ? 'the sync found work outside the plan' : 'workstream'}</span></div>`
      return
    }
    const below = rows.length
    // As designed: a story's tasks and questions sit under it; its decisions and risks at its own level.
    for (const c of kids) walk(c, N[c].kind === 'decision' || N[c].kind === 'risk' ? level : level + 1)
    const kidsShown = rows.length > below
    if (pass(n) || kidsShown) rows.splice(below, 0, workRow(w, n, level))
  }
  for (const r of w.roots || []) walk(r, 0)
  return `${head}${reviewList}<div class="wk-tree">${rows.join('') || '<p class="set-sub">Nothing matches.</p>'}</div>`
}

/** The selected work item, in the right panel (`work-item`): as the design's detail — a facts card
 *  read as text (click a value to change it), then its history with the words and the verifier's mark. */
function workPanel() {
  const w = S.pj.work, n = w && w.nodes && S.pj.node ? w.nodes[S.pj.node] : null
  if (!n) return '<div class="wp-empty">Pick a story, task or record on the Work tab.</div>'
  const N = w.nodes, ev = w.evidence || {}
  const kind = n.kind === 'task' ? (n.type === 'action' || !n.type ? 'task' : n.type) : n.kind
  const path = []
  for (let p = n.parent; p && N[p]; p = N[p].parent) path.unshift(N[p].title)
  const edit = S.pj.editField
  const val = (field, text, empty, input) => edit === field ? input
    : `<button class="wp-val${text ? '' : ' empty'}" data-pj="edit-field" data-field="${field}">${text ? esc(text) : `${esc(empty)} · <u>Set</u>`}</button>`
  const titles = (ids) => ids.map((i) => `<button class="wp-link" data-pj="node" data-node="${esc(i)}">${esc(i)} ${esc((N[i] || {}).title || '')}</button>`).join('')
  const trackable = n.kind === 'story' || n.kind === 'task' || n.kind === 'question'
  const [word, cls] = workStatus(n)
  const states = n.kind === 'question' ? [['open', 'open'], ['done', 'answered']] : [['open', 'to do'], ['in_progress', 'in progress'], ['blocked', 'blocked'], ['done', n.type === 'session' ? 'held' : 'done'], ['cancelled', 'cancelled']]
  const actionItems = (n.history || []).filter((h) => h.op === 'create' && h.quote).slice(0, 1)
  const rows = [
    trackable ? ['Status', edit === 'state' ? `<select class="wp-in" data-ledger-set="state" aria-label="Status">${states.map(([v, l]) => `<option value="${v}"${n.state === v ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>` : `<button class="wp-val" data-pj="edit-field" data-field="state"><span class="wk-st ${esc(cls)}">● ${esc(word)}</span></button>${n.derived ? ` <span class="set-sub">${esc(n.derived)}</span>` : ''}`] : null,
    n.kind === 'decision' ? ['Decided by', val('decided_by', n.decided_by, 'not known', `<input class="wp-in" data-ledger-set="decided_by" value="${esc(n.decided_by || '')}" placeholder="Me, or a name" aria-label="Decided by" autofocus>`)] : null,
    ...(n.kind === 'risk' ? [['likelihood', 'Likelihood'], ['impact', 'Impact']].map(([f, l]) => [l, val(f, LEVEL_LABEL[n[f]] || '', 'not rated', `<select class="wp-in" data-ledger-set="${f}" aria-label="${l}" autofocus><option value="">not rated</option>${Object.entries(LEVEL_LABEL).map(([v, t]) => `<option value="${v}"${n[f] === v ? ' selected' : ''}>${t}</option>`).join('')}</select>`)]) : []),
    n.kind === 'task' || n.kind === 'story' || n.kind === 'risk' ? ['Owner', val('internal_owner', n.internal_owner, 'nobody named', `<input class="wp-in" data-ledger-set="internal_owner" value="${esc(n.internal_owner || '')}" aria-label="Owner" autofocus>`)] : null,
    n.kind === 'task' ? ['Handed over by', val('delegated_by', n.delegated_by, n.creator ? `${n.creator} (said it)` : 'not known', `<input class="wp-in" data-ledger-set="delegated_by" value="${esc(n.delegated_by || '')}" placeholder="Me, or a name" aria-label="Handed over by" autofocus>`)] : null,
    n.client_owner ? ['Client owes', esc(n.client_owner)] : null,
    n.kind === 'task' || n.kind === 'story' ? ['Due', val('due', n.due ? new Date(n.due + 'T12:00').toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }) : '', 'no date', `<input class="wp-in" type="date" data-ledger-set="due" value="${esc(n.due || '')}" aria-label="Due" autofocus>`) + ((n.due_history || []).length > 1 ? ` <span class="set-sub">moved ${n.due_history.length - 1}×</span>` : '')] : null,
    n.waiting_for ? ['Waiting on', `${esc(n.waiting_for.who || 'someone')} — ${esc(n.waiting_for.what || '')}`] : null,
    (n.waiting_on || []).length ? ['Waits for', titles(n.waiting_on)] : null,
    (n.blocks || []).length ? ['Holds up', titles(n.blocks)] : null,
    // Work in other threads (#20): the same work, or what it waits on / holds up there.
    n.mirror_of ? ['Same as', `${xItem(n.mirror_of.thread, n.mirror_of.node, n.mirror_of.title)} <span class="set-sub">— done there: its status, owner and due show here</span> ${xUnlink(n, n.mirror_of.thread, n.mirror_of.node)}`] : null,
    (n.also_in || []).length ? ['Also in', n.also_in.map((a) => `${xItem(a.thread, a.node, a.title)} ${xUnlink(n, a.thread, a.node)}`).join('<br>')] : null,
    (n.xwaits || []).length ? ['Waits for', n.xwaits.map((a) => `${xItem(a.thread, a.node, a.title)}${a.owner ? ` <span class="set-sub">· ${esc(a.owner)}</span>` : ''} ${xUnlink(n, a.thread, a.node)}`).join('<br>')] : null,
    ...(n.xrefs || []).filter((r) => r.kind === 'blocks').map((r) => ['Holds up', `${xItem(r.thread, r.node, '')} <span class="set-sub">in another thread</span> ${xUnlink(n, r.thread, r.node)}`]),
    (n.links || []).length ? ['From', n.links.filter((l) => /^https:\/\//i.test(l.url || '')).map((l) => `<a class="wp-link" href="${esc(l.url)}" data-ext title="${esc(l.url)}">↗ ${esc(l.title)}</a>`).join('')] : null,
    n.kind !== 'story' && actionItems.length ? ['Said', `“${esc(actionItems[0].quote.length > 90 ? actionItems[0].quote.slice(0, 88) + '…' : actionItems[0].quote)}”`] : null,
  ].filter(Boolean)
  // What the work is: an agent's from the words that created it, or yours (then it stays yours).
  // A record's: its answer, the decision, its response — under the title, edited the same way.
  const rt = RECORD_TEXT[n.kind]
  const de = S.pj.descEdit && S.pj.descEdit.node === n.id ? S.pj.descEdit : null
  const head = rt && rt.label ? `<div class="wp-h wp-rec-h">${esc(rt.label)}</div>` : ''
  const description = !rt ? ''
    : de ? `${head}<div class="wp-desc-edit"><textarea id="wp-desc" rows="6" spellcheck="true" aria-label="${esc(rt.label || 'Description')} (Markdown)" placeholder="${esc(rt.placeholder)}">${esc(de.text)}</textarea><div class="wp-desc-bar"><span class="set-sub">${MAC ? '⌘S' : 'Ctrl+S'} saves, Esc cancels</span><span class="grow"></span><button class="btn ghost" data-pj="desc-cancel">Cancel</button><button class="btn primary" data-pj="desc-save">Save</button></div></div>`
    : n.description ? `${head}<div class="wp-desc doc" data-pj="desc-edit" title="Click to change">${n.description_html || esc(n.description)}</div>`
    : `${head}<button class="wp-val empty wp-desc-add${n.kind === 'risk' && !CLOSED_STATES.includes(n.state) ? ' warn' : ''}" data-pj="desc-edit">${esc(rt.empty)} · <u>Add</u></button>`
  const met = n.ac.filter((a) => a.met).length
  const criteria = n.kind === 'story' ? `<div class="wp-h">Done when · ${met} of ${n.ac.length}</div><div class="wp-ac-list">${n.ac.map((a, i) => `<label class="wp-ac${a.met ? ' met' : ''}"><input type="checkbox" data-ledger-ac="${i + 1}" ${a.met ? 'checked' : ''}><span>${esc(a.text)}</span></label>`).join('')}</div>` : ''
  const hist = (n.history || []).slice().reverse().map((h) => {
    const what = h.by === 'human' && (h.op === 'xref' || h.op === 'unxref') ? `${h.op === 'xref' ? 'you linked it to' : 'you unlinked'} ${(projById(h.thread) || {}).name || h.thread} · ${h.target}${h.op === 'xref' ? (h.kind === 'same' ? (h.master === 'here' ? ' (the same work, done here)' : ' (the same work, done there)') : h.kind === 'depends_on' ? ' (waits on it)' : ' (it waits on this)') : ''}`
      : h.from ? `${h.effect || h.op} · heard in ${(projById(h.from.thread) || {}).name || h.from.thread}`
      : h.by === 'human' ? (h.op === 'edit' ? `you set ${Object.keys(h.set || {}).map((k) => k === 'description' && rt ? rt.word : k.replace('_', ' ')).join(', ')}` : h.op === 'create' ? 'you created it' : `you: ${h.op}`)
      : h.op === 'answer' ? `${n.kind === 'risk' ? 'response' : 'answered'}: ${h.text || ''}`
      : h.op === 'create' ? `created${h.due_text ? ' · due ' + h.due_text : ''}` : h.effect ? `${h.effect === 'due' ? 'due ' + (h.due_text || '') : h.effect}${h.effect === 'done' || h.effect === 'held' ? '?' : ''}`
      : h.op === 'waiting' ? `waiting · ${h.who || '?'}` : h.op === 'link' ? `waits for ${h.depends_on}` : h.op === 'ac_met' ? `criterion ${h.ac} met` : h.op
    const mark = h.by === 'human' ? (h.mid && h.op !== 'create' ? `<button class="link-btn" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'undo', target: h.mid }))}'>undo</button>` : '')
      : h.applied === false && h.verdict === 'supports' ? '<span class="wk-v weak">kept yours</span>'
      : h.verdict === 'supports' || (h.op === 'create') ? '<span class="wk-v supports">verified</span>'
      : h.verdict === 'weak' ? '<span class="wk-v weak">weak · to review</span>' : h.verdict === 'wrong' ? '<span class="wk-v wrong">wrong</span>'
      : (h.effect || ['ac_met', 'link', 'waiting', 'answer'].includes(h.op)) && h.effect !== 'mention' ? '<span class="wk-v">to check</span>' : ''
    const item = h.evidence && ev[h.evidence]
    // A call's words jump to the call; a mail's open it in Outlook.
    const srcs = !item ? '' : item.kind === 'mail'
      ? (/^https:\/\//i.test(item.url || '') ? `<a class="wp-src" href="${esc(item.url)}" data-ext title="${esc(item.title || 'The mail')} — open in Outlook">✉ ${esc(relDay(item.date))}</a>` : `<span class="wp-src" title="${esc(item.title || '')}">✉ ${esc(relDay(item.date))}</span>`)
      : `<button class="wp-src" data-call="${esc(item.call || '')}" title="${esc(item.title || '')}">▮ ${esc(h.ts ? String(h.ts).replace(/^00:/, '').slice(0, 5) : relDay(item.date))}</button>`
    return `<div class="wp-ev"><div class="wp-ev-top"><span class="set-sub">${esc(relDay(h.date))}</span><b>${esc(what)}</b>${h.speaker ? `<span class="set-sub">${esc(h.speaker)}</span>` : ''}<span class="grow"></span>${mark}</div>${h.quote ? `<div class="wp-q">“${esc(h.quote)}” ${srcs}</div>` : ''}</div>`
  }).join('')
  return `<div class="wp">
    <div class="wp-top"><span class="wk-id">${esc(n.id)}</span><span class="wk-k ${esc(n.kind === 'task' ? n.type : n.kind)}">${esc(kind)}</span><span class="grow"></span><button class="btn" data-pj="edit-title">Edit</button></div>
    ${edit === 'title' ? `<textarea class="wp-title" data-ledger-set="title" rows="1" aria-label="Title" autofocus>${esc(n.title)}</textarea>` : `<h3 class="wp-name">${esc(n.title)}</h3>`}
    ${path.length ? `<div class="set-sub">${esc(path.join(' › '))}</div>` : ''}
    ${description}
    ${rows.length ? `<div class="wp-facts">${rows.map(([k, v]) => `<span>${esc(k)}</span><div>${v}</div>`).join('')}</div>` : ''}
    ${criteria}
    ${['task', 'story', 'question'].includes(n.kind) ? xLinkBlock(n) : ''}
    ${(() => { const b = slotItems('task.actions', { kind: n.kind, node: n, project: S.pj.sel }); return b ? `<div class="acts plug-acts">${b}</div>` : '' })()}
    ${mentionsBlock(n)}
    <div class="wp-h">History</div>${hist}
  </div>`
}
/** An item of another thread, as a link that opens it there (#20). */
function xItem(thread, node, title) {
  const p = projById(thread)
  return `<button class="wp-link" data-pj="goto-node" data-id="${esc(thread)}" data-node="${esc(node)}" title="Open it in ${esc(p ? p.name : thread)}">${esc(p ? p.name : thread)} · ${esc(node)}${title ? ' ' + esc(title) : ''}</button>`
}
const xUnlink = (n, thread, node) => `<button class="link-btn" data-pj="xunlink" data-node="${esc(n.id)}" data-thread="${esc(thread)}" data-target="${esc(node)}" title="Each is on its own again, with its own last values">Unlink</button>`
/** Link to an item in another thread (#20): a search over open work by its title (no model), then how they relate. */
function xLinkBlock(n) {
  const x = S.pj.xlink
  if (!x || x.node !== n.id) return `<button class="link-btn wp-xopen" data-pj="xlink-open" data-node="${esc(n.id)}">Link to an item in another thread…</button>`
  const rows = (x.results || []).map((r) => `<div class="wp-xr"><div class="wp-xt"><b>${esc(r.thread_name)} · ${esc(r.id)}</b> ${esc(r.title)}${r.owner ? ` <span class="set-sub">· ${esc(r.owner)}</span>` : ''}</div>
      <div class="wp-xa"><button class="btn" data-pj="xlink" data-thread="${esc(r.thread)}" data-target="${esc(r.id)}" data-kind="same" data-master="there" title="One piece of work; ${esc(r.thread_name)} does it — this one shows its status">Same work · done there</button><button class="btn" data-pj="xlink" data-thread="${esc(r.thread)}" data-target="${esc(r.id)}" data-kind="same" data-master="here" title="One piece of work; this thread does it">Done here</button><button class="btn" data-pj="xlink" data-thread="${esc(r.thread)}" data-target="${esc(r.id)}" data-kind="depends_on" title="This one waits until that one is done">Waits on it</button></div></div>`).join('')
  return `<div class="wp-xlink"><div class="wp-h">Link to an item in another thread</div>
    <input class="wp-in" id="wp-xq" value="${esc(x.q || '')}" placeholder="Words from its title, or its id" aria-label="Find an item in another thread" autofocus>
    ${x.q && !(x.results || []).length ? '<p class="set-sub">No open item matches in your other threads.</p>' : rows}
    <button class="link-btn" data-pj="xlink-close">Cancel</button></div>`
}
let xlinkTimer = null
document.addEventListener('input', (e) => {
  if (e.target.id !== 'wp-xq' || !S.pj.xlink) return
  S.pj.xlink.q = e.target.value
  clearTimeout(xlinkTimer)
  xlinkTimer = setTimeout(async () => {
    const x = S.pj.xlink
    if (!x) return
    try { x.results = await get(`api/work/search?q=${encodeURIComponent(x.q)}&not=${encodeURIComponent(S.pj.sel || '')}`) } catch { x.results = [] }
    render()
    const i = $('#wp-xq'); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length) }
  }, 250)
})
PANEL_VIEWS.push({ id: 'work-item', label: 'Details', icon: ICON('<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8.5 12.5l2.5 2.5 4.5-5"/>'), area: 'right', when: () => ((S.view === 'projects' && S.pj.tab === 'work') || S.pj.inline === S.view) && !!S.pj.node, render: workPanel })
function ledgerEdit(body) {
  send({ cmd: 'project', op: 'ledger', id: S.pj.sel, body })
  setTimeout(async () => {
    if (S.view === 'projects') await loadProject()
    else { await loadWork(); if (S.view === 'tasks') await loadTasks(); else if (S.selected) await loadCall(S.selected) }
    render()
  }, 300)
}

// ── Tasks: every open project's open tasks by who has to move, and what to ask whom ────────
// Computed by the app (`tasks.rs`) from the ledgers; a card opens the task's details on the right.
const TASK_PILES = [['mine', 'I owe'], ['owed', 'Owed to me'], ['waiting', 'Waiting for others'], ['nobody', 'Nobody named']]
const TASK_SORTS = [['due', 'Due'], ['quiet', 'Quiet longest'], ['project', 'Thread']]
const TASK_SHOWN = 6
const projById = (id) => ((S.pj.data && S.pj.data.projects) || []).find((p) => p.id === id)
const dueText = (c) => c.due ? `<span class="tk-due${c.late ? ' late' : ''}" title="${c.late ? 'Overdue' : 'Due'}">${esc(fmtDay(c.due))}</span>` : '<span class="tk-due none">no date</span>'
/** Who a card is about (the user's rules, 2026-11-02): what you owe — its story, and what blocks
 *  it; owed — who does it (· client); waiting — who, and which of yours it stops; others — who. */
function taskWho(c, pile) {
  const stops = (c.blocks || []).length ? ` · stops ${c.blocks.map((b) => `<span title="${esc(b.title)}">${esc(b.id)}</span>`).join(', ')}` : ''
  if (pile === 'waiting') return c.wait ? `${esc(c.who || 'someone')}${stops}` : `${esc(c.owner || 'nobody named')}${c.client ? ' · client' : ''}${stops}`
  if (pile === 'owed' || pile === 'others') return `${esc(c.owner)}${c.client ? ' · client' : ''}${c.blocked ? ' · <span class="late">blocked</span>' : ''}`
  if (pile === 'mine' && c.blocked) return `${c.story ? esc(c.story.title) + ' · ' : ''}<span class="late">blocked</span>${c.waiting_for && c.waiting_for.who ? ` by ${esc(c.waiting_for.who)}` : c.after.length ? ` by ${c.after.map((a) => `<span title="${esc(a.title)}">${esc(a.id)}</span>`).join(', ')}` : ''}`
  return c.story ? esc(c.story.title) : ''
}
/** Others: other people's work in your threads that you neither handed over nor wait on — folded. */
function othersPile(all, key) {
  if (!all.length) return ''
  const open = S.tk.more[key]
  return `<section class="tk-others"><button class="tk-more" data-tk="more" data-pile="${key}">${open ? '▾' : '▸'} Others · ${all.length} <span class="set-sub">— other people's work in your threads: not handed over by you, not stopping you</span></button>${open ? `<div class="tk-others-cards">${all.map((c) => taskCard(c, 'others')).join('')}</div>` : ''}</section>`
}
function taskCard(c, pile) {
  const p = projById(c.project) || { id: c.project, name: c.project }
  const quiet = c.quiet >= 5 && !c.due ? `<span class="tk-quiet" title="No word about it in ${c.quiet} days">quiet ${c.quiet} d</span>` : ''
  return `<button class="tk-card${c.wait ? ' tk-wait' : ''}${S.pj.node === c.id && S.pj.sel === c.project ? ' on' : ''}" data-tk="open" data-project="${esc(c.project)}" data-node="${esc(c.id)}">
    <span class="tk-t">${esc(c.title)}</span>${(() => { const w = taskWho(c, pile); return w ? `<span class="tk-who">${w}</span>` : '' })()}
    <span class="tk-m"><span class="tk-pj">${projIcon(p, 12)}${esc(p.name)}</span>${(c.also_in || []).map((a) => { const q = projById(a.thread) || { id: a.thread, name: a.thread }; return `<span class="tk-pj" title="Also raised in ${esc(q.name)} (${esc(a.node)})">${projIcon(q, 12)}${esc(q.name)}</span>` }).join('')}<span class="grow"></span>${quiet}${dueText(c)}</span>
  </button>`
}
function tasksView() {
  const d = S.tk.data
  if (!d) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<div class="tk"><p class="error">${esc(d.error)}</p></div>`
  const projects = d.projects || []
  const pick = S.tk.project && projects.some((p) => p.id === S.tk.project) ? S.tk.project : ''
  const name = (id) => (projById(id) || { name: id }).name
  const sorts = {
    due: () => 0,
    quiet: (a, b) => (b.quiet || 0) - (a.quiet || 0),
    project: (a, b) => name(a.project).localeCompare(name(b.project)),
  }
  const projChip = `<label class="fchip${pick ? ' on' : ''}">${esc(pick ? name(pick) : 'All threads')} ▾<select data-tk-project aria-label="Thread"><option value="">All threads</option>${projects.map((p) => `<option value="${esc(p.id)}"${pick === p.id ? ' selected' : ''}>${esc(p.name)} · ${p.tasks}</option>`).join('')}</select></label>`
  const sortChip = `<label class="fchip">${esc((TASK_SORTS.find(([k]) => k === S.tk.sort) || TASK_SORTS[0])[1])} ▾<select data-tk-sort aria-label="Order">${TASK_SORTS.map(([k, l]) => `<option value="${k}"${S.tk.sort === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  const head = `<header class="tk-head"><h2>Tasks</h2><span class="tk-sub">across ${projects.length} active project${projects.length === 1 ? '' : 's'}</span><span class="grow"></span>${projChip}${sortChip}</header>`
  if (!projects.length) return `<div class="tk">${head}<div class="wk-empty"><b>No active threads</b><p>Tasks come from your threads' calls: each thread's Work tab reads them into stories and tasks.</p><button class="btn primary" data-go="projects">Open Threads</button></div></div>`
  const total = TASK_PILES.reduce((n, [k]) => n + ((d.piles || {})[k] || []).length, 0) + ((d.piles || {}).others || []).length
  if (!total) return `<div class="tk">${head}<div class="wk-empty"><b>No open tasks</b><p>Once a thread's calls are synced, what was promised, owed and waited on shows here — from every thread at once.</p></div></div>`
  const cols = TASK_PILES.map(([k, label]) => {
    const all = ((d.piles || {})[k] || []).filter((c) => !pick || c.project === pick).slice().sort(sorts[S.tk.sort] || sorts.due)
    const open = S.tk.more[k], shown = open ? all : all.slice(0, TASK_SHOWN)
    const more = all.length > TASK_SHOWN ? `<button class="tk-more" data-tk="more" data-pile="${k}">${open ? 'Show fewer' : `+ ${all.length - TASK_SHOWN}`}</button>` : ''
    return `<section class="tk-col"><h3>${esc(label)} <span>${all.length}</span></h3>${shown.map((c) => taskCard(c, k)).join('') || '<p class="tk-none">Nothing here</p>'}${more}</section>`
  }).join('')
  const errs = (d.errors || []).length ? `<p class="set-sub tk-err">${d.errors.map((e) => `${esc(name(e.project))}: ${esc(e.error)}`).join(' · ')}</p>` : ''
  const others = ((d.piles || {}).others || []).filter((c) => !pick || c.project === pick).slice().sort(sorts[S.tk.sort] || sorts.due)
  return `<div class="tk">${head}${errs}<div class="tk-cols">${cols}</div>${othersPile(others, 'others')}</div>`
}
/** What to ask whom, next time you meet (right panel): late, due-soon and quiet things they owe,
 *  and what we wait on them for. */
function asksPanel() {
  const d = S.tk.data
  const asks = ((d && d.asks) || []).map((a) => ({ ...a, items: a.items.filter((c) => !S.tk.project || c.project === S.tk.project) })).filter((a) => a.items.length)
  const when = (n) => {
    if (!n || !n.start) return 'no meeting this week'
    const s = new Date(n.start), t = new Date()
    const day = s.toDateString() === t.toDateString() ? 'today' : s.toLocaleDateString([], { weekday: 'short' })
    return `${day} · ${n.people <= 1 ? '1:1' : esc(n.subject || 'a meeting')}`
  }
  const why = (c) => c.why === 'late' ? `was due ${esc(fmtDay(c.due))}` : c.why === 'due' ? `due ${esc(fmtDay(c.due))}` : c.why === 'quiet' ? `quiet ${c.quiet} days` : `waiting${c.waiting_for && c.waiting_for.what ? ` · ${esc(c.waiting_for.what)}` : ''}`
  const rows = asks.map((a) => `<div class="ask"><div class="ask-h"><b>${esc(a.who)}</b><span>${when(a.next)}</span></div>${a.items.map((c) => `<button class="ask-i" data-tk="open" data-project="${esc(c.project)}" data-node="${esc(c.id)}">${esc(c.title)} <span class="${c.why === 'late' ? 'late' : ''}">— ${why(c)}</span></button>`).join('')}</div>`).join('')
  return `<div class="asks"><div class="asks-h"><b>Asks · by person</b><span>next time you meet</span></div>${rows || '<p class="set-sub">Nothing to chase: nobody owes you anything late, due soon or gone quiet.</p>'}<p class="asks-foot">Computed from what went quiet, what's due and what's owed — never a model's guess.</p></div>`
}
PANEL_VIEWS.push({ id: 'asks', label: 'Asks', icon: ICON('<path d="M5 18.5V7a2.5 2.5 0 0 1 2.5-2.5h9A2.5 2.5 0 0 1 19 7v6a2.5 2.5 0 0 1-2.5 2.5H9z"/><path d="M10 9.2a2 2 0 1 1 2.6 1.9c-.4.2-.6.5-.6.9v.3M12 14v.01"/>'), area: 'right', when: () => S.view === 'tasks', render: asksPanel })
document.addEventListener('click', async (e) => {
  const b = e.target.closest && e.target.closest('[data-tk]')
  if (!b) return
  e.preventDefault()
  const d = b.dataset
  if (d.tk === 'more') { S.tk.more[d.pile] = !S.tk.more[d.pile]; return render() }
  if (d.tk === 'open') {
    if (S.pj.sel !== d.project) { S.pj.sel = d.project; S.pj.detail = null; S.pj.work = null; S.pj.tab = 'work' }
    S.pj.node = d.node
    S.pj.inline = S.view
    S.pj.editField = null
    const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
    if (eff.narrow) S.overlay = true
    if (!S.pj.work) await loadWork()
    Layout.show('work-item', known)
    layoutChanged()
  }
})
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-cc-sync]')
  if (!b) return
  send({ cmd: 'project', op: 'sync', id: b.dataset.ccSync })
  b.disabled = true
  b.textContent = 'Reading…'
})
document.addEventListener('change', (e) => {
  const t = e.target
  if (t.matches && t.matches('[data-tk-project]')) { S.tk.project = t.value; render() }
  if (t.matches && t.matches('[data-tk-sort]')) { S.tk.sort = t.value; render() }
})

/** Projects Echo would make from calls in none (`projects::found`): accept, merge, reject. */
function foundView(data) {
  const fd = S.pj.foundData
  const head = `<header class="pj-head"><div class="pj-title"><div style="min-width:0"><h2>Found for you</h2><div class="facts"><span>From your calls of the last 90 days that are in no thread — a recurring meeting, one organisation's calls, one Teams chat. Mail joins later.</span></div></div></div>${notices()}</header>`
  if (!fd) return `${head}<div class="pane pj-pane"><div class="skeleton">${'<i></i>'.repeat(6)}</div></div>`
  const clients = data.clients || [], projects = (data.projects || []).filter((p) => p.group !== 'closed')
  const cards = (fd.found || []).map((c) => {
    const calls = c.calls || []
    const titles = calls.slice(0, 4).map((x) => `<button class="pj-call" data-call="${esc(x.name)}"><span class="pj-call-d">${x.started_at ? esc(dayLabel(new Date(x.started_at))) : ''}</span><span class="t">${esc(x.title || x.name)}</span></button>`).join('')
    const more = calls.length > 4 ? `<div class="set-sub" style="padding:6px 14px">+ ${calls.length - 4} more</div>` : ''
    const nc = c.new_client
    const def = c.client || ''
    // The organisation it is with joins it as a counterpart (made from its domain when new) —
    // unless you make it the thread's client.
    const org = c.org ? `<div class="set-sub">With ${esc(c.org.name)}${c.org.new ? ` (${esc(c.org.domain)}) — a new organisation, not a client` : ''}: its counterpart</div>` : ''
    const clientSel = `<select class="found-client" aria-label="Client"><option value=""${def === '' ? ' selected' : ''}>No client</option>${nc ? `<option value="new"${def === 'new' ? ' selected' : ''}>New client: ${esc(nc.name)} (${esc(nc.domain)})</option>` : ''}${clients.map((x) => `<option value="${esc(x.id)}"${def === x.id ? ' selected' : ''}>${esc(x.name)}</option>`).join('')}</select>`
    const merge = projects.length ? `<select class="found-merge" aria-label="Add to a thread"><option value="">Add to a thread…</option>${projects.map((p) => `<option value="${esc(p.id)}">${esc(p.name)}</option>`).join('')}</select>` : ''
    return `<section class="set-card found-card" data-key="${esc(c.key)}">
      <div class="set-row"><div class="found-top">${projIcon({ id: c.key, name: c.name, color: PJ_COLORS[ProjectArt.hash(c.key) % PJ_COLORS.length] }, 28)}<input class="found-name" value="${esc(c.name)}" aria-label="Thread name"></div>
        <div class="set-ctl"><button class="btn primary" data-pj="found-accept">Accept</button><button class="btn ghost" data-pj="found-reject" title="Not a thread — never suggested again">Reject</button></div></div>
      <div class="set-row"><div><div class="set-sub">${esc(c.why)}</div>${org}</div><div class="set-ctl">${clientSel}${merge}</div></div>
      <div class="pj-calls">${titles}${more}</div></section>`
  }).join('')
  const empty = `<p class="set-sub" style="margin:4px 2px">Nothing found right now. Echo suggests a thread when two or more calls that are in none share a meeting series, one outside organisation, or a Teams chat.</p>`
  const newRow = `<div class="pj-new"><span aria-hidden="true">+</span><input id="pj-new" placeholder="New thread — name it, Echo finds its calls" aria-label="New thread name" autocomplete="off"><span class="set-sub">Client</span><select id="pj-new-client" aria-label="Client"><option value="">None</option>${clients.map((x) => `<option value="${esc(x.id)}">${esc(x.name)}</option>`).join('')}</select><button class="btn" data-pj="create">Create</button></div>`
  return `${head}<div class="pane pj-pane">${newRow}${fd.error ? `<p class="error">${esc(fd.error)}</p>` : ''}${cards || empty}</div>`
}

function projectBrief(p) {
  if (S.pj.briefEdit != null) {
    return `<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>What it is</b><div class="set-sub">Yours: goals, scope, who matters. Markdown. The minutes and Claude read it.</div></div><div class="set-ctl"><button class="btn ghost" data-pj="brief-cancel">Cancel</button><button class="btn primary" data-pj="brief-save">Save</button></div></div>
      <div class="me-edit"><textarea id="pj-brief" spellcheck="true" aria-label="The thread's brief (Markdown)">${esc(S.pj.briefEdit)}</textarea><div class="set-sub">⌘S / Ctrl+S saves, Esc cancels.</div></div></div></section>`
  }
  const body = p.brief && p.brief.trim() ? `<div class="doc me-doc">${p.brief_html}</div>` : '<div class="me-doc set-sub">Nothing yet — say what this thread is for, so the minutes and Claude know.</div>'
  const recent = (p.calls || []).slice(0, 5).map((c) => `<button class="pj-call" data-call="${esc(c.name)}"><span class="pj-call-d">${c.started_at ? esc(dayLabel(new Date(c.started_at))) : ''}</span><span class="t">${esc(c.title || c.name)}</span>${c.minutes ? '' : '<span class="set-sub">no minutes yet</span>'}</button>`).join('')
  return `${insightsBlock('thread', p.id, p.name)}<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>What it is</b></div><div class="set-ctl"><button class="btn" data-pj="brief-edit">Edit</button></div></div>${body}</div></section>
    <section class="set-sec"><h3>Latest calls</h3><div class="set-card pj-calls">${recent || '<div class="set-row"><div class="set-sub">No calls yet: add a rule on Profile, or put a call in from its header.</div></div>'}</div></section>`
}

const WHY = { series: 'its meeting series', people: "the client's people", chat: 'its Teams chat', subject: 'its subject', pinned: 'added by hand' }
function projectCalls(p) {
  const rows = (p.calls || []).map((c) => {
    const d = c.started_at ? new Date(c.started_at) : null
    const ctl = c.why === 'pinned'
      ? `<button class="btn ghost" data-pj="member" data-call-name="${esc(c.name)}" data-how="rules" title="Leave it to the rules">Unpin</button>`
      : `<button class="btn ghost" data-pj="member" data-call-name="${esc(c.name)}" data-how="exclude" title="Take it out (the rules won't put it back)">Take out</button>`
    return `<div class="set-row"><div><button class="link pj-call-t" data-call="${esc(c.name)}">${esc(c.title || c.name)}</button><div class="set-sub">${d ? `${esc(dayLabel(d))}, ${esc(hm(d))} · ` : ''}${esc(WHY[c.why] || c.why)}${c.minutes ? '' : ' · no minutes yet'}</div></div><div class="set-ctl">${ctl}</div></div>`
  }).join('')
  const out = p.exclusions && p.exclusions.length ? `<p class="set-foot">${p.exclusions.length} call${p.exclusions.length === 1 ? '' : 's'} taken out by hand.</p>` : ''
  return `<section class="set-sec"><div class="set-card">${rows || '<div class="set-row"><div class="set-sub">No calls match yet. Add a rule on Profile — a meeting series, the client\'s people, a Teams chat — or open a call and add it from its header.</div></div>'}</div>${out}</section>`
}

const RULE_TEXT = (r, clientName) => r.kind === 'series' ? `Meeting series “${esc(r.subject)}”`
  : r.kind === 'people' ? (r.domains && r.domains.length ? `People from ${esc(r.domains.join(', '))}` : `${esc(clientName || "The client")}'s people`)
  : r.kind === 'chat' ? `Calls from the Teams chat ${esc(r.name || r.id)}` : `Subject contains “${esc(r.contains)}”`
const RULE_SUB = { series: 'Every occurrence, by its calendar series or its subject.', people: "At least half of a call's outside people — your colleagues don't count.", chat: "A meeting's chat, or the chat a call was started from.", subject: 'Whole words, any case.' }

/** A token (× removes it) and the dashed "+ Add" that opens an inline box in its place. */
const tok = (html, remove, title = '') => `<span class="tok"${title ? ` title="${esc(title)}"` : ''}>${html}${remove ? `<button data-pj="${remove.op}" ${Object.entries(remove).filter(([k]) => k !== 'op').map(([k, v]) => `data-${k}="${esc(v)}"`).join(' ')} aria-label="Remove">×</button>` : ''}</span>`
function addTok(what, placeholder, extra = '') {
  if (S.pj.adding !== what) return `<button class="tok add" data-pj="adding" data-what="${what}">+ Add</button>`
  return `<span class="tok-in">${extra}<input id="pj-add-${what}" data-add="${what}" placeholder="${esc(placeholder)}" aria-label="${esc(placeholder)}" autofocus></span>`
}
const RULE_TOKEN = (r, clientName) => r.kind === 'series' ? `series “${esc(r.subject)}”` : r.kind === 'people' ? (r.domains && r.domains.length ? `people from ${esc(r.domains.join(', '))} ≥ half` : `${esc(clientName || 'client')} people ≥ half`) : r.kind === 'chat' ? `from ${esc(r.name || 'a Teams chat')}` : `subject “${esc(r.contains)}”`
const shortDay = (iso) => new Date(iso).toLocaleDateString([], { day: 'numeric', month: 'short' })

/** A thread's sources for the Brain: Teams channels and mail (read, never copied: each section links
 *  back to the message), how each was last read, and the pickers. Folders come next. */
const m365 = { teams: null, channels: {}, state: null, stateAt: 0 }
function loadSourceState(force) {
  if (!force && Date.now() - m365.stateAt < 15000) return
  m365.stateAt = Date.now()
  get('api/brain/sources').then((d) => { m365.state = d; render() }).catch(() => {})
  get('api/brain/files').then((d) => { m365.files = d && !d.error ? d : {}; render() }).catch(() => {})
}
/** A folder's files in a few words: read, waiting for the page reader, not read. */
function folderSummary(root) {
  const fs = ((m365.files || {})[root]) || []
  if (!fs.length) return { line: 'Not read yet', n: 0 }
  const by = (st) => fs.filter((f) => f.status === st).length
  const read = by('indexed') + by('pending'), pending = fs.filter((f) => f.status === 'pending').reduce((n, f) => n + ((f.flagged || 0) - (f.read || 0)), 0)
  const not = fs.length - read, err = by('error')
  return { line: [`${read} file${read === 1 ? '' : 's'} read`, pending ? `${pending} page${pending === 1 ? '' : 's'} to read from their images` : '', not ? `${not} not read (${[by('twin') ? `${by('twin')} twin${by('twin') === 1 ? '' : 's'}` : '', by('skipped') ? `${by('skipped')} skipped` : '', by('unsupported') ? `${by('unsupported')} other formats` : '', err ? `${err} couldn't be read` : ''].filter(Boolean).join(', ')})` : ''].filter(Boolean).join(' · '), n: fs.length, err }
}
function sourceLabel(x) {
  if (x.kind === 'mail') return x.id.startsWith('person:') ? `mail with ${x.id.slice(7)}` : `mail with ${x.id.replace(/^domain:/, '')}`
  return x.name || x.id
}
/** A thread's meeting chats, read into its work between calls (`ledger::chat_sync`), with the chats
 *  of its calls it doesn't read yet offered — whether or not the Brain is on. */
function chatSources(p) {
  const cs = p.chat_state || {}
  const chips = (p.sources || []).filter((x) => x.kind === 'teams_chat').map((x) => {
    const s = cs[x.id] || {}
    const info = s.error ? `Couldn't read it: ${s.error}` : s.read_at ? `Read ${relDay(s.read_at.slice(0, 10))} · ${s.days || 0} day${s.days === 1 ? '' : 's'} of messages between calls go into the work` : 'Read into the work between calls — at the next sync'
    return tok(`💬 ${esc(x.name || 'Meeting chat')}${s.error ? ' <span class="src-err">⚠</span>' : ''}`, { op: 'source-remove', kind: 'teams_chat', source: x.id }, info)
  }).join('')
  const offer = (p.chat_suggestions || []).map((c) => `<button class="tok add" data-pj="source-add" data-kind="teams_chat" data-source="${esc(c.id)}" data-name="${esc(c.name)}" title="The chat of ${c.calls} of its call${c.calls === 1 ? '' : 's'}: what is said there between calls goes into the work">+ 💬 ${esc(c.name)} chat</button>`).join('')
  return chips + offer
}
function threadSources(p, cl) {
  if (!brainOn()) {
    const kept = (p.sources || []).filter((x) => x.kind !== 'teams_chat').map((x) => tok(`${{ teams_channel: '#', mail: '✉', folder: '▣' }[x.kind] || '·'} ${esc(x.kind === 'mail' ? sourceLabel(x) : x.name || x.id)}`, null)).join('')
    return chatSources(p) + kept + `<div class="pf-sub">The Brain is off, so channels, mail and folders aren't read into it — turn it on in Settings → Brain. Meeting chats, and mail already added here, are read into the work either way.</div>`
  }
  loadSourceState()
  const st = (m365.state && m365.state.state) || {}
  const icon = { teams_channel: '#', mail: '✉', teams_chat: '💬', folder: '▣' }
  const chip = (x) => {
    if (x.kind === 'folder') {
      const f = folderSummary(x.id)
      const ex = (x.exclude || []).length ? ` · not reading ${x.exclude.join(', ')}` : ''
      return tok(`▣ ${esc(x.name || x.id.split(/[\\/]/).pop())}${f.err ? ' <span class="src-err">⚠</span>' : ''}`, { op: 'source-remove', kind: 'folder', source: x.id }, `${x.id}\n${f.line}${ex}`)
        + (S.pj.adding === `exclude:${x.id}` ? `<span class="tok-in"><input id="pj-add-exclude" data-add="exclude" data-root="${esc(x.id)}" value="${esc((x.exclude || []).join(', '))}" placeholder="Don't read — e.g. Archive/, *draft*" aria-label="What not to read" autofocus></span>` : `<button class="tok add" data-pj="adding" data-what="exclude:${esc(x.id)}" title="Folders or names in it not to read">Don't read…</button>`)
    }
    const s = st[`${x.kind}:${x.id}`] || {}
    let info = s.error ? `Couldn't read it: ${s.error}` : s.read_at ? `Read ${relDay(s.read_at.slice(0, 10))} · ${s.found || 0} new` : 'Not read yet'
    // Mail also goes into the work (`ledger::mail_sync`): trusted senders' mails, one item each.
    const w = x.kind === 'mail' ? (p.mail_state || {})[x.id] : null
    if (w) info += w.error ? `\nInto the work: couldn't read it — ${w.error}` : w.read_at ? `\nInto the work: read ${relDay(w.read_at.slice(0, 10))} · ${w.found || 0} from trusted senders` : ''
    return tok(`${icon[x.kind] || '·'} ${esc(sourceLabel(x))}${s.error || (w && w.error) ? ' <span class="src-err">⚠</span>' : ''}`, { op: 'source-remove', kind: x.kind, source: x.id }, info)
  }
  const have = new Set((p.sources || []).map((x) => `${x.kind}:${x.id}`))
  const signedIn = !m365.state || m365.state.signed_in
  const folderBtn = `<button class="tok add" data-pj="source-folder" title="A folder of this thread's documents">▣ Folder</button>`
  const others = (p.sources || []).filter((x) => x.kind !== 'teams_chat')
  if (!signedIn) return chatSources(p) + others.map(chip).join('') + folderBtn + `<div class="pf-sub">Channels, mail and meeting chats need Microsoft 365 — sign in under Settings → Calendar & chat.</div>`
  const suggest = ((cl && cl.domains) || []).filter((d) => !have.has(`mail:domain:${d}`)).map((d) => `<button class="tok add" data-pj="source-add" data-kind="mail" data-source="${esc(d)}" title="Mail with anyone at ${esc(d)}">+ mail with ${esc(d)}</button>`).join('')
  let channel = `<button class="tok add" data-pj="adding" data-what="channel"># Channel</button>`
  if (S.pj.adding === 'channel') {
    if (!m365.teams) { m365.teams = []; get('api/m365/teams').then((d) => { m365.teams = Array.isArray(d) ? d : { error: d.error }; render() }).catch(() => {}) }
    const team = S.pj.srcTeam
    const chans = team ? m365.channels[team] : null
    if (team && !chans) { m365.channels[team] = []; get('api/m365/channels?team=' + encodeURIComponent(team)).then((d) => { m365.channels[team] = Array.isArray(d) ? d : []; render() }).catch(() => {}) }
    const teams = Array.isArray(m365.teams) ? m365.teams : []
    channel = `<span class="tok-in"><select data-src-team aria-label="Team" autofocus><option value="">${teams.length ? 'Team…' : m365.teams && m365.teams.error ? 'Teams unavailable' : 'Loading teams…'}</option>${teams.map((t) => `<option value="${esc(t.id)}"${t.id === team ? ' selected' : ''}>${esc(t.name)}</option>`).join('')}</select>${team ? `<select data-src-channel aria-label="Channel"><option value="">${chans && chans.length ? 'Channel…' : 'Loading…'}</option>${(chans || []).filter((c) => !have.has(`teams_channel:${team}|${c.id}`)).map((c) => `<option value="${esc(c.id)}" data-name="${esc(teams.find((t) => t.id === team).name + ' › ' + c.name)}">${esc(c.name)}${c.membership === 'private' ? ' (private)' : ''}</option>`).join('')}</select>` : ''}</span>`
  }
  const mail = S.pj.adding === 'mail' ? `<span class="tok-in"><input id="pj-add-mail" data-add="mail" placeholder="A domain or an address — acme.example, jane@acme.example" aria-label="Mail with" autofocus></span>` : `<button class="tok add" data-pj="adding" data-what="mail">✉ Mail</button>`
  return chatSources(p) + others.map(chip).join('') + suggest + channel + mail + folderBtn
    + `<div class="pf-sub">A meeting chat goes into the work between calls (what was done, moved or blocked). Channels, mail and folders are read into the Brain, never copied: each section links to the post, the mail or the file's page. Mail is matched by its people's addresses; a folder's pages that their text misses (diagrams, timelines) are read from their images.</div>`
}
document.addEventListener('change', (e) => {
  const t = e.target
  if (t.matches && t.matches('[data-src-team]')) { S.pj.srcTeam = t.value || null; return render() }
  if (t.matches && t.matches('[data-src-channel]') && t.value && S.pj.srcTeam) {
    const name = t.selectedOptions[0].dataset.name
    projectSend('source_add', { kind: 'teams_channel', source: `${S.pj.srcTeam}|${t.value}`, name })
    S.pj.adding = null; S.pj.srcTeam = null
    setTimeout(() => loadSourceState(true), 1500)
    return render()
  }
})

function projectProfile(p, data) {
  const id = esc(p.id)
  const row = (label, html) => `<div class="pf-row"><span class="pf-l">${esc(label)}</span><div class="pf-v">${html}</div></div>`
  // Stage: the three groups side by side; its history as one line.
  const at = (st) => { for (let i = (p.stages || []).length - 1; i >= 0; i--) if (p.stages[i].stage === st) return i; return -1 }
  const passed = new Set((p.stages || []).map((c) => c.stage))
  const groups = ProjectArt.STAGES.map(([, glabel, xs]) => `<div class="pf-sg"><span>${esc(glabel)}</span><div class="seg">${xs.map(([sid, label]) => `<button data-pj="stage" data-stage="${sid}" aria-pressed="${p.stage === sid}" class="${p.stage !== sid && passed.has(sid) ? 'passed' : ''}">${esc(label)}</button>`).join('')}</div></div>`).join('')
  const st = p.stages || []
  const hist = st.length ? [`${ProjectArt.stage(p.stage).label} since ${shortDay(st[st.length - 1].at)}`, ...st.slice(0, -1).reverse().slice(0, 3).map((c, i, arr) => { const j = st.length - 2 - i; return `${ProjectArt.stage(c.stage).label} ${shortDay(c.at)} → ${shortDay(st[j + 1].at)}` })].join(' · ') : ''
  const reason = st.length && st[st.length - 1].reason ? ` (${esc(st[st.length - 1].reason)})` : ''
  // Client: its chip (opens it) and domains; changed through the chip's menu.
  const clients = data.clients || []
  const cl = projClient(p.client)
  const clientPick = `<label class="fchip">${cl ? 'Change' : 'Choose'} ▾<select data-pj-field="client" data-id="${id}" aria-label="Client"><option value=""${!p.client ? ' selected' : ''}>No client</option>${clients.map((c) => `<option value="${esc(c.id)}"${p.client === c.id ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}<option value="__new">+ New client…</option></select></label>`
  const newClient = S.pj.newClient ? `<div class="pj-inline"><input id="pj-client-name" placeholder="Client name" aria-label="Client name" autofocus><input id="pj-client-domains" placeholder="Mail domains, e.g. northwind.example" aria-label="Mail domains"><button class="btn primary" data-pj="client-add">Add</button><button class="btn ghost" data-pj="client-cancel">Cancel</button></div>` : ''
  // Counterparts: organisations the thread is with besides its client (their people, the people rule).
  const allOrgs = data.orgs || clients
  const mine = (p.orgs || []).map((o) => allOrgs.find((c) => c.id === o.client) || { id: o.client, name: o.client, domains: [] })
  const orgPick = S.pj.adding === 'org' ? `<span class="tok-in"><select id="pj-org-pick" aria-label="Organisation"><option value="">Organisation…</option>${allOrgs.filter((c) => c.id !== p.client && !c.internal && !mine.some((m) => m.id === c.id)).map((c) => `<option value="${esc(c.id)}">${esc(c.name)}${c.domains.length ? ` · ${esc(c.domains[0])}` : ''}</option>`).join('')}<option value="__new">+ New organisation…</option></select></span>`
    : '<button class="tok add" data-pj="adding" data-what="org">+ Organisation</button>'
  const newOrg = S.pj.newOrg ? `<div class="pj-inline"><input id="pj-org-name" placeholder="Organisation, e.g. Globex" aria-label="Organisation name" autofocus><input id="pj-org-domains" placeholder="Mail domains, e.g. globex.example" aria-label="Mail domains"><button class="btn primary" data-pj="org-new">Add</button><button class="btn ghost" data-pj="org-new-cancel">Cancel</button></div>` : ''
  const orgsRow = mine.map((c) => tok(esc(c.name), { op: 'org-unlink', org: c.id }, (c.domains || []).join(', '))).join('') + orgPick + newOrg
    + '<div class="pf-sub">Who this thread is with besides its client: their people are theirs in People, and the people rule counts their domains</div>'
  const clientRow = (cl ? `<button class="tok link" data-pj="client" data-id="${esc(cl.id)}" title="Open ${esc(cl.name)}">${clientIcon(cl, 14)}${esc(cl.name)}</button><span class="pf-sub pf-inl">${cl.domains.length ? `its domains: ${esc(cl.domains.join(', '))}` : 'no mail domains yet — the people rule needs them'}</span>` : '<span class="pf-sub pf-inl">No client — internal work, or not set</span>') + clientPick + newClient
  // Since: text; click to change.
  // Documents, posts and mail dated before this are history in the Brain: kept, not searched unless asked.
  const history = S.pj.pfEdit === 'history' ? `<input type="date" class="pf-in" data-pj-field="history_before" data-id="${id}" value="${esc(p.history_before || '')}" aria-label="History before" autofocus>`
    : `<button class="pf-text" data-pj="pf-edit" data-what="history">${p.history_before ? `Before ${esc(new Date(p.history_before + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' }))}` : 'None'}</button><span class="pf-sub pf-inl">· documents, posts and mail dated before it are history: kept, searched only when history is asked for</span>`
  const since = S.pj.pfEdit === 'since' ? `<input type="date" class="pf-in" data-pj-field="since" data-id="${id}" value="${esc(p.since || '')}" aria-label="Since" autofocus>`
    : `<button class="pf-text" data-pj="pf-edit" data-what="since">${p.since ? esc(new Date(p.since + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' })) : 'From its first call'}</button><span class="pf-sub pf-inl">· nothing older is read</span>`
  // Calls: the rules as tokens; how many match.
  const pins = (p.pins || []).length, outs = (p.exclusions || []).length
  const ruleKind = `<select id="pj-add-kind" aria-label="Kind of rule"><option value="series">Meeting series</option><option value="people">The client's people</option><option value="subject">Subject contains</option><option value="chat">Teams chat id</option></select>`
  const calls = (p.rules || []).map((r, i) => tok(RULE_TOKEN(r, p.client_name), { op: 'rule-remove', index: i })).join('') + addTok('rule', "a meeting's subject (empty for the client's people)", ruleKind)
    + `<div class="pf-sub">${p.call_count} call${p.call_count === 1 ? '' : 's'} match${pins || outs ? ` · ${[pins ? `${pins} pinned` : '', outs ? `${outs} taken out by hand` : ''].filter(Boolean).join(', ')}` : ''}</div>`
  const sources = threadSources(p, cl)
  const people = (p.people || []).map((m) => tok(`${esc(m.name)}${m.role ? ` · ${esc(m.role)}` : ''}`, { op: 'person-remove', name: m.name, email: m.email || '' }, m.email || '')).join('') + addTok('person', 'Name, email, role — e.g. Dana Kowal, dana@x.com, sponsor')
  const terms = threadBrain(p) || ((p.terms || []).slice(0, 24).map((t) => tok(esc(t.term), { op: 'term-remove', term: t.term }, [t.kind, t.note].filter(Boolean).join(' · '))).join('') + addTok('term', 'A term — a system, an acronym, a name')
    + `<div class="pf-sub">Hint transcription on its calls${(p.terms || []).length > 24 ? ` · ${p.terms.length - 24} more` : ''}</div>`)
  const notes = S.pj.pfEdit === 'notes' ? `<textarea class="pf-in pj-notes" id="pj-f-notes" data-pj-field="notes" data-id="${id}" rows="3" aria-label="Notes for Claude" autofocus>${esc(p.notes || '')}</textarea>`
    : `<button class="pf-text${p.notes ? '' : ' empty'}" data-pj="pf-edit" data-what="notes">${p.notes ? esc(p.notes) : 'What the minutes and Claude should know — what people call things, what to leave out'}</button>`
  // Look and AI: not on the board; the same row style.
  const symbols = `<button class="pj-pick${!p.icon ? ' on' : ''}" data-pj="icon" data-icon="" title="Its pattern" aria-label="Its pattern">${projIcon({ ...p, icon: null, has_logo: false }, 22)}</button>` +
    (cl && cl.logo_v ? `<button class="pj-pick${p.icon === 'client' ? ' on' : ''}" data-pj="icon" data-icon="client" title="The client's logo" aria-label="The client's logo">${projIcon({ ...p, icon: 'client', has_logo: false, client_logo_v: cl.logo_v }, 22)}</button>` : '') +
    Object.keys(PJ_SYMBOLS).map((k) => `<button class="pj-pick${p.icon === k ? ' on' : ''}" data-pj="icon" data-icon="${k}" title="${k}" aria-label="${k}">${projIcon({ ...p, icon: k, has_logo: false }, 22)}</button>`).join('')
  const colors = PJ_COLORS.map((c) => `<button class="pj-swatch${p.color === c ? ' on' : ''}" style="background:${c}" data-pj="color" data-color="${c}" aria-label="Colour ${c}"></button>`).join('')
  const look = `<div class="pf-look">${symbols}</div><div class="pf-look"><span class="pj-swatches">${colors}</span><span class="pj-logo-drop" data-drop-op="logo" data-id="${id}" title="Drop an image here">${p.has_logo ? projIcon(p, 22) : '<span class="pj-logo-empty">logo</span>'}</span>${logoPicker('logo', p.id, p.has_logo ? 'Change logo…' : 'Use a logo…')}${p.has_logo ? '<button class="link-btn" data-pj="logo-remove">Remove</button>' : ''}</div>`
  const profiles = ai().profiles || []
  const aiRow = `<label class="fchip">${esc(p.ai_profile ? profileName(p.ai_profile) : `The default · ${profileName(ai().default) || 'built in'}`)} ▾<select data-pj-field="ai_profile" data-id="${id}" aria-label="AI profile"><option value="">The default (${esc(profileName(ai().default) || 'built in')})</option>${profiles.map((x) => `<option value="${esc(x.id)}"${p.ai_profile === x.id ? ' selected' : ''}>${esc(x.name)}</option>`).join('')}</select></label>`
    + (S.pj.pfEdit === 'model' ? `<input class="pf-in" data-pj-field="model" data-id="${id}" value="${esc(p.model || '')}" placeholder="sonnet" aria-label="Model" autofocus>` : `<button class="tok" data-pj="pf-edit" data-what="model">model: ${esc(p.model || "the profile's minutes model")}</button>`)
    + `<div class="pf-sub">What Claude runs on for this thread's sync and questions, whichever profile a call was recorded on.</div>`
  return `<div class="pf-card">
    ${row('Stage', `<div class="pf-stages">${groups}</div><div class="pf-sub">${esc(hist)}${reason}</div>`)}
    ${row('Client', clientRow)}
    ${row('Organisations', orgsRow)}
    ${row('Since', since)}
    ${brainOn() ? row('History', history) : ''}
    ${row('Calls', calls)}
    ${row('Sources', sources)}
    ${row('People', people)}
    ${row(S.pj.tbrain && S.pj.tbrain.taxonomy && S.pj.tbrain.taxonomy.exists ? 'Topics & terms' : 'Terms', terms)}
    ${row('Notes for Claude', notes)}
    ${row('Look', look)}
    ${row('AI', aiRow)}
  </div>
  <p class="pf-foot">Saved as you change it · <code>projects/${id}/</code> · Delete in ⋯</p>`
}

/** A client's page (`#client/<id>`): its names, domains, whether it is the user's own company, its logo and projects. */
function clientView(data) {
  const c = (data.clients || []).find((x) => x.id === S.pj.client)
  if (!c) return `<div class="empty"><div><h3>This client is gone</h3><p><a href="#projects">Back to the threads</a></p></div></div>`
  const id = esc(c.id)
  const row = (label, html) => `<div class="pf-row"><span class="pf-l">${esc(label)}</span><div class="pf-v">${html}</div></div>`
  const ps = (data.projects || []).filter((p) => p.client === c.id)
  const name = S.pj.pfEdit === 'cname' ? `<input class="pf-in" id="pj-cname" data-client-field="name" data-id="${id}" value="${esc(c.name)}" aria-label="Name" autofocus>` : `<button class="pf-text" data-pj="pf-edit" data-what="cname">${esc(c.name)}</button>`
  const aliases = (c.aliases || []).map((a) => tok(esc(a), { op: 'client-list-remove', list: 'aliases', value: a })).join('') + addTok('alias', 'Another name — e.g. NW, Northwind Group')
  const domains = (c.domains || []).map((d) => tok(esc(d), { op: 'client-list-remove', list: 'domains', value: d })).join('') + addTok('domain', 'A mail domain — e.g. northwind.example')
    + `<div class="pf-sub">${c.internal ? 'Its people are your colleagues: they never count as a client\'s people.' : 'A call is this client\'s when half of its outside people come from these domains.'}</div>`
  const internal = `<button class="sw${c.internal ? ' on' : ''}" role="switch" aria-checked="${!!c.internal}" aria-label="Your own company" data-pj="client-internal" data-on="${!c.internal}"></button><span class="pf-sub">${c.internal ? 'Your own company (internal work)' : 'A client, partner or vendor'}</span>`
  const logo = `<div class="pf-look"><span class="pj-logo-drop" data-drop-op="client_logo" data-id="${id}" title="Drop an image here">${clientIcon(c, 28) || '<span class="pj-logo-empty">logo</span>'}</span>${logoPicker('client_logo', c.id, c.logo_v ? 'Change logo…' : 'Add a logo…')}${c.logo_v ? `<button class="link-btn" data-pj="client-logo-remove" data-id="${id}">Remove</button>` : ''}</div><div class="pf-sub">PNG, JPEG or WebP, up to 512 KB — shown beside its name and on its threads that wear it.</div>`
  const projects = ps.map((p) => `<button class="pf-proj" data-pj="open" data-id="${esc(p.id)}">${projIcon(p, 18)}<span class="t">${esc(p.name)}</span>${stageTag(p.stage)}<span class="pf-sub">${p.call_count} call${p.call_count === 1 ? '' : 's'}</span></button>`).join('')
    + (S.pj.adding === 'cproject' ? `<span class="tok-in"><input id="pj-add-cproject" data-add="cproject" placeholder="New thread for ${esc(c.name)}" aria-label="New thread name" autofocus></span>` : `<button class="tok add" data-pj="adding" data-what="cproject">+ New thread</button>`)
  const del = S.pj.confirmDelete ? `<span class="pf-sub">Remove ${esc(c.name)}?</span><button class="btn ghost" data-pj="delete-cancel">Cancel</button><button class="btn danger" data-pj="client-remove" data-id="${id}">Remove</button>`
    : ps.length ? `<span class="pf-sub">A client with threads stays: move or delete them first.</span>` : `<button class="btn ghost danger" data-pj="delete-ask">Remove this client…</button>`
  const cd = S.pj.cdata && S.pj.cdata.client && S.pj.cdata.client.id === c.id ? S.pj.cdata : null
  const openTasks = cd && cd.work ? TASK_PILES.reduce((n, [k]) => n + ((cd.work.piles || {})[k] || []).length, 0) : 0
  const tabs = [['overview', 'Overview'], ['calls', `Calls${cd && cd.calls ? `<span class="n">${cd.calls.length}</span>` : ''}`], ['tasks', `Tasks${openTasks ? `<span class="n">${openTasks}</span>` : ''}`], ['people', `People${peopleCount() ? `<span class="n">${peopleCount()}</span>` : ''}`], ['profile', 'Profile']]
  const tab = tabs.some(([t]) => t === S.pj.ctab) ? S.pj.ctab : 'overview'
  const head = `<header class="pj-head"><div class="pj-title">${clientIcon(c, 34) || `<span class="pj-ic pf-cic" style="--pj:#8a8a95;width:34px;height:34px">${esc(c.name.slice(0, 1).toUpperCase())}</span>`}<div style="min-width:0"><div class="pj-name"><h2>${esc(c.name)}</h2><span class="tag">${c.internal ? 'your company' : 'client'}</span></div><div class="facts"><span>${ps.length} thread${ps.length === 1 ? '' : 's'}${c.domains.length ? ` · ${esc(c.domains.join(', '))}` : ''}</span></div></div><span class="grow"></span><button class="btn" data-claude-about title="Claude, with the Brain, about ${esc(c.name)}">Claude about ${esc(c.name)}</button></div>
    <div class="tabs" role="tablist">${tabs.map(([t, label]) => `<button class="tab" role="tab" data-pj="ctab" data-tab-id="${t}" aria-selected="${tab === t}">${label}</button>`).join('')}</div>${notices()}</header>`
  if (tab !== 'profile') {
    if (!cd) return `${head}<div class="pane pj-pane"><div class="skeleton">${'<i></i>'.repeat(6)}</div></div>`
    if (cd.error) return `${head}<div class="pane pj-pane"><p class="error">${esc(cd.error)}</p></div>`
    return `${head}<div class="pane pj-pane${tab === 'tasks' || tab === 'people' ? ' wide' : ''}">${tab === 'calls' ? clientCalls(cd) : tab === 'tasks' ? clientTasks(cd) : tab === 'people' ? peopleView() : clientOverview(cd, c)}</div>`
  }
  return `${head}
  <div class="pane pj-pane"><div class="pf-card">
    ${row('Name', name)}
    ${row('Other names', aliases)}
    ${row('Mail domains', domains)}
    ${row('Kind', internal)}
    ${row('Logo', logo)}
    ${row('Threads', projects)}
    ${row('', del)}
  </div><p class="pf-foot">Saved as you change it · <code>projects/clients.json</code></p></div>`
}

/** A client's Overview: its description (the user's, edited in place as a thread's brief) and a card
 *  per thread — stage, calls, stories done, open tasks by who has to move, the latest decisions,
 *  what's open (risks, questions). */
function clientOverview(cd, c) {
  const desc = S.pj.briefEdit != null
    ? `<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>About ${esc(c.name)}</b><div class="set-sub">Yours: who they are, what you do for them, who matters. Markdown.</div></div><div class="set-ctl"><button class="btn ghost" data-pj="brief-cancel">Cancel</button><button class="btn primary" data-pj="brief-save">Save</button></div></div>
      <div class="me-edit"><textarea id="pj-brief" spellcheck="true" aria-label="About the client (Markdown)">${esc(S.pj.briefEdit)}</textarea><div class="set-sub">⌘S / Ctrl+S saves, Esc cancels.</div></div></div></section>`
    : `<section class="set-sec"><div class="set-card"><div class="set-row"><div><b>About ${esc(c.name)}</b></div><div class="set-ctl"><button class="btn" data-pj="brief-edit">Edit</button></div></div>${cd.brief && cd.brief.trim() ? `<div class="doc me-doc">${cd.brief_html}</div>` : '<div class="me-doc set-sub">Nothing yet — who they are, what you do for them, who matters.</div>'}</div></section>`
  const PILE_SHORT = { mine: 'I owe', owed: 'owed to me', waiting: 'waiting', nobody: 'nobody named', others: 'others' }
  const card = (p) => {
    const s = p.summary || {}, pl = s.piles || {}
    const tasks = TASK_PILES.filter(([k]) => pl[k]).map(([k]) => `<span><b>${pl[k]}</b> ${PILE_SHORT[k]}</span>`).join('')
    // Each with what it holds: the decision and who took it; a risk's response — none stands out.
    const recLine = (x, cls) => {
      const sub = cls === 'risk' ? (x.what ? `↳ ${x.what}` : 'no response yet') : x.what ? `${x.what}${x.by ? ` — ${x.by}` : ''}` : ''
      const lvl = cls === 'risk' && (x.likelihood || x.impact) ? ` <span class="set-sub">${[x.likelihood && `likelihood ${LEVEL_LABEL[x.likelihood]}`, x.impact && `impact ${LEVEL_LABEL[x.impact]}`].filter(Boolean).join(' · ')}</span>` : ''
      return `<button class="cl-rec${cls === 'risk' && !x.what ? ' bare' : ''}" data-pj="goto-node" data-id="${esc(p.id)}" data-node="${esc(x.id)}">${esc(x.title)}${lvl}${sub ? `<span class="cl-rec-sub">${esc(sub)}</span>` : ''}</button>`
    }
    const recs = (label, xs, cls) => xs && xs.length ? `<div class="cl-recs"><span class="cl-rl ${cls}">${label}</span><div>${xs.slice(0, 3).map((x) => recLine(x, cls)).join('')}${xs.length > 3 ? `<span class="set-sub">+ ${xs.length - 3} more</span>` : ''}</div></div>` : ''
    const prog = s.stories ? `<div class="cl-prog"><span class="bd-prog"><span style="width:${Math.round(100 * s.stories_done / s.stories)}%"></span></span><span class="set-sub">${s.stories_done} of ${s.stories} stor${s.stories === 1 ? 'y' : 'ies'} done</span></div>` : ''
    return `<div class="cl-card${p.group === 'closed' ? ' closed' : ''}">
      <button class="cl-top" data-pj="open" data-id="${esc(p.id)}">${projIcon(p, 22)}<b>${esc(p.name)}</b>${stageTag(p.stage)}</button>
      <div class="set-sub">${p.call_count} call${p.call_count === 1 ? '' : 's'}${p.last_call ? ` · last ${esc(dayLabel(new Date(p.last_call)).toLowerCase())}` : ''}</div>
      ${prog}
      ${tasks || s.late ? `<div class="cl-tasks">${tasks}${s.late ? `<span class="late">${s.late} late</span>` : ''}</div>` : '<div class="cl-tasks none">No open tasks</div>'}
      ${recs('Decided', s.decisions, '')}${recs('Risks', s.risks, 'risk')}${recs('Open questions', s.questions, '')}
    </div>`
  }
  const open = cd.threads.filter((p) => p.group !== 'closed'), closed = cd.threads.filter((p) => p.group === 'closed')
  return `${insightsBlock('client', c.id, c.name)}${desc}<section class="set-sec"><h3>Threads</h3>${cd.threads.length ? `<div class="cl-grid">${open.map(card).join('')}${closed.map(card).join('')}</div>` : '<div class="set-card"><div class="set-row"><div class="set-sub">No threads for this client yet — add one on Profile.</div></div></div>'}</section>`
}
/** Every call of the client's threads, newest first, with the threads each is in. */
function clientCalls(cd) {
  const rows = cd.calls.map((c) => {
    const d = c.started_at ? new Date(c.started_at) : null
    const chips = (c.threads || []).map((id) => projById(id)).filter(Boolean).map((p) => `<span class="tk-pj">${projIcon(p, 12)}${esc(p.name)}</span>`).join('')
    return `<div class="set-row"><div><button class="link pj-call-t" data-call="${esc(c.name)}">${esc(c.title || c.name)}</button><div class="set-sub">${d ? `${esc(dayLabel(d))}, ${esc(hm(d))}` : ''}${c.minutes ? '' : ' · no minutes yet'}</div></div><div class="set-ctl cl-chips">${chips}</div></div>`
  }).join('')
  return `<section class="set-sec"><div class="set-card">${rows || '<div class="set-row"><div class="set-sub">No calls in this client\'s threads yet.</div></div>'}</div></section>`
}
/** The Tasks activity's piles for this client's open threads. */
function clientTasks(cd) {
  const cols = TASK_PILES.map(([k, label]) => {
    const all = (cd.work.piles || {})[k] || []
    return `<section class="tk-col"><h3>${esc(label)} <span>${all.length}</span></h3>${all.map((c) => taskCard(c, k)).join('') || '<p class="tk-none">Nothing here</p>'}</section>`
  }).join('')
  return `<div class="tk-cols">${cols}</div>${othersPile((cd.work.piles || {}).others || [], 'client-others')}`
}

// ── People: stakeholders around a thread or a client (`people.rs`) ────────────────────────
// Facts from the calls (computed by the app every time) beside what the user set: a Map
// (influence × interest), the org chart, a List; the person in the right panel.
const STANCES = ['champion', 'supporter', 'neutral', 'sceptic', 'blocker']
/** Relationships between organisations (`projects::RELATIONS`), as a sentence reads them. */
const REL_KINDS = [['partner', 'partner of'], ['vendor', 'vendor to'], ['delivers_for', 'delivers for'], ['integrates_for', 'integrates for'], ['customer', 'customer of'], ['competitor', 'competitor of']]
const MATTERS = ['critical', 'high', 'normal', 'irrelevant']
const peopleData = () => { if (S.view === 'brain') return S.br.people && S.br.people.people ? S.br.people : null; const d = S.pj.people; const want = S.pj.client ? `api/client/${encodeURIComponent(S.pj.client)}/people` : `api/project/${encodeURIComponent(S.pj.sel)}/people`; return d && d.url === want ? d : null }
const peopleCount = () => { const d = peopleData(); return d && d.people ? d.people.filter((x) => x.in_thread).length : 0 }
const ppView = () => { try { return localStorage.getItem('echo.people.view') || 'map' } catch { return 'map' } }
const initials = (n) => (n || '?').split(/[\s@._-]+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('')
/** Where a person or organisation stands in this view: the thread's own place; on a client, the
 *  thread where they weigh most. */
function placeOf(x) {
  const ps = (x.places || []).filter((p) => p.influence || p.interest || p.stance || p.role)
  if (!ps.length) return x.places && x.places[0] ? x.places[0] : null
  return ps.slice().sort((a, b) => (b.influence || 0) - (a.influence || 0))[0]
}
const ppAv = (name, stance, org) => `<span class="pp-av${org ? ' org' : ''} ${esc(stance || 'none')}" aria-hidden="true">${esc(initials(name))}</span>`
const sideLabel = (x) => x.side === 'yours' ? 'your company' : x.org ? x.org.name : x.domain || 'outside'
function peopleView() {
  const d = peopleData()
  if (!d) return `<div class="skeleton">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<p class="error">${esc(d.error)}</p>`
  const v = ppView()
  const legend = STANCES.map((s) => `<span class="pp-leg"><i class="pp-av ${s}"></i>${s}</span>`).join('')
  const seg = `<div class="pp-seg" role="group" aria-label="View">${[['map', 'Map'], ['org', 'Org chart'], ['list', 'List']].map(([k, l]) => `<button data-pp="view" data-v="${k}" aria-pressed="${v === k}">${l}</button>`).join('')}</div>`
  const add = S.pj.client ? '' : S.pj.ppAdding ? `<span class="tok-in"><input id="pp-new" placeholder="Name, or name &lt;email&gt;" aria-label="New person" autofocus></span>` : '<button class="btn" data-pp="add">+ Person</button>'
  const bar = `<div class="pp-bar">${seg}<span class="grow"></span>${legend}${add}</div>`
  const body = v === 'list' ? peopleList(d) : v === 'org' ? peopleOrg(d) : peopleMap(d)
  return `<div class="pp">${bar}${peopleSuggestions(d)}${body}</div>`
}
/** What the sync's people pass proposed, with the words: Yes applies it as yours, No turns it down
 *  (never proposed again), Change… applies your own version instead (the proposed one stays turned
 *  down). Three at a time; the one being changed is always shown. */
function peopleSuggestions(d) {
  const all = d.suggestions || []
  if (!all.length) return ''
  const LV = { 1: 'low', 2: 'mid', 3: 'high' }
  const ask = (x) => {
    const n = `<b>${esc(x.target.name || '')}</b>`, v = esc(x.value)
    switch (x.field) {
      case 'stance': return `Is ${n} a <b>${v}</b>?`
      case 'role': return `${n}'s role here: <b>${v}</b>?${x.was ? ` <span class="set-sub">(was “${esc(x.was)}”)</span>` : ''}`
      case 'person': return `${n}${x.target.org_name ? ` (${esc(x.target.org_name)})` : ''} — mentioned${x.at && x.at.title ? ` in ${esc(x.at.title)}` : ''}${(x.mentions || 1) > 1 ? `, ${x.mentions} times` : ''}${x.role ? `, ${esc(x.role)}` : ''}: add?`
      case 'influence': case 'interest': return `${n}'s ${x.field}: <b>${LV[x.value] || v}</b>?`
      case 'title': return `${n} is <b>${v}</b>?`
      case 'group': return `${n} is in <b>${v}</b>?`
      case 'reports_to': return `${n} reports to <b>${esc(x.value_name || x.value)}</b>?`
      case 'left': return `Has ${n} left?`
      case 'relation': return `${n} <b>${esc(x.value.replace('_', ' '))}</b> ${esc(x.target.to_name || '')}?`
    }
    return `${n}: ${esc(x.field)} = <b>${v}</b>?`
  }
  /** The control for your own version: the field's choices, else the proposed text to correct. */
  const editor = (x) => {
    const sel = (opts) => `<select id="pp-sug-v" aria-label="Your version">${opts.map(([v, l]) => `<option value="${esc(v)}"${String(v) === String(x.value) ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>`
    switch (x.field) {
      case 'stance': return sel(STANCES.map((v) => [v, v]))
      case 'influence': case 'interest': return sel([1, 2, 3].map((n) => [n, LV[n]]))
      case 'relation': return sel(REL_KINDS)
      case 'reports_to': return sel(d.people.filter((p) => p.key !== x.target.key).map((p) => [p.key, p.name]))
    }
    return `<input id="pp-sug-v" value="${esc(x.value)}" maxlength="80" aria-label="Your version" autofocus>`
  }
  const row = (x) => {
    const at = x.at || {}
    const src = [at.title, at.date ? relDay(at.date) : '', at.ts ? String(at.ts).replace(/^00:/, '') : ''].filter(Boolean).join(' · ')
    const st = x.field === 'stance' ? x.value : null
    if (S.pj.ppSugEdit === x.sid) {
      return `<div class="pp-sug">${ppAv(x.target.name || '?', st, x.field === 'relation')}<span class="grow">${ask(x).replace(/\?$/, '')} → ${editor(x)} <span class="pp-quote">“${esc(x.quote || '')}”</span></span><button class="btn primary" data-pp="sug-save" data-sid="${esc(x.sid)}" data-thread="${esc(x.thread)}">Save</button><button class="btn ghost" data-pp="sug-cancel">Cancel</button></div>`
    }
    return `<div class="pp-sug">${ppAv(x.target.name || '?', st, x.field === 'relation')}<span class="grow">${ask(x)} <span class="pp-quote">“${esc(x.quote || '')}”</span> ${at.call ? `<button class="link-btn pp-src" data-call="${esc(at.call)}">${esc(src)}</button>` : /^https:\/\//i.test(at.url || '') ? `<a class="link-btn pp-src" href="${esc(at.url)}" data-ext title="Open the mail in Outlook">✉ ${esc(src)}</a>` : `<span class="set-sub">${esc(src)}</span>`}</span><button class="btn primary" data-pp="decide" data-sid="${esc(x.sid)}" data-thread="${esc(x.thread)}" data-yes="1">Yes</button><button class="btn ghost" data-pp="decide" data-sid="${esc(x.sid)}" data-thread="${esc(x.thread)}" data-yes="">No</button>${x.field === 'left' ? '' : `<button class="btn ghost" data-pp="sug-edit" data-sid="${esc(x.sid)}" title="Not quite: put it right">Change…</button>`}</div>`
  }
  const shown = S.pj.ppAllSug || all.slice(3).some((x) => x.sid === S.pj.ppSugEdit) ? all : all.slice(0, 3)
  return `<div class="pp-sugs">${shown.map(row).join('')}${all.length > 3 ? `<button class="link-btn" data-pp="all-sug">${S.pj.ppAllSug ? 'Show fewer' : `${all.length - 3} more from the calls`}</button>` : ''}</div>`
}
function peopleMap(d) {
  const thread = !S.pj.client
  const people = d.people.filter((x) => x.in_thread)
  const orgs = d.orgs.filter((o) => (o.places || []).length)
  const items = [...people.map((x) => ({ x, org: false, pl: placeOf(x) })), ...orgs.map((o) => ({ x: o, org: true, pl: placeOf(o) }))]
  const hi = (n) => (n || 0) >= 2
  const placed = items.filter((i) => i.pl && i.pl.influence && i.pl.interest)
  const loose = items.filter((i) => !(i.pl && i.pl.influence && i.pl.interest))
  const chip = (i) => {
    const key = i.org ? `org:${i.x.id}` : i.x.key
    const sub = i.org ? (i.x.internal ? 'your company' : 'organisation') : (i.pl && i.pl.role) || i.x.title || sideLabel(i.x)
    return `<button class="pp-chip${S.pj.person === key ? ' on' : ''}${i.x.new ? ' new' : ''}" data-pp="pick" data-key="${esc(key)}"${thread ? ` draggable="true" data-pp-drag="${esc(key)}"` : ''} title="${esc(i.x.name)}">${ppAv(i.x.name, i.pl && i.pl.stance, i.org)}<span class="pp-cn">${esc(i.x.name)}</span><small>${esc(sub)}</small>${i.x.new ? '<span class="pp-new">new</span>' : ''}${i.x.quiet ? '<span class="pp-quiet" title="Not seen for six weeks while the calls went on">quiet</span>' : ''}</button>`
  }
  const Q = [['3', '1', 'Keep satisfied', 'high influence · low interest'], ['3', '3', 'Manage closely', 'high influence · high interest'], ['1', '1', 'Monitor', 'low influence · low interest'], ['1', '3', 'Keep informed', 'low influence · high interest']]
  const quad = ([inf, int, t, sub]) => {
    const xs = placed.filter((i) => hi(i.pl.influence) === (inf === '3') && hi(i.pl.interest) === (int === '3'))
    return `<section class="pp-q${inf === '3' && int === '3' ? ' key' : ''}" data-pp-drop="${inf}/${int}" aria-label="${esc(t)}"><div class="pp-qh"><b>${esc(t)}</b><span class="set-sub">${esc(sub)}</span></div><div class="pp-chips">${xs.map(chip).join('')}</div></section>`
  }
  const seenOnly = d.people.filter((x) => !x.in_thread).length
  const tray = loose.length ? `<div class="pp-tray" data-pp-drop="tray"><span class="cap">Not placed yet</span>${loose.map(chip).join('')}</div>` : ''
  const empty = !items.length ? `<div class="wk-empty"><b>Nobody in ${thread ? 'this thread' : "this client's threads"} yet</b><p>${seenOnly ? `${seenOnly} people were in its calls — add them from <button class="link-btn" data-pp="view" data-v="list">List</button>, then drag them where they stand.` : 'People join as its calls come in, or add someone with + Person.'}</p></div>` : ''
  return `${empty}${items.length ? `<div class="pp-grid"><div class="pp-axis y"><span>Influence →</span></div>${Q.map(quad).join('')}<span></span><div class="pp-axis x"><span>Interest →</span></div></div>${tray}` : ''}
    ${seenOnly ? `<p class="set-sub">${thread ? 'Drag a person to another quadrant. ' : ''}${seenOnly} more ${seenOnly === 1 ? 'was' : 'were'} in the calls and ${seenOnly === 1 ? "isn't" : "aren't"} in ${thread ? 'this thread' : 'a thread'} yet — <button class="link-btn" data-pp="view" data-v="list">List</button>.</p>` : ''}`
}
function peopleList(d) {
  const thread = !S.pj.client
  const row = (x) => {
    const pl = placeOf(x)
    const threads = !thread ? (x.places || []).map((p) => projById(p.thread)).filter(Boolean).map((p) => `<span class="tk-pj">${projIcon(p, 12)}${esc(p.name)}</span>`).join('') : ''
    return `<tr class="${S.pj.person === x.key ? 'on' : ''}" data-pp="pick" data-key="${esc(x.key)}">
      <td><span class="pp-who">${ppAv(x.name, pl && pl.stance)}<span><span class="pp-nm"><b>${esc(x.name)}</b>${x.new ? ' <span class="pp-new">new</span>' : ''}${x.quiet ? ' <span class="pp-quiet">quiet</span>' : ''}</span><span class="set-sub">${esc(x.title || x.email || '')}</span></span></span></td>
      <td>${esc(sideLabel(x))}${x.group ? `<span class="set-sub"> · ${esc(x.group)}</span>` : ''}</td>
      <td>${thread ? esc((pl && pl.role) || '') : threads}</td>
      <td>${pl && pl.stance ? `<span class="pp-leg"><i class="pp-av ${esc(pl.stance)}"></i>${esc(pl.stance)}</span>` : ''}</td>
      <td class="num">${x.calls}<span class="set-sub">/${x.of_calls}</span></td>
      <td class="num">${x.share ? `${x.share}%` : ''}</td>
      <td class="num">${x.owes ? `${x.owes}${x.late ? ` <span class="late">${x.late} late</span>` : ''}` : ''}</td>
      <td>${x.last ? esc(relDay(x.last)) : ''}</td>
      <td class="pp-act">${x.email ? `<a class="icon-btn" href="https://teams.microsoft.com/l/chat/0/0?users=${encodeURIComponent(x.email)}" data-ext title="Chat in Teams" aria-label="Chat with ${esc(x.name)} in Teams">${ICON('<path d="M5 18.5V7a2.5 2.5 0 0 1 2.5-2.5h9A2.5 2.5 0 0 1 19 7v6a2.5 2.5 0 0 1-2.5 2.5H9z"/>')}</a><button class="icon-btn" data-copy="${esc(x.email)}" title="Copy ${esc(x.email)}" aria-label="Copy ${esc(x.name)}'s email">${ICON('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>')}</button>` : ''}</td>
    </tr>`
  }
  const inn = d.people.filter((x) => x.in_thread), seen = d.people.filter((x) => !x.in_thread)
  const head = `<thead><tr><th>Person</th><th>Organisation</th><th>${thread ? 'Role' : 'Threads'}</th><th>Stance</th><th class="num">Calls</th><th class="num">Talk</th><th class="num">Owes</th><th>Last seen</th><th><span class="sr">Contact</span></th></tr></thead>`
  const seenBox = seen.length ? `<div class="pp-seen"><div class="pp-seen-h"><span class="cap">Seen in calls</span><span class="set-sub">${seen.length} ${seen.length === 1 ? "isn't" : "aren't"} in ${thread ? 'this thread' : 'a thread'} yet</span><span class="grow"></span>${thread ? `<button class="btn" data-pp="add-all">Add all</button>` : ''}</div>
    <table class="pp-tbl">${head}<tbody>${seen.map((x) => row(x).replace('<td class="pp-act">', `<td class="pp-act">${thread ? `<button class="btn ghost" data-pp="add-one" data-key="${esc(x.key)}">Add</button>` : ''}`)).join('')}</tbody></table></div>` : ''
  return `${inn.length ? `<div class="pp-card"><table class="pp-tbl">${head}<tbody>${inn.map(row).join('')}</tbody></table></div>` : `<p class="set-sub">Nobody in ${thread ? 'this thread' : "this client's threads"} yet.</p>`}${seenBox}`
}
/** The org chart (#43): who does what in this engagement, not who reports to whom (those lines are
 *  rarely known). Each organisation — yours first, then the others — has a governance band on top
 *  (sponsor, partner, programme lead, PM: marked, else high influence and high interest), then a
 *  column per workstream (the person's `group` in the thread), its lead on top — the one the others
 *  work for here — and the rest by title seniority; people inactive in the thread folded ("+N").
 *  Reporting lines are metadata: the card's tooltip, the panel, faint lines when asked for. In a
 *  thread, drag a card to a column (its workstream), to the band (governance), or onto a person
 *  (works for them here). */
// ── org chart layout (no DOM; org-chart.test.mjs runs app.js from here to the end mark) ──────────
/** Who goes where on the org chart (#46): each person's workstream — their saved one (the thread's
 *  `group`, else the directory's), else the one their role names ("<X> stream lead", a "<X>:" prefix
 *  two people share, a stream name in the role: "Design (CX)") —, whether they lead it (a "<X> stream
 *  lead" role, or a "lead" role in their saved stream), and whether they sit in the governance band:
 *  set explicitly, else high influence and interest for people with no workstream only — a stream's
 *  lead leads its column, not the band. `place(x)` = their place in this view. */
const OrgLayout = (() => {
  const reEsc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const streamOfRole = (role) => { const m = String(role || '').match(/(?:^|[:;(,]\s*)([A-Za-z][\w &/+-]{0,40}?)\s+(?:work)?stream\b/i); return m ? m[1].trim() : null }
  const isLead = (role) => /\blead\b/i.test(String(role || ''))
  const saved = (x, place) => place(x).group || x.group || ''
  /** Workstream names the thread gives (saved groups, "<X> stream" roles, a shared "<X>:" prefix) and
   *  whom a role puts in one. */
  function roleStreams(people, place) {
    const names = new Set(people.map((x) => saved(x, place)).filter(Boolean))
    const prefix = new Map()
    for (const x of people) {
      const r = String(place(x).role || '')
      const s = streamOfRole(r)
      if (s) names.add(s)
      const m = r.match(/^\s*([A-Z][\w &/+-]{1,40}):/)
      if (m) prefix.set(m[1].trim(), (prefix.get(m[1].trim()) || 0) + 1)
    }
    for (const [n, c] of prefix) if (c >= 2) names.add(n)
    const out = new Map()
    for (const x of people) {
      const r = String(place(x).role || '')
      const own = streamOfRole(r)
      const hit = own ? [...names].find((n) => n.toLowerCase() === own.toLowerCase()) || own : [...names].find((n) => new RegExp(`(^|[^\\w])${reEsc(n)}([^\\w]|$)`, 'i').test(r))
      if (hit) out.set(x.key, hit)
    }
    return out
  }
  function assign(people, place) {
    const roles = roleStreams(people, place)
    const out = new Map()
    for (const x of people) {
      const p = place(x), own = saved(x, place), role = p.role || ''
      const stream = own || roles.get(x.key) || ''
      const leads = isLead(role) && stream && (own ? true : !!streamOfRole(role))
      const band = p.governance != null ? !!p.governance : !stream && (p.influence || 0) >= 3 && (p.interest || 0) >= 3
      out.set(x.key, { stream, fromRoles: !own && !!stream, band, lead: leads && !band })
    }
    return out
  }
  return { assign, roleStreams, streamOfRole }
})()
// end of the org chart layout block
const ocPlace = (x) => placeOf(x) || {}
/** The chart's layout of the people shown now (`OrgLayout.assign`, set by `peopleOrg`). */
let ocLay = new Map()
const ocGov = (x) => { const l = ocLay.get(x.key); if (l) return l.band; const p = ocPlace(x); return p.governance != null ? !!p.governance : (p.influence || 0) >= 3 && (p.interest || 0) >= 3 }
const ocStream = (x) => (ocLay.get(x.key) || {}).stream || ocPlace(x).group || x.group || ''
const ocBoss = (x) => ocPlace(x).works_for || x.works_for_hint || null
const ocIdle = (x) => !!x.quiet || (!x.calls && !x.owes && !(x.facts || []).length)
function ocCard(x, d) {
  const p = ocPlace(x), thread = !S.pj.client
  const boss = ocPlace(x).works_for ? d.people.find((y) => y.key === ocPlace(x).works_for) : null
  const rep = x.reports_to ? d.people.find((y) => y.key === x.reports_to) : null
  const roleWhen = p.role && p.role_at ? `role since ${fmtDay(p.role_at)}${p.role_by === 'you' ? ' (yours)' : p.role_from ? ` (from ${p.role_from})` : ''}` : ''
  const tip = [x.title, roleWhen, rep ? `reports to ${rep.name}` : '', boss ? `works for ${boss.name} here` : x.works_for_hint ? `works for ${(d.people.find((y) => y.key === x.works_for_hint) || {}).name || '?'} here (from the work)` : ''].filter(Boolean).join(' · ')
  const meta = [x.owes ? `${x.owes} open` : '', x.late ? `<span class="late">${x.late} late</span>` : '', x.last ? `seen ${esc(relDay(x.last))}` : 'not in a call yet'].filter(Boolean).join(' · ')
  return `<button class="oc-card${S.pj.person === x.key ? ' on' : ''}" data-pp="pick" data-key="${esc(x.key)}" data-pk="${esc(x.key)}"${thread ? ` draggable="true" data-pp-drag="${esc(x.key)}" data-oc-person="${esc(x.key)}"` : ''} title="${esc(tip)}">${ppAv(x.name, p.stance)}<span class="oc-t"><b>${esc(x.name)}</b><small>${esc(p.role || x.title || '')}</small><span class="oc-m">${meta}</span></span></button>`
}
/** A workstream's people: its lead (whom most of the others work for here; else the most senior),
 *  then by seniority, then by calls; the inactive folded. */
function ocColumn(ys, key, label, d) {
  // Its lead: the one whose role leads this stream, else whom the others work for here.
  const named = (x) => (ocLay.get(x.key) || {}).lead ? 100 : 0
  const n = (x) => ys.filter((y) => ocBoss(y) === x.key).length + named(x)
  const order = ys.slice().sort((a, b) => n(b) - n(a) || (b.seniority || 0) - (a.seniority || 0) || (b.calls || 0) - (a.calls || 0) || a.name.localeCompare(b.name))
  const lead = order.length && n(order[0]) > 0 && (order.length > 1 || named(order[0])) ? order[0] : null
  const rest = order.filter((x) => x !== lead)
  const on = rest.filter((x) => !ocIdle(x)), idle = rest.filter(ocIdle)
  const open = S.pj.ocOpen && S.pj.ocOpen[key]
  const fold = idle.length ? (open ? idle.map((x) => ocCard(x, d)).join('') + `<button class="link-btn oc-more" data-pp="oc-fold" data-col="${esc(key)}">Fold</button>` : `<button class="link-btn oc-more" data-pp="oc-fold" data-col="${esc(key)}" title="${esc(idle.map((x) => x.name).join(', '))}">+${idle.length} not active</button>`) : ''
  return `<div class="oc-col" data-oc-drop="${esc(key)}"><span class="cap">${esc(label)}</span>${lead ? `<div class="oc-lead">${ocCard(lead, d)}</div>` : ''}${on.map((x) => ocCard(x, d)).join('')}${fold}${!ys.length ? '<span class="set-sub">Drop people here</span>' : ''}</div>`
}
function peopleOrg(d) {
  const people = d.people.filter((x) => x.in_thread)
  ocLay = OrgLayout.assign(people, ocPlace)
  const derived = people.filter((x) => (ocLay.get(x.key) || {}).fromRoles)
  const byOrg = new Map()
  for (const x of people) { const k = x.side === 'yours' ? '#yours' : x.org ? x.org.id : `#${x.domain || 'unknown'}`; if (!byOrg.has(k)) byOrg.set(k, []); byOrg.get(k).push(x) }
  const orgName = (k) => k === '#yours' ? 'Your company' : k.startsWith('#') ? (k.slice(1) === 'unknown' ? 'Not known' : k.slice(1)) : ((d.orgs.find((o) => o.id === k) || {}).name || k)
  const org = (k) => {
    const o = d.orgs.find((x) => x.id === k)
    const xs = byOrg.get(k) || []
    const gov = xs.filter(ocGov), work = xs.filter((x) => !ocGov(x))
    const streams = new Map()
    for (const x of work) { const g = ocStream(x); if (!streams.has(g)) streams.set(g, []); streams.get(g).push(x) }
    const cols = [...streams.entries()].sort((a, b) => (a[0] === '') - (b[0] === '') || a[0].localeCompare(b[0]))
    const rel = o ? [o.parent ? `part of ${esc((d.orgs.find((x) => x.id === o.parent) || { name: o.parent }).name)}` : '', ...(o.relations || []).map((r) => `${esc(r.kind.replace('_', ' '))} ${esc((d.orgs.find((x) => x.id === r.to) || { name: r.to }).name)}`)].filter(Boolean).join(' · ') : ''
    const band = gov.length || xs.length > 3 ? `<div class="oc-gov" data-oc-drop="${esc(k)}|gov"><span class="cap">Governance</span><div class="oc-row">${gov.slice().sort((a, b) => (b.seniority || 0) - (a.seniority || 0)).map((x) => ocCard(x, d)).join('') || '<span class="set-sub">Sponsor, partner, programme lead, PM — drop them here</span>'}</div></div>` : ''
    const fromRoles = (g) => g && xs.some((x) => ocStream(x) === g && (ocLay.get(x.key) || {}).fromRoles)
    const body = cols.map(([g, ys]) => ocColumn(ys, `${k}|${g}`, g ? `${g}${fromRoles(g) ? ' · from roles' : ''}` : (cols.length > 1 ? 'No workstream' : 'Team'), d)).join('')
    const newCol = !S.pj.client && xs.length ? (S.pj.ocNewCol === k ? `<div class="oc-col oc-new"><input class="wp-in" id="oc-new-col" data-org="${esc(k)}" placeholder="Workstream name" aria-label="New workstream" autofocus><span class="set-sub">then drop people on it</span></div>` : `<button class="oc-add" data-pp="oc-new-col" data-org="${esc(k)}" title="A new workstream column">+ Workstream</button>`) : ''
    const extra = (S.pj.ocCols || []).filter((c) => c.org === k && !streams.has(c.name)).map((c) => ocColumn([], `${k}|${c.name}`, c.name, d)).join('')
    return `<section class="pp-org oc-org${k === '#yours' ? ' yours' : ''}" data-org="${esc(o ? o.id : k)}"><div class="pp-org-h">${o ? `<button class="pp-org-n" data-pp="pick" data-key="org:${esc(o.id)}">${ppAv(o.name, (placeOf(o) || {}).stance, true)}<b>${esc(o.name)}</b></button>` : `<b>${esc(orgName(k))}</b>`}<span class="set-sub">${xs.length} ${xs.length === 1 ? 'person' : 'people'}${rel ? ` · ${rel}` : ''}</span></div>
      ${band}<div class="oc-cols">${body}${extra}${newCol}</div></section>`
  }
  // Organisations a shown one is related to (parent, partner, vendor…) are shown too, so the line has an end.
  const shown = new Set(byOrg.keys())
  for (const o of d.orgs || []) {
    if (!shown.has(o.id)) continue
    for (const t of [o.parent, ...(o.relations || []).map((r) => r.to)].filter(Boolean)) if (d.orgs.some((x) => x.id === t)) shown.add(t)
  }
  for (const o of d.orgs || []) if ((o.relations || []).some((r) => shown.has(r.to)) || (o.parent && shown.has(o.parent))) shown.add(o.id)
  // Yours on the left, the others mirrored on the right.
  const keys = [...shown].sort((a, b) => (b === '#yours') - (a === '#yours') || (a === '#unknown') - (b === '#unknown') || orgName(a).localeCompare(orgName(b)))
  // Columns read from the roles: kept as each person's workstream in one go (as dragging them would).
  const keep = derived.length && !S.pj.client ? `<button class="btn" data-pp="oc-keep" title="Save ${derived.length} people's workstream as their roles give it">Keep these columns</button>` : ''
  const bar = `<div class="oc-bar"><span class="set-sub">Who does what here. Lines: works for (solid; dashed when the work suggests it) · works with across organisations (faint)</span><span class="grow"></span>${keep}<label class="oc-tog"><input type="checkbox" data-pp-reports${S.pj.ocReports ? ' checked' : ''}> Reporting lines</label></div>`
  return keys.length ? `${bar}<div class="pp-orgs oc">${keys.map(org).join('')}</div>` : '<p class="set-sub">Nobody placed yet: add people from List.</p>'
}
/** The chart's people lines, over it: works for (solid; from the work, dashed), works with across
 *  organisations (shared calls, faint), reports to (dotted, when asked for). */
function drawPeopleLinks(wrap, d) {
  const old = wrap.querySelector(':scope > svg.oc-links')
  const box = wrap.getBoundingClientRect()
  const at = (k) => { const e = wrap.querySelector(`[data-pk="${CSS.escape(k)}"]`); if (!e || !e.offsetParent) return null; const r = e.getBoundingClientRect(); return { x: r.left - box.left + r.width / 2, y: r.top - box.top + r.height / 2, t: r.top - box.top, b: r.bottom - box.top, org: e.closest('[data-org]') } }
  const lines = []
  for (const x of d.people.filter((y) => y.in_thread)) {
    const set = ocPlace(x).works_for
    const boss = set || x.works_for_hint
    if (boss) lines.push({ a: x.key, b: boss, cls: set ? 'wf' : 'wf hint' })
    if (S.pj.ocReports && x.reports_to) lines.push({ a: x.key, b: x.reports_to, cls: 'rep' })
    for (const w of x.with || []) if (x.key < w.key) lines.push({ a: x.key, b: w.key, cls: 'ww', cross: true })
  }
  const paths = lines.map((l) => {
    const a = at(l.a), b = at(l.b)
    if (!a || !b || (l.cross && a.org === b.org)) return ''
    // From the bottom of the upper card to the top of the lower; side by side, centre to centre.
    const [p, q] = a.y <= b.y ? [{ x: a.x, y: a.b }, { x: b.x, y: b.t }] : [{ x: a.x, y: a.t }, { x: b.x, y: b.b }]
    const my = (p.y + q.y) / 2
    return `<path class="${l.cls}" d="M${p.x.toFixed(1)},${p.y.toFixed(1)} C${p.x.toFixed(1)},${my.toFixed(1)} ${q.x.toFixed(1)},${my.toFixed(1)} ${q.x.toFixed(1)},${q.y.toFixed(1)}"/>`
  }).join('')
  const key = `${paths.length}|${wrap.scrollWidth}x${wrap.scrollHeight}|${paths.slice(0, 200)}`
  if (old && old.dataset.key === key) return
  if (old) old.remove()
  if (paths) wrap.insertAdjacentHTML('afterbegin', `<svg class="oc-links" data-key="${esc(key)}" width="${wrap.scrollWidth}" height="${wrap.scrollHeight}" aria-hidden="true">${paths}</svg>`)
}
/** Lines between the org chart's organisations (drawn after the page is laid out, over it): each
 *  relationship labelled with its kind and pointing at the other side; "part of" dashed. */
function drawOrgLinks() {
  const wrap = document.querySelector('.pp-orgs')
  if (!wrap) return
  const d = peopleData()
  if (d && wrap.classList.contains('oc')) drawPeopleLinks(wrap, d)
  const edges = []
  for (const o of (d && d.orgs) || []) {
    if (o.parent) edges.push({ a: o.id, b: o.parent, kind: 'part of', dash: true })
    for (const r of o.relations || []) edges.push({ a: o.id, b: r.to, kind: r.kind.replace(/_/g, ' ') })
  }
  const key = JSON.stringify(edges) + `|${wrap.clientWidth}x${wrap.scrollHeight}`
  const old = wrap.querySelector(':scope > svg.pp-links')
  if (old && old.dataset.key === key) return
  if (old) old.remove()
  // Room for the lines between cards, before anything is measured.
  const linked = edges.some((e) => wrap.querySelector(`[data-org="${CSS.escape(e.a)}"]`) && wrap.querySelector(`[data-org="${CSS.escape(e.b)}"]`))
  wrap.classList.toggle('linked', linked)
  if (!linked) return
  const box = wrap.getBoundingClientRect()
  // Each card's side midpoints; a line joins the closest pair, so it runs card edge to card edge.
  const sides = (id) => {
    const s = wrap.querySelector(`[data-org="${CSS.escape(id)}"]`)
    if (!s) return null
    const r = s.getBoundingClientRect()
    const l = r.left - box.left, t = r.top - box.top
    return [{ x: l + r.width / 2, y: t }, { x: l + r.width / 2, y: t + r.height }, { x: l, y: t + Math.min(r.height / 2, 30) }, { x: l + r.width, y: t + Math.min(r.height / 2, 30) }]
  }
  const drawn = edges.map((e) => {
    const a = sides(e.a), b = sides(e.b)
    if (!a || !b) return null
    let best = null
    for (const p of a) for (const q of b) { const dd = Math.hypot(p.x - q.x, p.y - q.y); if (!best || dd < best.d) best = { p, q, d: dd } }
    return { ...e, p: best.p, q: best.q }
  }).filter(Boolean)
  if (!drawn.length) return
  const lines = drawn.map((e, i) => {
    const { p, q } = e
    // A curve that bows away from the cards' straight path; its label at the middle.
    const mx = (p.x + q.x) / 2, my = (p.y + q.y) / 2, dx = q.x - p.x, dy = q.y - p.y, len = Math.hypot(dx, dy) || 1
    const bow = Math.min(60, len / 4) * (i % 2 ? 1 : -1)
    const cx = mx - (dy / len) * bow, cy = my + (dx / len) * bow
    const lx = (p.x + 2 * cx + q.x) / 4, ly = (p.y + 2 * cy + q.y) / 4
    return `<path d="M${p.x.toFixed(1)},${p.y.toFixed(1)} Q${cx.toFixed(1)},${cy.toFixed(1)} ${q.x.toFixed(1)},${q.y.toFixed(1)}" class="${e.dash ? 'dash' : ''}" marker-end="url(#pp-arrow)"/>` +
      `<g class="lbl" transform="translate(${lx.toFixed(1)},${ly.toFixed(1)})"><rect x="${-(e.kind.length * 3.4 + 7)}" y="-9" width="${e.kind.length * 6.8 + 14}" height="18" rx="9"/><text y="4">${esc(e.kind)}</text></g>`
  }).join('')
  wrap.insertAdjacentHTML('beforeend', `<svg class="pp-links" data-key="${esc(key)}" width="${wrap.scrollWidth}" height="${wrap.scrollHeight}" aria-hidden="true"><defs><marker id="pp-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z"/></marker></defs>${lines}</svg>`)
}
window.addEventListener('resize', () => { if (S.view === 'projects') drawOrgLinks() })

/** The selected person or organisation, in the right panel. */
function personPanel() {
  const d = peopleData(), k = S.pj.person
  if (!d || !k) return '<div class="wp-empty">Pick someone on the People tab.</div>'
  if (k.startsWith('org:')) return orgPanel(d, k.slice(4))
  const x = d.people.find((y) => y.key === k)
  if (!x) return '<div class="wp-empty">Not in this view any more.</div>'
  const thread = S.pj.client || S.view === 'brain' ? null : S.pj.sel
  const pl = thread ? (x.places || []).find((p) => p.thread === thread) || {} : placeOf(x) || {}
  const SRC = { you: 'you', m365: 'Microsoft 365', echo: 'Echo' }
  const src = (f) => x.sources && x.sources[f] ? `<span class="set-sub"> · ${esc(SRC[x.sources[f]] || x.sources[f])}</span>` : ''
  const field = (f, label, input) => [label, S.pj.ppEdit === f ? input : `<button class="wp-val${x[f] ? '' : ' empty'}" data-pp="edit" data-f="${f}">${x[f] ? esc(x[f]) : '<u>Set</u>'}</button>${src(f)}`]
  const others = d.people.filter((y) => y.key !== x.key && y.in_thread)
  const orgs = (S.pj.data && (S.pj.data.orgs || S.pj.data.clients)) || []
  // A domain no organisation has: name it (a new organisation, this thread's counterpart).
  const nameOrg = x.side === 'other' && x.domain && !x.org
    ? (S.pj.nameOrg === x.domain ? `<span class="pp-rel-add"><input class="wp-in" id="pp-org-name" placeholder="Who is ${esc(x.domain)}?" aria-label="Organisation name" autofocus><button class="btn primary" data-pp="org-name" data-domain="${esc(x.domain)}">Add</button></span>`
      : `<button class="link-btn" data-pp="org-name-ask" data-domain="${esc(x.domain)}">Name this organisation (${esc(x.domain)})</button>`) : ''
  const rows = [
    field('title', 'Title', `<input class="wp-in" data-pp-set="title" value="${esc(x.title || '')}" aria-label="Title" autofocus>`),
    ['Organisation', `<select class="wp-in" data-pp-set="org" aria-label="Organisation"><option value="">${esc(x.org && !(x.sources || {}).org ? `${x.org.name} (by mail domain)` : x.side === 'yours' ? 'Your company' : 'Not known')}</option>${orgs.map((c) => `<option value="${esc(c.id)}"${(x.sources || {}).org && x.org && x.org.id === c.id ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}</select>${nameOrg}`],
    field('group', 'Group', `<input class="wp-in" data-pp-set="group" value="${esc(x.group || '')}" list="pp-groups" aria-label="Group" autofocus><datalist id="pp-groups">${[...new Set(d.people.map((y) => y.group).filter(Boolean).concat(...d.orgs.map((o) => o.groups || [])))].map((g) => `<option value="${esc(g)}">`).join('')}</datalist>`),
    ['Reports to', `<select class="wp-in" data-pp-set="reports_to" aria-label="Reports to"><option value="">nobody set</option>${others.map((y) => `<option value="${esc(y.key)}"${x.reports_to === y.key ? ' selected' : ''}>${esc(y.name)}</option>`).join('')}</select>`],
    ['Matters', `<select class="wp-in" data-pp-set="matters" aria-label="How much they matter to you"><option value="">not set</option>${MATTERS.map((m) => `<option${x.matters === m ? ' selected' : ''}>${m}</option>`).join('')}</select>`],
    field('phone', 'Phone', `<input class="wp-in" data-pp-set="phone" value="${esc(x.phone || '')}" aria-label="Phone" autofocus>`),
    ...(x.department || x.company || x.office ? [
      field('department', 'Department', `<input class="wp-in" data-pp-set="department" value="${esc(x.department || '')}" aria-label="Department" autofocus>`),
      field('company', 'Company', `<input class="wp-in" data-pp-set="company" value="${esc(x.company || '')}" aria-label="Company" autofocus>`),
      field('office', 'Office', `<input class="wp-in" data-pp-set="office" value="${esc(x.office || '')}" aria-label="Office" autofocus>`),
    ] : []),
  ]
  const asks = (x.questions || []).map((q) => `<div class="pp-ask"><span class="grow"><b>${esc(SRC[q.source] || q.source)}</b> says ${esc(q.field)} is “${esc(q.theirs)}” — you set “${esc(q.yours || '')}”.</span><button class="btn" data-pp="answer" data-f="${esc(q.field)}" data-use="1">Use theirs</button><button class="btn ghost" data-pp="answer" data-f="${esc(q.field)}" data-use="">Keep mine</button></div>`).join('')
  const lvl = (f) => `<span class="pp-lvl" role="group" aria-label="${f}">${[1, 2, 3].map((n) => `<button data-pp="level" data-f="${f}" data-n="${n}" aria-pressed="${(pl[f] || 0) >= n}" title="${['low', 'mid', 'high'][n - 1]}"></button>`).join('')}<span class="set-sub">${pl[f] ? ['low', 'mid', 'high'][pl[f] - 1] : 'not set'}</span></span>`
  const inThread = thread ? `<div class="wp-h">In ${esc((projById(thread) || {}).name || 'this thread')}</div><div class="wp-facts">
      <span>Role</span><div>${S.pj.ppEdit === 'role' ? `<input class="wp-in" data-pp-place="role" value="${esc(pl.role || '')}" aria-label="Role" autofocus>` : `<button class="wp-val${pl.role ? '' : ' empty'}" data-pp="edit" data-f="role">${pl.role ? esc(pl.role) : '<u>Set</u>'}</button>${pl.role && pl.role_at ? `<div class="set-sub">${esc(fmtDay(pl.role_at))} · ${pl.role_by === 'you' ? 'yours (Echo leaves it)' : `from ${esc(pl.role_from || 'a call')}`}</div>` : ''}`}</div>
      <span>Stance</span><div><select class="wp-in" data-pp-place="stance" aria-label="Stance"><option value="">not set</option>${STANCES.map((s) => `<option${pl.stance === s ? ' selected' : ''}>${s}</option>`).join('')}</select></div>
      <span>Influence</span><div>${lvl('influence')}</div><span>Interest</span><div>${lvl('interest')}</div>
      <span>Workstream</span><div>${S.pj.ppEdit === 'group' ? `<input class="wp-in" data-pp-place="group" value="${esc(pl.group || '')}" list="pp-streams" aria-label="Workstream" autofocus><datalist id="pp-streams">${[...new Set(d.people.map((y) => ocStream(y)).filter(Boolean))].map((g) => `<option value="${esc(g)}">`).join('')}</datalist>` : `<button class="wp-val${pl.group ? '' : ' empty'}" data-pp="edit" data-f="group">${pl.group ? esc(pl.group) : x.group ? `${esc(x.group)} <span class="set-sub">(their group)</span>` : '<u>Set</u>'}</button>`}</div>
      <span>Governance</span><div><select class="wp-in" data-pp-place="governance" aria-label="Governance"><option value=""${pl.governance == null ? ' selected' : ''}>${ocGov(x) ? 'yes — high influence and interest' : 'no — by influence and interest'}</option><option value="yes"${pl.governance === true ? ' selected' : ''}>yes</option><option value="no"${pl.governance === false ? ' selected' : ''}>no</option></select></div>
      <span>Works for here</span><div><select class="wp-in" data-pp-place="works_for" aria-label="Works for in this thread"><option value="">${x.works_for_hint && !pl.works_for ? `${esc((d.people.find((y) => y.key === x.works_for_hint) || {}).name || '?')}? (from the work)` : 'nobody set'}</option>${d.people.filter((y) => y.key !== x.key && y.in_thread).map((y) => `<option value="${esc(y.key)}"${pl.works_for === y.key ? ' selected' : ''}>${esc(y.name)}</option>`).join('')}</select></div>
    </div>${x.in_thread ? '' : '<p class="set-sub">Not in this thread yet: setting any of these adds them.</p>'}` : `<div class="wp-h">In threads</div><div class="pp-in">${(x.places || []).map((p) => `<button class="link-btn" data-pj="goto" data-id="${esc(p.thread)}">${esc((projById(p.thread) || {}).name || p.thread)}</button>${p.role ? ` · ${esc(p.role)}` : ''}${p.stance ? ` · ${esc(p.stance)}` : ''}`).join('<br>') || '<span class="set-sub">none</span>'}</div>`
  const stat = (n, l) => `<div><b>${n}</b><span class="set-sub">${l}</span></div>`
  const hist = (x.history || []).slice().reverse().slice(0, 8).map((h) => `<div class="pp-h"><span class="set-sub">${esc(relDay(String(h.at || '').slice(0, 10)))}</span> ${h.field === 'about' ? `who they are: ${h.to || h.source === 'echo' ? 'rewritten' : 'cleared'}` : `${esc(h.field)}: ${h.from ? `<s>${esc(h.from)}</s> → ` : ''}${esc(h.to || 'cleared')}`} <span class="set-sub">· ${esc(SRC[h.source] || h.source)}</span></div>`).join('')
  // Who they are (Echo's summary from what was said, or the user's own words) and what was said.
  const about = S.pj.aboutEdit != null
    ? `<textarea class="wp-in pp-about-in" id="pp-about" rows="5" maxlength="2000" aria-label="Who they are">${esc(S.pj.aboutEdit)}</textarea><div class="wp-desc-bar"><button class="btn" data-pp="about-save">Save</button><button class="btn ghost" data-pp="about-cancel">Cancel</button><span class="set-sub">⌘S saves · Esc cancels${x.about_by === 'echo' ? ' · yours from then on: Echo won\'t rewrite it' : ''}</span></div>`
    : x.about ? `<button class="pp-about" data-pp="about-edit" title="Edit">${esc(x.about)}</button><div class="set-sub">${x.about_by === 'you' ? 'In your words' : 'Echo\'s summary of what was said — click to write your own'}</div>`
    : '<button class="link-btn" data-pp="about-edit">Describe them…</button><span class="set-sub"> Echo writes this from their calls, chats and mail once it knows enough.</span>'
  const factSrc = (f) => f.kind === 'mail'
    ? (/^https:\/\//i.test(f.url || '') ? `<a class="wp-src" href="${esc(f.url)}" data-ext title="${esc(f.title || 'The mail')} — open in Outlook">✉</a>` : '<span class="wp-src">✉</span>')
    : f.call ? `<button class="wp-src" data-call="${esc(f.call)}" title="${esc(f.title || '')}">▮ ${esc(f.ts ? String(f.ts).replace(/^00:/, '').slice(0, 5) : '')}</button>` : `<span class="wp-src" title="${esc(f.title || '')}">💬</span>`
  const allFacts = x.facts || []
  const facts = allFacts.slice(0, S.pj.ppAllFacts ? 300 : 8).map((f) => `<div class="pp-fact"><div><span class="set-sub">${esc(relDay(f.date))}</span> ${esc(f.text)}${!thread && f.thread ? ` <span class="set-sub">· ${esc((projById(f.thread) || {}).name || f.thread)}</span>` : ''}</div><div class="wp-q" title="${esc(f.title || '')}">“${esc(f.quote)}” ${factSrc(f)}</div></div>`).join('')
  const withThem = (x.with || []).map((y) => { const o = personOf(y.key); return o ? `<button class="link-btn" data-pp="pick" data-key="${esc(y.key)}">${esc(o.name)}</button><span class="set-sub"> · ${y.calls} calls</span>` : '' }).filter(Boolean).join('<br>')
  return `<div class="wp pp-panel">
    <div class="pp-p-top">${ppAv(x.name, pl.stance)}<div style="min-width:0">${S.pj.ppEdit === 'name' ? `<input class="wp-in" data-pp-set="name" value="${esc(x.name)}" aria-label="Name" autofocus>` : `<button class="wp-name pp-name" data-pp="edit" data-f="name">${esc(x.name)}</button>`}<div class="set-sub">${esc([x.title, sideLabel(x)].filter(Boolean).join(' · '))}</div></div></div>
    ${(x.maybe_same || []).map((m) => `<div class="pp-ask"><span class="grow">Same person as <b>${esc(m.name)}</b>${m.email ? ` <span class="set-sub">${esc(m.email)}</span>` : ''}?</span><button class="btn" data-pp="merge" data-into="${esc(m.key)}">Merge</button></div>`).join('')}
    ${(x.addresses || []).length ? `<div class="pp-addrs">${x.addresses.map((a, i) => `<div class="pp-addr"><a class="icon-btn" href="https://teams.microsoft.com/l/chat/0/0?users=${encodeURIComponent(a.email)}" data-ext title="Chat in Teams" aria-label="Chat in Teams with ${esc(a.email)}">${ICON('<path d="M5 18.5V7a2.5 2.5 0 0 1 2.5-2.5h9A2.5 2.5 0 0 1 19 7v6a2.5 2.5 0 0 1-2.5 2.5H9z"/>')}</a><button class="icon-btn" data-copy="${esc(a.email)}" title="Copy ${esc(a.email)}" aria-label="Copy ${esc(a.email)}">${ICON('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>')}</button><span class="pp-addr-t">${esc(a.email)}<span class="set-sub"> · ${esc(a.org || a.domain || '')}</span></span>${i ? `<button class="link-btn" data-pp="split" data-email="${esc(a.email)}" title="This address is someone else">Not the same</button>` : ''}</div>`).join('')}</div>` : ''}
    <div class="pp-same">${S.pj.ppSame ? `<select class="wp-in" id="pp-same" aria-label="Same person as"><option value="">Same person as…</option>${d.people.filter((y) => y.key !== x.key).map((y) => `<option value="${esc(y.key)}">${esc(y.name)}${y.email ? ` · ${esc(y.email)}` : ''}</option>`).join('')}</select><button class="btn" data-pp="merge-pick">Merge</button><button class="btn ghost" data-pp="same-cancel">Cancel</button>` : '<button class="link-btn" data-pp="same">Same person as…</button>'}</div>
    <div class="wp-h">Who they are</div><div class="pp-about-box">${about}</div>
    ${inThread}
    ${facts ? `<div class="wp-h">What was said about them</div>${facts}${allFacts.length > 8 ? `<button class="link-btn" data-pp="all-facts">${S.pj.ppAllFacts ? 'Fewer' : `All ${allFacts.length}`}</button>` : ''}` : ''}
    ${withThem ? `<div class="wp-h">Often in calls with</div><div class="pp-in">${withThem}</div>` : ''}
    <div class="wp-h">About them</div>${asks}<div class="wp-facts">${rows.map(([l, v]) => `<span>${esc(l)}</span><div>${v}</div>`).join('')}</div>
    <div class="wp-h">What Echo saw</div>
    <div class="pp-stats">${stat(`${x.calls}<small> of ${x.of_calls}</small>`, 'calls')}${stat(`${x.share || 0}%`, 'of the talking')}${stat(x.owes, `tasks owed${x.late ? ` · <span class="late">${x.late} late</span>` : ''}`)}${stat(x.waiting, 'waiting on them')}</div>
    <div class="set-sub">${x.first ? `First seen ${esc(relDay(x.first))}, last ${esc(relDay(x.last))}` : 'Not in a call yet'}${x.chat ? ` · ${x.chat} chat message${x.chat === 1 ? '' : 's'}` : ''}</div>
    ${hist ? `<div class="wp-h">Changes</div>${hist}` : ''}
  </div>`
}
function orgPanel(d, id) {
  const o = d.orgs.find((x) => x.id === id)
  if (!o) return '<div class="wp-empty">Not in this view any more.</div>'
  const thread = S.pj.client || S.view === 'brain' ? null : S.pj.sel
  const pl = thread ? (o.places || []).find((p) => p.thread === thread) || {} : placeOf(o) || {}
  const all = ((S.pj.data && (S.pj.data.orgs || S.pj.data.clients)) || []).filter((c) => c.id !== o.id)
  const linked = thread && ((projById(thread) || {}).orgs || []).some((x) => x.client === o.id)
  const KINDS = REL_KINDS
  const lvl = (f) => `<span class="pp-lvl" role="group" aria-label="${f}">${[1, 2, 3].map((n) => `<button data-pp="org-level" data-id="${esc(o.id)}" data-f="${f}" data-n="${n}" aria-pressed="${(pl[f] || 0) >= n}" title="${['low', 'mid', 'high'][n - 1]}"></button>`).join('')}<span class="set-sub">${pl[f] ? ['low', 'mid', 'high'][pl[f] - 1] : 'not set'}</span></span>`
  return `<div class="wp pp-panel">
    <div class="pp-p-top">${ppAv(o.name, pl.stance, true)}<div style="min-width:0">${o.account === false ? `<span class="wp-name pp-name">${esc(o.name)}</span>` : `<button class="wp-name pp-name" data-pj="client" data-id="${esc(o.id)}">${esc(o.name)}</button>`}<div class="set-sub">${o.internal ? 'your company' : o.account === false ? 'organisation' : 'client'} · ${o.people} ${o.people === 1 ? 'person' : 'people'}${o.domains && o.domains.length ? ` · ${esc(o.domains.join(', '))}` : ''}</div></div></div>
    ${thread && !o.internal && (projById(thread) || {}).client !== o.id ? `<div class="set-sub">${linked ? `A counterpart of this thread · <button class="link-btn" data-pj="org-unlink" data-org="${esc(o.id)}">Unlink</button>` : `<button class="link-btn" data-pj="org-link" data-org="${esc(o.id)}">Make it this thread's counterpart</button>`}</div>` : ''}
    ${thread ? `<div class="wp-h">In ${esc((projById(thread) || {}).name || 'this thread')}</div><div class="wp-facts">
      <span>Stance</span><div><select class="wp-in" data-pp-org="${esc(o.id)}" data-f="stance" aria-label="Stance"><option value="">not set</option>${STANCES.map((s) => `<option${pl.stance === s ? ' selected' : ''}>${s}</option>`).join('')}</select></div>
      <span>Influence</span><div>${lvl('influence')}</div><span>Interest</span><div>${lvl('interest')}</div></div>` : ''}
    <div class="wp-h">Relationships</div><div class="wp-facts">
      <span>Part of</span><div><select class="wp-in" data-pp-rel="parent" data-id="${esc(o.id)}" aria-label="Part of"><option value="">nothing</option>${all.map((c) => `<option value="${esc(c.id)}"${o.parent === c.id ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}</select></div>
      ${(o.relations || []).map((r) => `<span>${esc((KINDS.find(([k]) => k === r.kind) || [0, r.kind])[1])}</span><div>${esc((all.find((c) => c.id === r.to) || { name: r.to }).name)} <button class="link-btn" data-pp="rel-del" data-id="${esc(o.id)}" data-to="${esc(r.to)}">remove</button></div>`).join('')}
      <span>Add</span><div class="pp-rel-add"><select class="wp-in" id="pp-rel-kind" aria-label="Relationship">${KINDS.map(([k, l]) => `<option value="${k}">${esc(l)}</option>`).join('')}</select><select class="wp-in" id="pp-rel-to" aria-label="Organisation"><option value="">organisation…</option>${all.map((c) => `<option value="${esc(c.id)}">${esc(c.name)}</option>`).join('')}</select><button class="btn" data-pp="rel-add" data-id="${esc(o.id)}">Add</button></div>
    </div>
    <div class="wp-h">Groups</div><div class="pp-in">${(o.groups || []).map((g) => `<span class="tok">${esc(g)}</span>`).join('') || '<span class="set-sub">none yet — set a person\'s group and it appears here</span>'}</div>
  </div>`
}
PANEL_VIEWS.push({ id: 'person', label: 'Person', icon: ICON('<circle cx="12" cy="8.5" r="3.5"/><path d="M5 20a7 7 0 0 1 14 0"/>'), area: 'right', when: () => !!S.pj.person && ((S.view === 'brain' && S.br.tab === 'people') || (S.view === 'projects' && ((S.pj.client && S.pj.ctab === 'people') || (!S.pj.client && S.pj.sel && S.pj.tab === 'people')))), render: personPanel })

function peopleSend(op, extra) {
  send({ cmd: 'project', op, id: S.pj.sel, ...extra })
  setTimeout(() => (S.view === 'brain' ? loadBrainPeople() : loadPeople()).then(render), 250)
}
function aboutSave() {
  const i = $('#pp-about')
  if (!i) return
  const v = i.value.trim()
  S.pj.aboutEdit = null
  personSet('about', v)
}
/** Change… on a suggestion: its row becomes your version, focused. */
function sugEdit(sid) {
  S.pj.ppSugEdit = sid
  render()
  const i = $('#pp-sug-v')
  if (i) { i.focus(); i.select && i.select() }
}
function sugSave(thread, sid) {
  const i = $('#pp-sug-v'), value = i ? i.value.trim() : ''
  if (!value) return window.echo.notice('Say what it should be, or press No.')
  S.pj.ppSugEdit = null
  send({ cmd: 'project', op: 'people_decide', id: thread, sid, yes: true, value })
  setTimeout(() => { loadPeople().then(render); loadAttention(true).then(render) }, 250)
}
function personOf(key) { const d = peopleData(); return d && d.people.find((y) => y.key === key) }
function personSet(field, value) {
  const x = personOf(S.pj.person)
  if (!x) return
  // The directory's group list follows a person's group (the organisation keeps the names).
  if (field === 'group' && value && x.org) {
    const o = ((S.pj.people || {}).orgs || []).find((y) => y.id === x.org.id)
    if (o && !(o.groups || []).includes(value)) send({ cmd: 'project', op: 'client_set', id: o.id, groups: [...(o.groups || []), value] })
  }
  S.pj.ppEdit = null
  peopleSend('person_set', { key: x.key, name: x.name, email: x.email, field, value })
}
function personPlace(key, set) {
  const x = personOf(key)
  if (x) return peopleSend('people_place', { name: x.name, email: x.email, set })
  if (key.startsWith('org:')) peopleSend('org_place', { client: key.slice(4), set })
}
function showPerson(key) {
  S.pj.person = key
  S.pj.ppEdit = null
  const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (eff.narrow) S.overlay = true
  Layout.show('person', known)
  layoutChanged()
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-pp]')
  if (!b) return
  e.preventDefault()
  const d = b.dataset
  switch (d.pp) {
    case 'view': try { localStorage.setItem('echo.people.view', d.v) } catch { /* storage off */ } return render()
    case 'pick': S.pj.aboutEdit = null; return showPerson(d.key)
    case 'about-edit': { const x = personOf(S.pj.person); S.pj.aboutEdit = (x && x.about) || ''; render(); const i = $('#pp-about'); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length) } return }
    case 'about-cancel': S.pj.aboutEdit = null; return render()
    case 'about-save': return aboutSave()
    case 'all-facts': S.pj.ppAllFacts = !S.pj.ppAllFacts; return render()
    case 'oc-keep': { const d2 = peopleData(); if (!d2) return; for (const x of d2.people.filter((y) => y.in_thread && (ocLay.get(y.key) || {}).fromRoles)) personPlace(x.key, { group: ocLay.get(x.key).stream }); return }
    case 'oc-fold': S.pj.ocOpen = { ...(S.pj.ocOpen || {}), [d.col]: !(S.pj.ocOpen || {})[d.col] }; return render()
    case 'oc-new-col': S.pj.ocNewCol = d.org; render(); { const i = $('#oc-new-col'); if (i) i.focus() } return
    case 'org-name-ask': S.pj.nameOrg = d.domain; render(); { const i = $('#pp-org-name'); if (i) i.focus() } return
    case 'org-name': { const name = ($('#pp-org-name') || {}).value || ''; if (!name.trim()) return; S.pj.nameOrg = null; send({ cmd: 'project', op: 'org_name', id: S.pj.client || S.view === 'brain' ? '' : S.pj.sel || '', name: name.trim(), domain: d.domain }); return setTimeout(async () => { await loadProjects(); await loadPeople(); render() }, 300) }
    case 'edit': S.pj.ppEdit = d.f; render(); { const i = document.querySelector('[data-pp-set][autofocus],[data-pp-place][autofocus]'); if (i) { i.focus(); i.select && i.select() } } return
    case 'level': { const x = personOf(S.pj.person), pl = x && (x.places || []).find((p) => p.thread === S.pj.sel); const cur = pl && pl[d.f]; return personPlace(S.pj.person, { [d.f]: cur === Number(d.n) ? '' : Number(d.n) }) }
    case 'org-level': { const o = ((S.pj.people || {}).orgs || []).find((y) => y.id === d.id), pl = o && (o.places || []).find((p) => p.thread === S.pj.sel); const cur = pl && pl[d.f]; return peopleSend('org_place', { client: d.id, set: { [d.f]: cur === Number(d.n) ? '' : Number(d.n) } }) }
    case 'merge': case 'merge-pick': {
      const x = personOf(S.pj.person), into = d.pp === 'merge' ? d.into : ($('#pp-same') || {}).value, y = into && personOf(into)
      if (!x || !y) return
      S.pj.ppSame = false
      // The one in the thread (else the one seen more) stays; the other's addresses join it.
      const keep = (y.in_thread && !x.in_thread) || (y.in_thread === x.in_thread && y.calls >= x.calls) ? y : x, other = keep === y ? x : y
      S.pj.person = keep.key
      return peopleSend('person_merge', { key: other.key, key_name: other.name, key_emails: [other.email, ...(other.also || [])].filter(Boolean), into: keep.key, into_name: keep.name, into_email: keep.email })
    }
    case 'same': S.pj.ppSame = true; return render()
    case 'same-cancel': S.pj.ppSame = false; return render()
    case 'split': { const x = personOf(S.pj.person); if (x) peopleSend('person_split', { key: x.key, name: x.name, email: d.email }); return }
    case 'decide': send({ cmd: 'project', op: 'people_decide', id: d.thread, sid: d.sid, yes: !!d.yes }); return setTimeout(() => loadPeople().then(render), 250)
    case 'sug-edit': return sugEdit(d.sid)
    case 'sug-cancel': S.pj.ppSugEdit = null; return render()
    case 'sug-save': return sugSave(d.thread, d.sid)
    case 'all-sug': S.pj.ppAllSug = !S.pj.ppAllSug; return render()
    case 'answer': { const x = personOf(S.pj.person); if (x) peopleSend('person_answer', { key: x.key, field: d.f, use: !!d.use }); return }
    case 'add': S.pj.ppAdding = true; render(); { const i = $('#pp-new'); if (i) i.focus() } return
    case 'add-all': { const dd = peopleData(); return peopleSend('people_add', { people: dd.people.filter((x) => !x.in_thread).map((x) => ({ name: x.name, email: x.email })) }) }
    case 'add-one': { const x = personOf(d.key); if (x) peopleSend('people_add', { people: [{ name: x.name, email: x.email }] }); return }
    case 'rel-add': { const kind = $('#pp-rel-kind').value, to = $('#pp-rel-to').value; if (!to) return; send({ cmd: 'project', op: 'client_set', id: d.id, relation: { to, kind } }); return setTimeout(() => loadProjects().then(loadPeople).then(render), 250) }
    case 'rel-del': send({ cmd: 'project', op: 'client_set', id: d.id, relation: { to: d.to, kind: '' } }); return setTimeout(() => loadProjects().then(loadPeople).then(render), 250)
  }
})
document.addEventListener('change', (e) => {
  const t = e.target
  if (!t.matches) return
  if (t.matches('[data-pp-set]') && t.tagName === 'SELECT') return personSet(t.dataset.ppSet, t.value)
  if (t.matches('[data-pp-place="governance"]')) return personPlace(S.pj.person, { governance: t.value === 'yes' ? true : t.value === 'no' ? false : null })
  if (t.matches('[data-pp-place]') && t.tagName === 'SELECT') return personPlace(S.pj.person, { [t.dataset.ppPlace]: t.value })
  if (t.matches('[data-pp-org]')) return peopleSend('org_place', { client: t.dataset.ppOrg, set: { [t.dataset.f]: t.value } })
  if (t.matches('[data-pp-rel="parent"]')) { send({ cmd: 'project', op: 'client_set', id: t.dataset.id, parent: t.value }); return setTimeout(() => loadProjects().then(loadPeople).then(render), 250) }
})
document.addEventListener('focusout', (e) => {
  const t = e.target
  if (!t.matches || t.tagName !== 'INPUT') return
  if (t.matches('[data-pp-set]')) return personSet(t.dataset.ppSet, t.value)
  if (t.matches('[data-pp-place]')) { S.pj.ppEdit = null; return personPlace(S.pj.person, { [t.dataset.ppPlace]: t.value }) }
  if (t.id === 'pp-new' && !t.value.trim()) { S.pj.ppAdding = false; render() }
})
document.addEventListener('keydown', (e) => {
  const t = e.target
  if (!t.matches) return
  if ((t.matches('[data-pp-set]') || t.matches('[data-pp-place]')) && t.tagName === 'INPUT') {
    if (e.key === 'Enter') { e.preventDefault(); t.blur() }
    if (e.key === 'Escape') { e.preventDefault(); S.pj.ppEdit = null; render() }
  }
  if (t.id === 'pp-new' && e.key === 'Enter') {
    e.preventDefault()
    const m = t.value.trim().match(/^(.*?)\s*<([^>]+@[^>]+)>$/) || t.value.trim().match(/^()([^\s]+@[^\s]+)$/)
    const name = m ? (m[1] || m[2].split('@')[0]) : t.value.trim(), email = m ? m[2] : undefined
    S.pj.ppAdding = false
    if (name) peopleSend('people_add', { people: [{ name, email }] })
    else render()
  }
  if (t.id === 'pp-new' && e.key === 'Escape') { S.pj.ppAdding = false; render() }
  if (t.id === 'pp-about') {
    if (e.key === 'Escape') { e.preventDefault(); S.pj.aboutEdit = null; return render() }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') { e.preventDefault(); return aboutSave() }
  }
  if (t.id === 'pp-sug-v' && e.key === 'Enter') { e.preventDefault(); const b = document.querySelector('[data-pp="sug-save"]'); if (b) b.click() }
  if (t.id === 'pp-sug-v' && e.key === 'Escape') { e.preventDefault(); S.pj.ppSugEdit = null; render() }
})
// The org chart: a new workstream column (kept in the page until someone is dropped on it), reporting lines.
document.addEventListener('keydown', (e) => {
  const t = e.target
  if (t.id !== 'oc-new-col') return
  if (e.key === 'Escape') { S.pj.ocNewCol = null; return render() }
  if (e.key === 'Enter' && t.value.trim()) { S.pj.ocCols = [...(S.pj.ocCols || []), { org: t.dataset.org, name: t.value.trim() }]; S.pj.ocNewCol = null; render() }
})
document.addEventListener('change', (e) => { if (e.target.matches && e.target.matches('[data-pp-reports]')) { S.pj.ocReports = e.target.checked; render() } })
// Drag a card on the org chart: onto a person (works for them here), a column (that workstream), the
// governance band.
document.addEventListener('dragover', (e) => {
  const q = e.target.closest && (e.target.closest('[data-oc-person]') || e.target.closest('[data-oc-drop]'))
  if (q && [...e.dataTransfer.types].includes('text/x-echo-person')) { e.preventDefault(); document.querySelectorAll('.oc .over').forEach((x) => x !== q && x.classList.remove('over')); q.classList.add('over') }
})
document.addEventListener('drop', (e) => {
  const card = e.target.closest && e.target.closest('[data-oc-person]')
  const col = e.target.closest && e.target.closest('[data-oc-drop]')
  const key = e.dataTransfer.getData('text/x-echo-person')
  if (!key || !(card || col)) return
  e.preventDefault()
  document.querySelectorAll('.oc .over').forEach((x) => x.classList.remove('over'))
  if (card && card.dataset.ocPerson !== key) return personPlace(key, { works_for: card.dataset.ocPerson })
  if (!col || card) return
  const [, g] = col.dataset.ocDrop.split('|')
  personPlace(key, g === 'gov' ? { governance: true } : { governance: false, group: g || '' })
})
// Drag a chip to a quadrant: high or low influence and interest.
document.addEventListener('dragstart', (e) => { const c = e.target.closest && e.target.closest('[data-pp-drag]'); if (c) { e.dataTransfer.setData('text/x-echo-person', c.dataset.ppDrag); e.dataTransfer.effectAllowed = 'move' } })
document.addEventListener('dragover', (e) => { const q = e.target.closest && e.target.closest('[data-pp-drop]'); if (q && [...e.dataTransfer.types].includes('text/x-echo-person')) { e.preventDefault(); q.classList.add('over') } })
document.addEventListener('dragleave', (e) => { const q = e.target.closest && e.target.closest('[data-pp-drop]'); if (q && !q.contains(e.relatedTarget)) q.classList.remove('over') })
document.addEventListener('drop', (e) => {
  const q = e.target.closest && e.target.closest('[data-pp-drop]')
  const key = e.dataTransfer.getData('text/x-echo-person')
  if (!q || !key) return
  e.preventDefault()
  q.classList.remove('over')
  const [inf, int] = q.dataset.ppDrop === 'tray' ? ['', ''] : q.dataset.ppDrop.split('/').map(Number)
  personPlace(key, { influence: inf, interest: int })
})

// ── the Brain (`brain/`): what Echo knows across calls, by client and thread ──────────────
// Its topics and terms to review (two layers: your practice, and each client's), the sources it
// holds, its coverage, and a graph of documents around their topics. Asking is Claude's job (the
// window's Claude sessions have the Brain); this shows and edits what it is made of.
const BRAIN_TABS = [['topics', 'Topics'], ['terms', 'Terms'], ['people', 'People'], ['sources', 'Sources'], ['numbers', 'Numbers'], ['coverage', 'Coverage'], ['graph', 'Graph']]
const brainQuery = () => { const [k, v] = (S.br.scope || '').split(':'); return k && v ? `?${k}=${encodeURIComponent(v)}` : '' }
async function loadBrain() {
  const q = brainQuery()
  try { const d = await get('api/brain' + q); d.q = q; S.br.data = d } catch { /* app restarting */ }
  if (S.br.tab === 'graph') { try { const g = await get('api/brain/graph' + q); g.q = q; S.br.graph = g } catch { /* app restarting */ } }
  if (S.br.tab === 'numbers') { try { S.br.numbers = await get('api/brain/numbers') } catch { /* app restarting */ } }
  if (S.br.tab === 'people') await loadBrainPeople()
  if (S.br.node) await loadBrainNode()
}
async function loadBrainNode() {
  const id = S.br.node
  try { const d = await get(`api/brain/node?id=${encodeURIComponent(id)}${brainQuery().replace('?', '&')}`); if (S.br.node === id) S.br.nodeData = d } catch { /* app restarting */ }
}
const clientName = (id) => id === '-' ? 'No client' : ((projClient(id) || {}).name || id)
const threadChips = (ids) => (ids || []).filter((t) => t !== '-').map((t) => projById(t)).filter(Boolean).map((p) => `<span class="tk-pj">${projIcon(p, 12)}${esc(p.name)}</span>`).join('') + ((ids || []).includes('-') ? '<span class="tk-pj br-none">no thread</span>' : '')
function brainView() {
  const d = S.br.data
  if (!d) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<div class="br"><p class="error">${esc(d.error)}</p></div>`
  const st = d.status || {}, tx = d.taxonomy || {}, sc = d.scopes || { clients: [], threads: [] }
  const scopeSel = `<label class="fchip${S.br.scope ? ' on' : ''}">${esc(S.br.scope ? (S.br.scope.startsWith('client:') ? clientName(S.br.scope.slice(7)) : ((projById(S.br.scope.slice(7)) || {}).name || 'In no thread')) : 'Everything')} ▾<select data-br-scope aria-label="Scope"><option value="">Everything</option><optgroup label="Clients">${sc.clients.map((c) => `<option value="client:${esc(c.id)}"${S.br.scope === `client:${c.id}` ? ' selected' : ''}>${esc(c.name)} · ${c.documents}</option>`).join('')}</optgroup><optgroup label="Threads">${sc.threads.map((t) => `<option value="thread:${esc(t.id)}"${S.br.scope === `thread:${t.id}` ? ' selected' : ''}>${esc(t.name)} · ${t.documents}</option>`).join('')}</optgroup></select></label>`
  const n = { topics: (tx.topics || []).length, terms: (tx.terms || []).length, sources: (d.documents || []).length }
  const tabs = BRAIN_TABS.map(([id, l]) => `<button class="tab" role="tab" data-br-tab="${id}" aria-selected="${S.br.tab === id}">${l}${n[id] ? `<span class="n">${n[id]}</span>` : ''}</button>`).join('')
  const sub = st.exists ? `${st.documents} sources · ${st.chunks} sections${st.embed_model ? ` · meaning by ${esc(st.embed_model)}` : ' · words only (no embedding server)'}` : 'Empty — it fills as your calls are synced'
  const head = `<header class="pj-head br-head"><div class="br-title"><h2>Brain</h2><span class="tk-sub">${sub}</span><span class="grow"></span>${scopeSel}<button class="btn ghost" data-go="settings" title="Settings → Brain">Settings</button></div><div class="tabs" role="tablist">${tabs}</div></header>`
  const body = S.br.tab === 'terms' ? brainTerms(tx) : S.br.tab === 'sources' ? brainSources(d) : S.br.tab === 'numbers' ? brainNumbers() : S.br.tab === 'coverage' ? brainCoverage(tx) : S.br.tab === 'graph' ? brainGraph() : S.br.tab === 'people' ? brainPeople() : brainTopics(tx)
  return `<div class="br">${head}<div class="pane pj-pane wide br-pane">${body}</div></div>`
}
function brainTopics(tx) {
  if (!tx.exists) return `<div class="wk-empty"><b>No topics yet</b><p>Claude builds them from your calls — your practice, and each client's own — and you review them here. <code>echoctl brain taxonomy build</code></p></div>`
  // One tree for the whole Brain: the practice's topics, each with its subtopics and the clients' topics
  // placed under it (a chip names the client); then what only one client has. Builds stay per layer
  // (the practice, each client): their state and actions above the tree; suggested moves (up to the
  // practice, down to a client) next to it.
  const layers = tx.layers || []
  const topics = tx.topics || []
  const lname = (id) => id === 'shared' ? 'Your practice' : ((layers.find((l) => l.id === id) || {}).name || id)
  const lcolor = (id) => { const i = layers.filter((l) => l.id !== 'shared').findIndex((l) => l.id === id); return i < 0 ? '#8a8a95' : CLIENT_COLORS[i % CLIENT_COLORS.length] }
  const open = (id, dflt) => (S.br.fold || {})[id] == null ? dflt : !(S.br.fold || {})[id]
  const jobs = new Set([...((S.br.data || {}).topic_jobs || []), ...Object.keys(S.br.rebuilding || {})])
  const catchAll = new Map(layers.filter((l) => l.drift && (l.drift.reasons || []).some((r) => r.rule === 'catch_all') && l.drift.top).map((l) => [`${l.id}|${l.drift.top.label}`, Math.round(l.drift.top.pct)]))
  const kidsOf = (x) => topics.filter((k) => k.level === 2 && k.parent_id === x.id).sort((a, b) => (a.layer !== 'shared') - (b.layer !== 'shared') || (b.sections || 0) - (a.sections || 0))
  const roots = topics.filter((x) => x.level === 1 || !x.parent_id)
  const max = Math.max(1, ...roots.map((t) => t.sections || 0))
  const total = roots.reduce((n, t) => n + (t.sections || 0), 0)
  const row = (x, top) => {
    const sub = kidsOf(x)
    const isOpen = open(x.id, false)
    const pct = total ? Math.round(100 * (x.sections || 0) / total) : 0
    const ca = catchAll.get(`${x.layer}|${x.label}`)
    const flag = ca ? `<span class="br-flag" title="Took ${ca}% of ${esc(lname(x.layer))}'s new sections: a catch-all — split it, or rebuild">took ${ca}% of new</span>` : ''
    const chip = x.layer !== 'shared' ? `<span class="br-cl" style="--c:${lcolor(x.layer)}">${esc(lname(x.layer))}</span>` : ''
    const syn = (x.synonyms || []).length ? ` · also ${esc(x.synonyms.slice(0, 3).join(', '))}` : ''
    const fold = sub.length ? `<button class="br-fold" data-br-fold="${esc(x.id)}" aria-expanded="${isOpen}" title="${isOpen ? 'Fold' : 'Show'} ${sub.length} subtopic${sub.length === 1 ? '' : 's'}">${isOpen ? '▾' : '▸'}</button>` : '<span class="br-fold"></span>'
    const subs = sub.length && !isOpen ? ` <span class="set-sub">· ${sub.length} sub${sub.some((k) => k.layer !== 'shared') ? `, ${[...new Set(sub.filter((k) => k.layer !== 'shared').map((k) => lname(k.layer)))].map(esc).join(', ')}` : ''}</span>` : ''
    return `<div class="br-tr${top ? '' : ' l2'}${S.br.node === x.id ? ' on' : ''}">${fold}<button class="br-tm" data-br-node="${esc(x.id)}"><span class="br-tn">${top ? `<b>${esc(x.label)}</b>` : esc(x.label)} ${chip}${subs}${flag}</span>${x.description || syn ? `<span class="br-td">${esc(x.description || '')}${syn}</span>` : ''}</button>
      <span class="br-bar" title="${x.sections || 0} sections · ${pct}%"><i style="width:${Math.round(100 * (x.sections || 0) / max)}%"></i></span><span class="br-n">${x.sections || 0}<small> · ${pct}%</small></span><span class="br-th">${threadChips(x.threads)}</span></div>${isOpen ? sub.map((k) => row(k, false)).join('') : ''}`
  }
  const practice = roots.filter((x) => x.layer === 'shared').sort((a, b) => (b.sections || 0) - (a.sections || 0))
  const own = layers.filter((l) => l.id !== 'shared').map((l) => [l, roots.filter((x) => x.layer === l.id).sort((a, b) => (b.sections || 0) - (a.sections || 0))]).filter(([, xs]) => xs.length)
  // Builds: one line per layer — in plain words what is wrong, if anything, and the one thing to do
  // about it (the button that does it); Tag again stays as a quiet extra.
  const why = (d) => (d.reasons || []).map((r) => r.text.charAt(0).toUpperCase() + r.text.slice(1) + '.').join(' ')
  const build = (l) => {
    const d = l.drift || {}
    const running = jobs.has(`taxonomy:${l.id}`) ? 'Rebuilding its topics from its calls…' : jobs.has(`split:${l.id}`) ? 'Splitting a topic into subtopics…' : jobs.has(`retag:${l.id}`) ? 'Tagging its sections again…' : ''
    // One topic took most of the new sections (#31): splitting that topic fixes it; a rebuild alone doesn't.
    const broad = ((d.reasons || []).find((r) => r.rule === 'catch_all' && r.topic) || {}).topic
    const splitBtn = broad ? `<button class="btn primary" data-br-split="${esc(JSON.stringify([broad.layer, broad.label, l.id]))}" title="Make 2–6 subtopics of it from its own sections (Claude), then tag only those again">Split “${esc(broad.label)}”</button>` : ''
    const retag = `<button class="link-btn" data-br-retag="${esc(l.id)}" title="Tag its sections again with the topics as they are now (no new topics)">Tag again</button>`
    const rebuild = `<button class="link-btn" data-br-rebuild="${esc(l.id)}" title="Build its topics again from its calls (Claude): existing topics keep their place, a provisional version to review">Rebuild</button>`
    const accept = `<button class="btn${d.stale ? '' : ' primary'}" data-br-accept="${esc(l.id)}">Accept</button>`
    const calls = `Built from ${l.built_from || 0} call${l.built_from === 1 ? '' : 's'}.`
    let head, text, acts
    if (running) [head, text, acts] = [running, 'You can keep working; a notification says when it is done.', '']
    else if (d.stale) [head, text, acts] = ['Needs a rebuild', `${why(d)} ${broad ? `Split makes subtopics of “${broad.label}” from its own sections and tags only those again; Rebuild makes a new version of every topic.` : 'Rebuild makes a new version from its calls for you to review; its topics keep their place.'}${l.provisional ? ' Or accept this version as it is.' : ''}`,
      `${splitBtn}<button class="btn${broad ? '' : ' primary'}" data-br-rebuild="${esc(l.id)}">Rebuild</button>${l.provisional ? accept : ''}<button class="link-btn" data-br-snooze="${esc(l.id)}" title="Ask again once it has half as many new sections or calls again">Not now</button>${retag}`]
    else if (l.provisional) [head, text, acts] = ['Waiting for your review', 'Claude built these topics and nobody accepted them yet. Look through its topics below; rename, merge or remove what is wrong, then Accept.', `${accept}${retag}${rebuild}`]
    else if (d.snoozed) [head, text, acts] = ['Rebuild put off', `${why(d)} You chose Not now; Echo asks again once it has half as many new sections or calls again.`, `${retag}${rebuild}`]
    else if (!l.built_from) [head, text, acts] = ['Not built from calls yet', 'Rebuild makes its topics from its calls.', `${retag}${rebuild}`]
    else [head, text, acts] = ['Nothing to do', `${d.untagged ? `${d.untagged} new section${d.untagged === 1 ? ' fits' : 's fit'} no topic, too few to matter yet. ` : ''}${calls}`, `${retag}${rebuild}`]
    const kind = running ? 'run' : d.stale ? 'warn' : l.provisional ? 'todo' : 'ok'
    return `<div class="br-bld ${kind}"><span class="br-cl" style="--c:${l.id === 'shared' ? '#8a8a95' : lcolor(l.id)}">${esc(lname(l.id))}</span><div class="br-bld-t"><b>${running ? '<i class="spin"></i>' : ''}${esc(head)}</b><span class="set-sub">${esc(text)}</span></div><div class="br-bld-a">${acts}</div></div>`
  }
  const ordered = [...layers.filter((l) => l.id === 'shared'), ...layers.filter((l) => l.id !== 'shared')]
  const nStale = ordered.filter((l) => l.drift && l.drift.stale).length, nReview = ordered.filter((l) => l.provisional).length
  const todo = [nStale ? `${nStale} need${nStale === 1 ? 's' : ''} a rebuild` : '', nReview ? `${nReview} waiting for your review` : ''].filter(Boolean).join(' · ') || 'nothing to do'
  const builds = `<details class="br-blds"${nStale || nReview || jobs.size ? ' open' : ''}><summary>Topic builds · ${ordered.length} layer${ordered.length === 1 ? '' : 's'} · ${todo}</summary>${ordered.map(build).join('')}</details>`
  const mv = (tx.moves || []).map((m) => `<div class="br-mv"><span class="grow">${m.dir === 'up' ? `↑ <b>${esc(m.label)}</b> belongs to your practice? <span class="set-sub">${esc(m.why)}</span>` : `↓ <b>${esc(m.label)}</b> is only ${esc(m.to_name)}'s? <span class="set-sub">${esc(m.why)}</span>`}</span><button class="btn ghost" data-br-move="${esc(m.key)}" data-do="dismiss">Keep it</button><button class="btn primary" data-br-move="${esc(m.key)}" data-do="apply">${m.dir === 'up' ? 'Move up' : 'Move down'}</button></div>`).join('')
  const top = `<div class="br-tbar"><span class="set-sub">${topics.length} topics · version ${tx.version}</span><span class="grow"></span><button class="btn" data-brain-sync title="Read new calls, mail and files into the Brain now; their new sections are tagged after it">Sync now</button></div>`
  const tree = practice.length ? `<div class="br-card">${practice.map((x) => row(x, true)).join('')}</div>` : ''
  const only = own.map(([l, xs]) => `<section class="br-layer"><div class="br-lh"><span class="cap">Only ${esc(lname(l.id))}</span><span class="set-sub">topics no other client shares — move one up when it does</span></div><div class="br-card">${xs.map((x) => row(x, true)).join('')}</div></section>`).join('')
  return `${top}${builds}${mv ? `<div class="br-mvs">${mv}</div>` : ''}${practice.length ? `<section class="br-layer"><div class="br-lh"><span class="cap">Your practice</span><span class="set-sub">one tree for every client: their topics sit under the practice's where they fit</span></div>${tree}</section>` : ''}${only}` || '<p class="set-sub">Nothing in this scope.</p>'
}
/** The dictionary (the Terms tab): every term — named things: organisations, products, initiatives… — by
 *  group, A–Z within it; a search over its name, other names and spellings; each with what it is, whose it
 *  is (the practice, or a client), the topics it comes up in and how often. A click opens it to edit. */
function brainTerms(tx) {
  if (!tx.exists) return '<p class="set-sub">No terms yet: they come with the topics.</p>'
  const layers = tx.layers || []
  const lname = (id) => id === 'shared' ? 'Practice' : ((layers.find((l) => l.id === id) || {}).name || id)
  const lcolor = (id) => { const i = layers.filter((l) => l.id !== 'shared').findIndex((l) => l.id === id); return i < 0 ? '#8a8a95' : CLIENT_COLORS[i % CLIENT_COLORS.length] }
  const q = (S.br.tq || '').trim().toLowerCase()
  const hit = (x) => !q || [x.name, ...(x.synonyms || []), ...(x.variants || []), x.description, x.kind].join(' ').toLowerCase().includes(q)
  const all = (tx.terms || []).filter(hit)
  const g = S.br.tg || ''
  const groups = (tx.groups || []).map((x) => ({ ...x, n: all.filter((t) => t.group === x.id).length })).filter((x) => x.n)
  const shown = all.filter((t) => !g || t.group === g).sort((a, b) => a.name.localeCompare(b.name))
  const chips = `<div class="dict-groups"><button class="fchip${!g ? ' on' : ''}" data-dict-group="">All · ${all.length}</button>${groups.map((x) => `<button class="fchip${g === x.id ? ' on' : ''}" data-dict-group="${esc(x.id)}">${esc(x.label)} · ${x.n}</button>`).join('')}</div>`
  const box = `<input class="wp-in br-pq" id="dict-q" type="search" placeholder="Find a term — its name, another name, a spelling" value="${esc(S.br.tq || '')}" aria-label="Find a term">`
  const entry = (x) => {
    const names = (x.synonyms || []).length ? `<span class="dict-aka">also ${esc(x.synonyms.join(', '))}</span>` : ''
    const flags = [x.always ? '<span class="br-pin" title="Hints every call’s transcription">∞</span>' : '', (x.pinned || []).length ? '<span class="br-pin" title="Pinned to a thread: hints its calls’ transcription">📌</span>' : '', x.learned ? '<span class="dict-flag" title="Learned from the minutes">learned</span>' : ''].join('')
    const tps = (x.topics || []).map((t) => `<span class="dict-tp">${esc(t.label)}</span>`).join('')
    return `<button class="dict-e${S.br.node === x.id ? ' on' : ''}" data-br-node="${esc(x.id)}"><span class="dict-h"><b>${esc(x.name)}</b> <span class="br-cl" style="--c:${lcolor(x.layer)}">${esc(lname(x.layer))}</span> <span class="set-sub">${esc(x.kind || '')}</span>${flags}<span class="grow"></span><span class="br-n" title="Sections that mention it">${x.sections || 0}</span></span>${names}${x.description ? `<span class="dict-d">${esc(x.description)}</span>` : ''}${tps ? `<span class="dict-tps">${tps}</span>` : ''}</button>`
  }
  const body = !shown.length ? `<p class="set-sub">${q ? 'Nothing matches.' : 'No terms here.'}</p>`
    : g ? `<div class="dict-list">${shown.map(entry).join('')}</div>`
    : groups.map((x) => `<section class="br-layer"><div class="br-lh"><span class="cap">${esc(x.label)}</span><span class="set-sub">${x.n}</span></div><div class="dict-list">${shown.filter((t) => t.group === x.id).map(entry).join('')}</div></section>`).join('')
  return `<div class="br-graph-bar">${box}<span class="grow"></span></div>${chips}${body}<p class="set-foot">A term is a named thing: its name, its other names (what search and Claude take for it) and its spellings (how it is written or misheard — they hint transcription). Merge two that are one thing; split off a name that is a thing of its own.</p>`
}
/** A Brain section's source outside Echo (a Teams post, a mail): opened in its own app. */
const extAttr = (link) => link && link.startsWith('echo://file/') ? `data-br-file="${esc(link.slice(12).split('?')[0])}" title="Open the document${link.includes('page=') ? ` (page ${esc(link.split('page=')[1])})` : ''}"`
  : link && /^https:\/\//i.test(link) ? `data-br-ext="${esc(link)}" title="Open in ${/outlook|office\.com|owa/i.test(link) ? 'Outlook' : 'Teams'}"` : ''
const KIND_MARK = { call: '▮', channel: '#', mail: '✉', file: '▣' }
const kindMark = (x) => KIND_MARK[x.kind || (x.source || '').split('/')[0]] || '·'
document.addEventListener('click', (e) => {
  const f = e.target.closest && e.target.closest('[data-br-file]')
  if (f) { e.preventDefault(); e.stopPropagation(); return send({ cmd: 'open_brain_file', id: f.dataset.brFile }) }
  const b = e.target.closest && e.target.closest('[data-br-ext]')
  if (!b) return
  e.preventDefault(); e.stopPropagation()
  send({ cmd: 'open_url', url: b.dataset.brExt })
}, true)
/** Who may speak into the Brain through mail and channels, per client: senders waiting for a decision
 *  (their messages aren't read until trusted), then the trusted and ignored ones. Colleagues and the
 *  user are trusted without asking; a trusted outside sender's mail is read only when authenticated. */
function brainSenders() {
  loadSourceState()
  const s = (m365.state || {}).senders
  if (!s) return ''
  const scope = S.br.scope && S.br.scope.startsWith('client:') ? S.br.scope.slice(7) : null
  const name = (c) => c === '-' ? 'No client' : ((((S.pj.data || {}).clients) || []).find((x) => x.id === c) || {}).name || c
  const cards = Object.entries(s.clients || {}).filter(([c]) => !scope || c === scope).map(([c, l]) => {
    const btn = (k, d, label, primary) => `<button class="btn${primary ? ' primary' : ''}" data-sender="${esc(d)}" data-client="${esc(c)}" data-key="${esc(k)}">${label}</button>`
    const who = (x) => `<b>${esc(x.name || x.key)}</b>${x.key.startsWith('teams:') ? '' : ` <span class="set-sub">${esc(x.key)}</span>`}`
    const wait = (l.waiting || []).sort((a, b) => (b.last_at || '').localeCompare(a.last_at || '')).map((w) => `<tr><td><span class="pp-who"><span class="br-kind">${w.kind === 'teams' ? '#' : '✉'}</span>${who(w)}<span class="set-sub br-why">${w.count} message${w.count === 1 ? '' : 's'} · last ${esc(relDay((w.last_at || '').slice(0, 10)))} · “${esc(w.last_about)}”${w.kind === 'mail' && !w.verified ? ' · not authenticated' : ''}${w.hint ? ' · ' + esc(w.hint) : ''}</span></span></td><td class="br-acts">${btn(w.key, 'trust', 'Trust', true)}${btn(w.key, 'ignore', 'Ignore')}</td></tr>`)
    // A trusted sender's mail with no authentication verdict (a mailing list, a domain without DMARC).
    const unver = (l.unverified || []).map((w) => `<tr><td><span class="pp-who"><span class="br-kind">✉</span>${who(w)}<span class="set-sub br-why">${w.count} message${w.count === 1 ? '' : 's'} not authenticated · last ${esc(relDay((w.last_at || '').slice(0, 10)))} · “${esc(w.last_about)}” · could be forged</span></span></td><td class="br-acts">${btn(w.key, 'read_unverified', 'Read anyway')}${btn(w.key, 'keep_unverified_out', 'Keep out')}</td></tr>`)
    const unverWord = (x) => x.unverified === true ? ' · unauthenticated mail read too' : x.unverified === false ? ' · unauthenticated mail kept out' : ''
    const decided = [...(l.trusted || []).map((x) => [x, 'trusted' + unverWord(x)]), ...(l.ignored || []).map((x) => [x, 'ignored'])].map(([x, st]) => `<tr class="br-sub"><td><span class="pp-who">${who(x)}<span class="set-sub br-why">${st}</span></span></td><td class="br-acts">${btn(x.key, 'forget', 'Forget')}</td></tr>`)
    if (!wait.length && !unver.length && !decided.length) return ''
    return `<div class="pp-card"><table class="pp-tbl"><thead><tr><th>${esc(name(c))} · senders${wait.length ? ` · <span class="error">${wait.length} waiting</span>` : ''}${unver.length ? ` · ${unver.length} not authenticated` : ''}</th><th></th></tr></thead><tbody>${wait.join('')}${unver.join('')}${decided.join('')}</tbody></table></div>`
  }).filter(Boolean)
  return cards.length ? `<p class="set-sub">Mail and posts are read only from you, colleagues and the senders you trust; the Brain keeps facts read from them, never their text.</p>${cards.join('')}` : ''
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-sender]')
  if (!b) return
  b.disabled = true
  send({ cmd: 'brain_sender', client: b.dataset.client, key: b.dataset.key, decision: b.dataset.sender })
  setTimeout(() => loadSourceState(true), 400)
})
function brainSources(d) {
  const senders = brainSenders()
  const docs = d.documents || []
  if (!docs.length) return senders + '<p class="set-sub">No sources in this scope yet.</p>'
  const threads = (x) => threadChips((x.threads || []).map((t) => t.id).concat((x.threads || []).length ? [] : ['-']))
  const row = (x, sub) => {
    const call = x.link && x.link.startsWith('echo://call/') ? x.link.slice(12) : ''
    const title = sub && x.channel && (x.title || '').startsWith(x.channel + ': ') ? x.title.slice(x.channel.length + 2) : (x.title || x.source)
    const hist = x.history ? ' <span class="br-hist" title="Dated before its thread\'s history date: kept, searched only when history is asked for">history</span>' : ''
    return `<tr${call ? ` data-call="${esc(call)}"` : ` ${extAttr(x.link)}`}${sub ? ' class="br-sub"' : ''}><td><span class="pp-who"><span class="br-kind">${kindMark(x)}</span>${sub ? esc(title) : `<b>${esc(title)}</b>`}${hist}</span></td><td>${x.date ? esc(relDay(x.date)) : ''}</td><td>${threads(x)}</td><td class="num">${x.sections}</td></tr>`
  }
  // A channel is one row (its posts, newest first, open under it); a folder too (every file in it,
  // read or not, with why); calls and mail one row each.
  loadSourceState()
  const out = [], seen = new Set()
  const STATUS = { indexed: 'read', pending: 'read · pages waiting for the page reader', twin: 'not read', skipped: 'not read', unsupported: 'not read', error: "couldn't be read", reading: 'reading…' }
  for (const x of docs) {
    if (x.folder) {
      if (seen.has('f:' + x.folder)) continue
      seen.add('f:' + x.folder)
      const files = ((m365.files || {})[x.folder]) || []
      const open = S.br.openCh === 'f:' + x.folder
      const name = x.folder.split(/[\\/]/).pop()
      const fdocs = docs.filter((y) => y.folder === x.folder)
      out.push(`<tr data-br-ch="${esc('f:' + x.folder)}" class="br-ch" title="${esc(x.folder)}"><td><span class="pp-who"><span class="br-kind">▣</span><b>${esc(name)}</b><span class="set-sub">${esc(folderSummary(x.folder).line)} ${open ? '▾' : '▸'}</span></span></td><td>${fdocs[0] && fdocs[0].date ? esc(relDay(fdocs[0].date)) : ''}</td><td>${threads(x)}</td><td class="num">${fdocs.reduce((n, y) => n + y.sections, 0)}</td></tr>`)
      if (open) {
        for (const f of files) {
          const d = fdocs.find((y) => y.source === 'file/' + f.id)
          const histTag = d && d.history ? ' <span class="br-hist">history</span>' : ''
          const why = [STATUS[f.status] || f.status, f.pages ? `${f.pages} page${f.pages === 1 ? '' : 's'}${f.flagged ? `, ${f.read || 0}/${f.flagged} read from images` : ''}` : '', f.fidelity && f.fidelity !== 'full' ? f.fidelity : '', f.reason || ''].filter(Boolean).join(' · ')
          out.push(`<tr class="br-sub${d ? '' : ' br-off'}"${d ? ` data-br-file="${esc(f.id)}"` : ''}><td><span class="pp-who"><span class="br-kind">${d ? '▣' : '·'}</span>${esc(f.rel)}${histTag}<span class="set-sub br-why">${esc(why)}</span></span></td><td>${f.date ? esc(relDay(f.date)) : ''}</td><td></td><td class="num">${d ? d.sections : ''}</td></tr>`)
        }
      }
      continue
    }
    if (!x.channel) { out.push(row(x)); continue }
    if (seen.has(x.channel)) continue
    seen.add(x.channel)
    const posts = docs.filter((y) => y.channel === x.channel)
    const open = S.br.openCh === x.channel
    out.push(`<tr data-br-ch="${esc(x.channel)}" class="br-ch"><td><span class="pp-who"><span class="br-kind">#</span><b>${esc(x.channel)}</b><span class="set-sub">${posts.length} post${posts.length === 1 ? '' : 's'} ${open ? '▾' : '▸'}</span></span></td><td>${x.date ? esc(relDay(x.date)) : ''}</td><td>${threads(x)}</td><td class="num">${posts.reduce((n, y) => n + y.sections, 0)}</td></tr>`)
    if (open) posts.forEach((y) => out.push(row(y, true)))
  }
  return `${senders}<div class="pp-card"><table class="pp-tbl"><thead><tr><th>Source</th><th>Date</th><th>Threads</th><th class="num">Sections</th></tr></thead><tbody>${out.join('')}</tbody></table></div>`
}
document.addEventListener('click', (e) => {
  const r = e.target.closest && e.target.closest('[data-br-ch]')
  if (!r) return
  S.br.openCh = S.br.openCh === r.dataset.brCh ? null : r.dataset.brCh
  render()
})
/** The numbers lane, per client: its reporting families (Claude proposes them from the workbooks;
 *  nothing is built until accepted), what each loaded, the catalog of governed metrics, and what the
 *  last build warns about (a file that yielded nothing, figures restated or in conflict, holes). */
function brainNumbers() {
  const d = S.br.numbers
  if (!d) return '<div class="skeleton"><i></i><i></i></div>'
  const busy = d.busy || {}
  const scope = S.br.scope && S.br.scope.startsWith('client:') ? S.br.scope.slice(7) : null
  const cs = (d.clients || []).filter((c) => !scope || c.client === scope)
  if (!cs.length) return '<p class="set-sub">No reporting workbooks yet. Add a folder to a client\'s thread (Profile → Sources → Folder); its spreadsheets show up here.</p>'
  return cs.map((c) => {
    const cfg = c.config || {}, fams = cfg.families || [], st = c.status
    const job = busy[c.client], err = busy[`${c.client}:error`]
    const units = c.units || []
    const famStat = (name) => { const u = units.filter((x) => x.family === name); const k = (s) => u.filter((x) => x.status === s).length; return u.length ? `${k('ok')} ok${k('partial') ? ` · ${k('partial')} partial` : ''}${k('zero') + k('error') ? ` · <span class="error">${k('zero') + k('error')} empty</span>` : ''}` : '' }
    const rounds = ((cfg._echo || {}).rounds || []), lastRound = (cfg._echo || {}).kept || rounds[rounds.length - 1]
    const state = job ? `<span class="set-sub">${job.op === 'propose' ? 'Claude is reading the workbooks and proposing families — a few minutes…' : job.op === 'accept' ? 'Accepting and building…' : 'Building…'}</span>`
      : st === 'accepted' ? `<span class="tk-pj">accepted</span><span class="set-sub">${c.built_at ? `built ${esc(relDay(String(c.built_at).slice(0, 10)))} · ${c.audit ? c.audit.facts : 0} facts` : 'not built yet'}</span>`
      : st === 'proposed' ? `<span class="tk-pj br-prop">proposed</span><span class="set-sub">${lastRound ? `loads ${lastRound.ok} of ${lastRound.units} sheets fully · ${lastRound.facts} facts — review, then accept` : 'review, then accept'}</span>` : `<span class="set-sub">${c.workbooks} workbook${c.workbooks === 1 ? '' : 's'} · no families yet</span>`
    const acts = job ? '' : [
      `<button class="btn${st ? '' : ' primary'}" data-num="propose" data-client="${esc(c.client)}">${st ? 'Propose again' : 'Propose families'}</button>`,
      st === 'proposed' ? `<button class="btn primary" data-num="accept" data-client="${esc(c.client)}">Accept and build</button>` : '',
      st === 'accepted' ? `<button class="btn" data-num="build" data-client="${esc(c.client)}">Build now</button>` : '',
    ].join('')
    const famRows = fams.map((f) => `<tr><td><b>${esc(f.name)}</b><div class="set-sub">${esc([].concat(f.glob).join(' · '))}</div></td><td>${esc(f.layout)}</td><td>${esc(Object.keys(f.grains || {}).concat(Object.keys(f.dim_candidates || {})).join(', ') || '—')}</td><td title="${esc(Object.keys(f.measures || {}).join(', '))}">${Object.keys(f.measures || {}).length}</td><td>${famStat(f.name)}</td></tr>`).join('')
    const cat = Object.entries(c.catalog || {})
    const catRows = cat.map(([n, m]) => `<tr><td><b>${esc(n)}</b></td><td>${esc(m.unit || '')}</td><td>${esc(m.desc || '')}${m.definition ? `<div class="set-sub">${esc(m.definition)}</div>` : ''}</td></tr>`).join('')
    const warn = units.filter((x) => x.status === 'zero' || x.status === 'error' || x.status === 'partial').slice(0, 8)
    const a = c.audit || {}
    const notes = [a.restated ? `${a.restated} figures restated by a later report` : '', a.conflicts ? `${a.conflicts} where reports of one month disagree` : '', a.holes ? `${a.holes} grains with an entity missing some months` : '', a.collisions ? `${a.collisions} rows listed twice in one file (${a.collisions_merged} merged by policy)` : ''].filter(Boolean)
    const skipped = ((cfg._echo || {}).skipped || [])
    return `<section class="set-sec num-sec">
      <div class="ins-h"><h3>${esc(clientName(c.client))}</h3>${state}<span class="grow"></span>${acts}</div>
      ${err ? `<p class="error">${esc(err)}</p>` : ''}
      ${fams.length ? `<div class="pp-card"><table class="pp-tbl"><thead><tr><th>Family</th><th>Layout</th><th>Grains</th><th class="num">Measures</th><th>Last build</th></tr></thead><tbody>${famRows}</tbody></table></div>` : ''}
      ${notes.length || warn.length ? `<div class="num-warn">${notes.map((n) => `<div>⚠ ${esc(n)}</div>`).join('')}${warn.map((x) => `<div class="set-sub">${esc(x.status)} · ${esc(x.family)} / ${esc(x.unit)} · ${esc(String(x.file || '').split('/').pop())} — ${esc(x.reason || '')}</div>`).join('')}</div>` : ''}
      ${cat.length ? `<details class="num-cat"><summary>${cat.length} governed metrics — what Claude's sessions can ask for (list_metrics, get_metric)</summary><div class="pp-card"><table class="pp-tbl"><tbody>${catRows}</tbody></table></div></details>` : ''}
      ${skipped.length ? `<details class="num-cat"><summary>${skipped.length} workbooks left out</summary>${skipped.map((s) => `<div class="set-sub">${esc(String(s.series || '').split('/').pop())} — ${esc(s.why || '')}</div>`).join('')}</details>` : ''}
    </section>`
  }).join('') + '<p class="set-foot">Numbers are computed from the workbooks\' real cells and cited to their file — never read or estimated by a model. Claude proposes which tables to load; nothing is built until you accept. Rebuilt when the workbooks change.</p>'
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-num]')
  if (!b) return
  send({ cmd: 'brain_numbers', op: b.dataset.num, client: b.dataset.client })
  b.disabled = true
  const poll = () => get('api/brain/numbers').then((d) => { S.br.numbers = d; render(); if (Object.keys(d.busy || {}).some((k) => !k.endsWith(':error'))) setTimeout(poll, 3000) }).catch(() => {})
  setTimeout(poll, 600)
})

function brainCoverage(tx) {
  const c = tx.coverage
  if (!c || !c.total) return '<p class="set-sub">Nothing to cover yet.</p>'
  const pct = (n) => Math.round(100 * n / c.total)
  const bar = (label, n, cls) => `<div class="br-cov"><span>${label}</span><span class="br-bar"><i class="${cls}" style="width:${pct(n)}%"></i></span><span class="br-n">${n} · ${pct(n)}%</span></div>`
  return `<section class="set-sec"><div class="set-card br-covs">${bar('Tagged with a topic', c.tagged, 'ok')}${bar('Small talk', c.no_topic, 'mute')}${bar('Nothing fits', c.nothing_fits, 'warn')}${bar('Not tagged yet', c.not_judged, 'mute')}</div>
    <p class="set-foot">Of ${c.total} sections. New sections are tagged after each sync; "nothing fits" growing means the topics need another one — rebuild a layer with <code>echoctl brain taxonomy build --layer …</code>.</p></section>`
}

// ── graphs: Sigma.js (WebGL) over graphology, vendored (`ui/vendor/README.md`) ─────────────────────
// One instance per graph box: built once per data set (ForceAtlas2 run in the page, then spaced
// apart), kept across renders — a render that replaces the box remounts the same graph, positions
// and all. Search focuses: the matches and their neighbours stay lit, the rest is dimmed, the camera
// flies to them; hovering does the same for one node.
const CLIENT_COLORS = ['#9b78ff', '#5aa9ff', '#3ccf91', '#f0a24a', '#e8603c', '#d36bd1', '#4cc3c8', '#c9b458']
const GraphView = (() => {
  const live = new Map() // box id → { sigma, graph, key, box, match, focus, hover, sel, opts }
  const cache = new Map() // data key → graph (laid out once)
  const css = (v, f) => (getComputedStyle(document.documentElement).getPropertyValue(v) || '').trim() || f
  function layout(graph) {
    const n = graph.order
    if (!n || typeof graphologyLibrary === 'undefined') return
    const L = graphologyLibrary
    const fa2 = L.layoutForceAtlas2
    const settings = { ...fa2.inferSettings(graph), barnesHutOptimize: n > 400, gravity: 1, scalingRatio: n > 400 ? 6 : 12, slowDown: 2 }
    fa2.assign(graph, { iterations: n > 3000 ? 60 : n > 800 ? 120 : 250, settings, getEdgeWeight: 'weight' })
    if (L.layoutNoverlap) L.layoutNoverlap.assign(graph, { maxIterations: n > 2000 ? 20 : 80, settings: { margin: 4, ratio: 1.3 } })
  }
  /** Draw `box` (an element with an id) from `build()` → a graphology graph whose nodes carry x, y,
   *  size, color, label, `group` (seeds the layout) and `pinLabel` (always labelled); edges weight,
   *  size, color. `key` names the data: the same key keeps the same graph. */
  function draw(box, key, build, opts) {
    if (!box || typeof Sigma === 'undefined' || typeof graphology === 'undefined') return null
    const id = box.id
    let v = live.get(id)
    if (v && v.key === key && v.box === box) return v
    // The same data in a new box (a render replaced it): the camera stays where it was.
    const cam = v && v.key === key ? v.sigma.getCamera().getState() : null
    if (v) { try { v.sigma.kill() } catch { /* gone with its box */ } }
    let graph = cache.get(key)
    if (!graph) { graph = build(); layout(graph); cache.set(key, graph); if (cache.size > 8) cache.delete(cache.keys().next().value) }
    box.innerHTML = ''
    const st = { key, box, graph, match: v && v.key === key ? v.match : null, focus: v && v.key === key ? v.focus : null, hover: null, sel: null, opts }
    // WebGL reads plain colours only: the theme's hex values, a dimmed node by the background's lightness.
    const bg = css('--bg', '#f6f6f8'), dark = /^#/.test(bg) && parseInt(bg.slice(1, 3), 16) < 128
    const dim = dark ? '#33333d' : '#e1e1e8', text = css('--text', dark ? '#eee' : '#222'), muted = css('--text-3', '#8a8a95'), soft = css('--text-2', '#55555f')
    // The hovered node's label sits on a card (sigma's backdrop, white by default): the theme's.
    const card = dark
      ? { backdropColor: '#26262e', backdropBorderColor: '#3a3a45', backdropBorderWidth: 1, backdropShadowColor: 'rgba(0, 0, 0, 0.55)', backdropCornerRadius: 6 }
      : { backdropColor: '#ffffff', backdropBorderColor: '#e1e1e8', backdropBorderWidth: 1, backdropShadowColor: 'rgba(17, 17, 34, 0.18)', backdropCornerRadius: 6 }
    const near = (k) => new Set([k, ...graph.neighbors(k)])
    const sigma = new Sigma(graph, box, {
      settings: { labelDensity: 0.08, labelGridCellSize: 120, labelRenderedSizeThreshold: 7, hideEdgesOnMove: graph.size > 3000, zIndex: true, minCameraRatio: 0.02, maxCameraRatio: 4, stagePadding: 24, labelFont: getComputedStyle(document.body).fontFamily, labelSize: 11 },
      nodeReducer: (k, d, a) => {
        const hov = st.hover && near(st.hover)
        const out = { ...d, ...card, labelColor: a.pinLabel ? text : muted }
        const lit = (st.focus && st.focus.has(k)) || (hov && hov.has(k)) || k === st.sel
        const off = (st.focus && !st.focus.has(k)) || (hov && !hov.has(k))
        if (off) return { ...out, color: dim, label: null, backdropVisibility: 'hidden', zIndex: 0 }
        if ((st.match && st.match.has(k)) || k === st.sel || k === st.hover) return { ...out, highlighted: true, labelVisibility: 'visible', labelColor: text, zIndex: 3 }
        // Labels where they fit (sigma's grid, the biggest first: topics and threads); lit ones always.
        if (lit) return { ...out, labelVisibility: 'visible', labelColor: a.pinLabel ? text : soft, zIndex: 2 }
        if (a.pinLabel) return { ...out, zIndex: 1 }
        return out
      },
      edgeReducer: (k, d) => {
        const [s, t] = graph.extremities(k)
        const hov = st.hover && near(st.hover)
        const on = (x) => (!st.focus || st.focus.has(x)) && (!hov || hov.has(x))
        if (!on(s) || !on(t)) return { ...d, visibility: 'hidden' }
        return (st.hover && (s === st.hover || t === st.hover)) || (st.sel && (s === st.sel || t === st.sel)) ? { ...d, color: css('--accent', '#8b62ff'), size: Math.max(d.size, 1.5), zIndex: 2 } : d
      },
    })
    st.sigma = sigma
    sigma.on('enterNode', ({ node }) => { st.hover = node; box.style.cursor = 'pointer'; sigma.refresh({ skipIndexation: true }) })
    sigma.on('leaveNode', () => { st.hover = null; box.style.cursor = ''; sigma.refresh({ skipIndexation: true }) })
    sigma.on('clickNode', ({ node }) => opts.click && opts.click(node, graph.getNodeAttributes(node)))
    live.set(id, st)
    if (cam) sigma.getCamera().setState(cam)
    return st
  }
  /** Fly the camera to these nodes (their bounding box), or back to the whole graph. */
  function fly(st, keys) {
    const cam = st.sigma.getCamera()
    const pts = [...(keys || [])].map((k) => st.sigma.getNodeDisplayData(k)).filter(Boolean)
    if (!pts.length) return cam.animate({ x: 0.5, y: 0.5, ratio: 1, angle: 0 }, { duration: 400 })
    const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y)
    const span = Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys))
    cam.animate({ x: (Math.max(...xs) + Math.min(...xs)) / 2, y: (Math.max(...ys) + Math.min(...ys)) / 2, ratio: Math.min(1, Math.max(0.08, span * 1.4)) }, { duration: 500 })
  }
  /** Light these nodes (null: none) and their neighbours, dim the rest, fly to them. */
  function focus(id, keys) {
    const st = live.get(id)
    if (!st) return 0
    const g = st.graph
    const m = keys ? new Set([...keys].filter((k) => g.hasNode(k))) : null
    st.match = m && m.size ? m : null
    st.focus = st.match ? new Set([...st.match].flatMap((k) => [k, ...g.neighbors(k)])) : null
    st.sigma.refresh({ skipIndexation: true })
    fly(st, st.match)
    return st.match ? st.match.size : 0
  }
  function select(id, key) {
    const st = live.get(id)
    if (!st || st.sel === key) return
    st.sel = key && st.graph.hasNode(key) ? key : null
    st.sigma.refresh({ skipIndexation: true })
  }
  /** Nodes whose label (or `search` text) has every word typed. */
  function find(id, q) {
    const st = live.get(id)
    const words = String(q || '').toLowerCase().split(/\s+/).filter(Boolean)
    if (!st || !words.length) return null
    const hits = new Set()
    st.graph.forEachNode((k, a) => { const t = `${a.label || ''} ${a.search || ''}`.toLowerCase(); if (words.every((w) => t.includes(w))) hits.add(k) })
    return hits
  }
  const resize = () => { for (const st of live.values()) if (st.box.isConnected) st.sigma.resize() }
  /** The theme changed (the system's appearance): drawn again in its colours, laid out as they were. */
  function retheme() {
    for (const [id, st] of live) { try { st.sigma.kill() } catch { /* gone with its box */ } live.delete(id) }
  }
  return { draw, focus, select, find, resize, retheme, live }
})()

// The Brain's graph: documents (coloured by client) and their topics (labelled, coloured by layer);
// a document joins the topics its sections are tagged with, related documents join each other.
function brainGraph() {
  const g = S.br.graph
  if (!g || g.q !== brainQuery()) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (!g.nodes || !g.nodes.length) return '<p class="set-sub">Nothing to draw in this scope yet.</p>'
  const clients = [...new Set(g.nodes.filter((n) => n.type === 'doc').map((n) => n.client || '-'))]
  const legend = clients.map((c, i) => `<span class="pp-leg"><i class="br-dot" style="background:${c === '-' ? '#8a8a95' : CLIENT_COLORS[i % CLIENT_COLORS.length]}"></i>${esc(clientName(c))}</span>`).join('')
  const box = `<input class="wp-in br-pq" id="br-gq" type="search" placeholder="Find a topic or source — Enter searches the Brain" value="${esc(S.br.gq || '')}" aria-label="Find in the graph">`
  const res = S.br.gqRes ? `<span class="set-sub br-gq-res">${esc(S.br.gqRes)}</span>` : ''
  return `<div class="br-graph-bar">${box}${res}<span class="grow"></span>${legend}</div><div class="br-graph" id="br-graph"></div><p class="set-foot">${g.nodes.length} nodes · ${g.edges.length} links. Scroll to zoom, drag to move; hover to see a node's links, click for its details. Topics are labelled; sources by size once you zoom in.</p>`
}
function graphKey(g) { return `brain|${g.q}|${g.nodes.length}|${g.edges.length}|${g.nodes.slice(0, 3).map((n) => n.id).join(',')}` }
function drawGraph() {
  const box = $('#br-graph'), g = S.br.graph
  if (!box || !g || !g.nodes) return
  const clients = [...new Set(g.nodes.filter((n) => n.type === 'doc').map((n) => n.client || '-'))]
  const color = (c) => c && c !== '-' && c !== 'shared' && clients.indexOf(c) >= 0 ? CLIENT_COLORS[clients.indexOf(c) % CLIENT_COLORS.length] : '#8a8a95'
  const st = GraphView.draw(box, graphKey(g), () => {
    const G = new graphology.Graph({ multi: false, type: 'undirected' })
    const topics = g.nodes.filter((n) => n.type === 'topic')
    // Seeds: topics on a ring by layer, documents near the middle (ForceAtlas2 does the rest).
    topics.forEach((t, i) => { const a = (2 * Math.PI * i) / Math.max(1, topics.length); G.addNode(t.id, { x: 100 * Math.cos(a), y: 100 * Math.sin(a), size: 5 + Math.sqrt(t.sections || 1) * 1.2, color: color(t.layer), label: t.label, pinLabel: true, kind: 'topic' }) })
    for (const n of g.nodes.filter((x) => x.type === 'doc')) {
      const h = ProjectArt.hash(n.id)
      G.addNode(n.id, { x: (h % 101) - 50, y: ((h >> 7) % 101) - 50, size: 2.5 + Math.sqrt(n.sections || 1) * 0.6, color: color(n.client || '-'), label: n.label || n.id, search: [n.kind, n.date, n.client && clientName(n.client)].filter(Boolean).join(' '), kind: 'doc' })
    }
    for (const e of g.edges) {
      if (!G.hasNode(e.source) || !G.hasNode(e.target) || G.hasEdge(e.source, e.target)) continue
      G.addEdge(e.source, e.target, { weight: e.weight || 1, size: e.kind === 'about' ? Math.min(2.5, 0.4 + (e.weight || 1) / 8) : 0.6, color: e.kind === 'about' ? 'rgba(130,130,145,0.22)' : 'rgba(139,98,255,0.28)' })
    }
    return G
  }, { click: (node, a) => a.kind === 'topic' ? press({ brNode: node.replace(/^topic:/, '') }) : press({ graphDoc: node }) })
  if (st) GraphView.select('br-graph', S.br.doc || (S.br.node ? `topic:${S.br.node}` : null))
}
/** The graph's search: typing lights matching topics and sources (by name); Enter asks the Brain (words
 *  and meaning) and lights the sources of the sections it finds. */
async function graphSearch(q, ask) {
  S.br.gq = q
  if (!q.trim()) { S.br.gqRes = ''; GraphView.focus('br-graph', null); return }
  if (!ask) { const hits = GraphView.find('br-graph', q); const n = GraphView.focus('br-graph', hits); S.br.gqRes = n ? `${n} by name · Enter searches the Brain` : 'no name matches · Enter searches the Brain'; return }
  S.br.gqRes = 'searching the Brain…'; render()
  try {
    const sc = brainQuery().replace('?', '&')
    const r = await get(`api/brain/search?q=${encodeURIComponent(q)}${sc}`)
    if (S.br.gq !== q) return
    const hits = r.hits || []
    const srcs = new Set(hits.map((h) => h.source).filter(Boolean))
    const n = GraphView.focus('br-graph', srcs)
    S.br.gqRes = r.error ? r.error : `${hits.length} section${hits.length === 1 ? '' : 's'} in ${n} source${n === 1 ? '' : 's'} shown`
  } catch (e) { S.br.gqRes = String(e) }
  render()
}

// ── the Brain's People: who is who across the threads (`people::everyone`) ─────────────────────
// Threads on a ring (a client's side by side, in its colour); people inside, each near the threads
// they are in. Shown: people with two sources or more (calls, mails, chats that say something about
// them), placed in a thread, or described by the user — a one-off name stays out, as in a vault.
async function loadBrainPeople() {
  try { const d = await get('api/brain/people'); d.at = Date.now(); S.br.people = d } catch { /* app restarting */ }
}
const brShown = (x) => x.touches >= 2 || (x.places || []).length > 0 || x.about_by === 'you'
/** The people and threads in the Brain's scope (the box lights matches; it doesn't remove anyone). */
function brPeopleIn() {
  const d = S.br.people
  if (!d || !d.people) return null
  const [k, v] = (S.br.scope || '').split(':')
  const inScope = (t) => !k ? true : k === 'thread' ? t === v : ((projById(t) || {}).client || '-') === v
  const people = d.people.filter((x) => brShown(x) && (x.threads || []).some(inScope))
  const threads = (d.threads || []).filter((t) => inScope(t.id) && people.some((x) => (x.threads || []).includes(t.id)))
  return { people, threads }
}
function brainPeople() {
  const d = S.br.people
  if (!d) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<p class="error">${esc(d.error)}</p>`
  const v = brPeopleIn()
  const hidden = d.people.filter((x) => !brShown(x)).length
  const box = `<input class="wp-in br-pq" id="br-pq" type="search" placeholder="Find someone — name, organisation, role" value="${esc(S.br.pq || '')}" aria-label="Find someone">`
  const sub = `${v.people.length} ${v.people.length === 1 ? 'person' : 'people'} · ${v.threads.length} thread${v.threads.length === 1 ? '' : 's'}${hidden ? ` · ${hidden} seen once, not shown` : ''}`
  const res = S.br.pqRes ? `<span class="set-sub br-pq-res">${esc(S.br.pqRes)}</span>` : ''
  if (!v.people.length) return `<div class="br-graph-bar">${box}<span class="set-sub">${sub}</span></div><p class="set-sub">Nobody yet: people appear as their threads are synced.</p>`
  return `<div class="br-graph-bar">${box}${res}<span class="set-sub">${sub}</span></div><div class="br-graph" id="br-people"></div><p class="set-foot">Threads labelled, people near theirs: a line is a role, calls (faint), reports to (dark) or calls together. Scroll to zoom, hover for someone's links, click for who they are.</p>`
}
function drawPeople() {
  const box = $('#br-people')
  const v = box && brPeopleIn()
  if (!box || !v) return
  const key = `people|${S.br.scope || ''}|${v.people.length}|${v.threads.length}|${(S.br.people || {}).at || ''}`
  const st = GraphView.draw(box, key, () => {
    const G = new graphology.Graph({ type: 'undirected' })
    const clients = [...new Set(v.threads.map((t) => (projById(t.id) || {}).client || '-'))]
    const tColor = (id) => { const c = (projById(id) || {}).client; return c ? CLIENT_COLORS[clients.indexOf(c) % CLIENT_COLORS.length] : '#8a8a95' }
    const orgs = [...new Set(v.people.map((x) => (x.org || {}).id || x.side))]
    const pColor = (x) => x.side === 'yours' ? '#8a8a95' : CLIENT_COLORS[(orgs.indexOf((x.org || {}).id || x.side) + 3) % CLIENT_COLORS.length]
    v.threads.forEach((t, i) => { const a = (2 * Math.PI * i) / Math.max(1, v.threads.length); G.addNode('t:' + t.id, { x: 100 * Math.cos(a), y: 100 * Math.sin(a), size: 9, color: tColor(t.id), label: t.name, pinLabel: true, kind: 'thread', thread: t.id }) })
    for (const x of v.people) {
      const h = ProjectArt.hash(x.key)
      const roles = (x.places || []).map((p) => p.role).filter(Boolean)
      G.addNode('p:' + x.key, { x: (h % 101) - 50, y: ((h >> 7) % 101) - 50, size: 3 + Math.min(7, Math.sqrt(x.touches || 1) * 1.3), color: pColor(x), label: x.name, kind: 'person', key: x.key,
        search: [x.email, x.title, x.org && x.org.name, x.group, ...roles].filter(Boolean).join(' ') })
    }
    const add = (a, b, attrs) => { if (G.hasNode(a) && G.hasNode(b) && a !== b && !G.hasEdge(a, b)) G.addEdge(a, b, attrs) }
    for (const x of v.people) {
      for (const t of x.threads || []) { const pl = (x.places || []).find((p) => p.thread === t); add('p:' + x.key, 't:' + t, { weight: pl ? 3 : 1, size: pl ? 1 : 0.5, color: pl ? 'rgba(130,130,145,0.45)' : 'rgba(130,130,145,0.15)' }) }
      if (x.reports_to) add('p:' + x.key, 'p:' + x.reports_to, { weight: 2, size: 1.2, color: 'rgba(91,52,214,0.55)' })
      for (const y of x.with || []) add('p:' + x.key, 'p:' + y.key, { weight: Math.min(4, y.calls || 1), size: 0.6, color: 'rgba(139,98,255,0.18)' })
    }
    return G
  }, { click: (node, a) => a.kind === 'thread' ? press({ pj: 'goto', id: a.thread }) : press({ pp: 'pick', key: a.key }) })
  if (!st) return
  GraphView.select('br-people', S.view === 'brain' && S.pj.person ? 'p:' + S.pj.person : null)
  if ((S.br.pq || '') !== (st.q || '')) { st.q = S.br.pq || ''; const hits = GraphView.find('br-people', S.br.pq); const n = GraphView.focus('br-people', hits); S.br.pqRes = S.br.pq ? (n ? `${n} found` : 'nobody matches') : '' }
}
document.addEventListener('input', (e) => {
  if (e.target.id === 'pp-about') { S.pj.aboutEdit = e.target.value; return }
  if (e.target.id === 'dict-q') {
    S.br.tq = e.target.value
    clearTimeout(S.br.tqT)
    S.br.tqT = setTimeout(() => { const at = e.target.selectionStart; render(); const i = $('#dict-q'); if (i) { i.focus(); i.setSelectionRange(at, at) } }, 150)
    return
  }
  if (e.target.id === 'br-gq') {
    const q = e.target.value
    clearTimeout(S.br.gqT)
    S.br.gqT = setTimeout(() => { graphSearch(q, false); const r = $('.br-gq-res'); if (r) r.textContent = S.br.gqRes; else if (S.br.gqRes) { const at = e.target.selectionStart; render(); const i = $('#br-gq'); if (i) { i.focus(); i.setSelectionRange(at, at) } } }, 150)
    return
  }
  if (e.target.id !== 'br-pq') return
  S.br.pq = e.target.value
  clearTimeout(S.br.pqT)
  S.br.pqT = setTimeout(() => { drawPeople(); const r = $('.br-pq-res'); if (r) r.textContent = S.br.pqRes; else { const at = e.target.selectionStart; render(); const i = $('#br-pq'); if (i) { i.focus(); i.setSelectionRange(at, at) } } }, 150)
})
document.addEventListener('keydown', (e) => {
  if (e.target.id === 'br-gq' && e.key === 'Enter') { e.preventDefault(); graphSearch(e.target.value, true) }
  if ((e.target.id === 'br-gq' || e.target.id === 'br-pq') && e.key === 'Escape') { e.target.value = ''; e.target.dispatchEvent(new Event('input', { bubbles: true })) }
})

/** A topic or term in the right panel: its description, where it comes up, the user's review
 *  (rename, merge, move, remove; a term's kind, variants, pins) and its latest sections. */
function brainNodePanel() {
  const d = S.br.data, id = S.br.node, tx = (d && d.taxonomy) || {}
  const x = (tx.topics || []).find((t) => t.id === id) || (tx.terms || []).find((t) => t.id === id)
  if (!x) return '<div class="wp-empty">Pick a topic or a term.</div>'
  const isTerm = !!x.name, name = x.name || x.label, layer = x.layer
  // A client's topic may merge into, or sit under, one of the practice's too (one tree).
  const same = (isTerm ? (tx.terms || []) : (tx.topics || [])).filter((t) => (t.layer === layer || (layer !== 'shared' && t.layer === 'shared')) && t.id !== id)
  const l1s = (tx.topics || []).filter((t) => (t.layer === layer || (layer !== 'shared' && t.layer === 'shared')) && t.level === 1 && t.id !== id)
  const lnm = (l) => l === 'shared' ? 'practice' : (((tx.layers || []).find((y) => y.id === l) || {}).name || l)
  const ed = S.br.edit
  const body = (o) => esc(JSON.stringify({ ...o, layer, [isTerm ? 'name' : 'label']: name, term: isTerm }))
  const actions = ed === 'rename' ? `<span class="tok-in"><input id="br-rename" value="${esc(name)}" aria-label="New name" autofocus></span><button class="btn primary" data-br-do="rename">Rename</button>`
    : ed === 'merge' ? `<select class="wp-in" id="br-into" aria-label="Merge into">${same.map((t) => `<option value="${esc(JSON.stringify([t.name || t.label, t.layer]))}">${esc(t.name || t.label)}${t.layer !== layer ? ` (${esc(lnm(t.layer))})` : ''}</option>`).join('')}</select><button class="btn primary" data-br-do="merge">Merge</button>`
    : ed === 'scope' ? `<select class="wp-in" id="br-scope-to" aria-label="Whose topic">${layer === 'shared' ? (tx.layers || []).filter((l) => l.id !== 'shared').map((l) => `<option value="${esc(l.id)}">Only ${esc(l.name || l.id)}</option>`).join('') : '<option value="shared">Your practice (every client)</option>'}</select><button class="btn primary" data-br-do="scope">Move</button>`
    : ed === 'define' ? `<div class="br-def"><textarea class="wp-in" id="br-desc" rows="3" aria-label="What it is" placeholder="${isTerm ? 'What it is' : "What belongs in it — and what doesn't"}">${esc(x.description || '')}</textarea><input class="wp-in" id="br-syn" value="${esc((x.synonyms || []).join(', '))}" placeholder="Other names, comma-separated" aria-label="Synonyms">${isTerm ? `<input class="wp-in" id="br-spell" value="${esc((x.variants || []).join(', '))}" placeholder="Spellings — how it is written or misheard" aria-label="Spellings"><select class="wp-in" id="br-group" aria-label="Group">${(tx.groups || []).map((g) => `<option value="${esc(g.id)}"${x.group === g.id ? ' selected' : ''}>${esc(g.label)}</option>`).join('')}</select>` : ''}<button class="btn primary" data-br-do="define">Save</button></div>`
    : ed === 'split' ? `<select class="wp-in" id="br-split" aria-label="Which name">${[...(x.synonyms || []), ...(x.variants || [])].map((n) => `<option>${esc(n)}</option>`).join('')}</select><button class="btn primary" data-br-do="split">Make it a term</button>`
    : ed === 'move' ? `<select class="wp-in" id="br-parent" aria-label="Move under"><option value="">(top level)</option>${l1s.map((t) => `<option${x.parent === t.label ? ' selected' : ''}>${esc(t.label)}</option>`).join('')}</select><button class="btn primary" data-br-do="move">Move</button>`
    : ed === 'remove' ? `<span class="set-sub">Remove it? Its tags go too.</span><button class="btn danger" data-br-op='${body({ op: 'remove' })}'>Remove</button>`
    : `<button class="btn" data-br-edit="rename">Rename</button>${same.length ? '<button class="btn" data-br-edit="merge">Merge into…</button>' : ''}<button class="btn" data-br-edit="define">Define…</button>${isTerm ? ([...(x.synonyms || []), ...(x.variants || [])].length ? '<button class="btn" data-br-edit="split">Split off…</button>' : '') : `<button class="btn" data-br-edit="move">Move under…</button><button class="btn" data-br-split="${esc(JSON.stringify([layer, name]))}" title="Make 2–6 subtopics of it from its own sections (Claude), then tag only those again">Split into subtopics</button>`}<button class="btn" data-br-edit="scope">${layer === 'shared' ? 'Only one client…' : 'Up to the practice…'}</button><button class="btn ghost danger" data-br-edit="remove">Remove</button>`
  const nd = S.br.nodeData && S.br.nodeData.id === id ? S.br.nodeData : null
  const secs = !nd ? '<div class="skeleton"><i></i><i></i></div>' : nd.sections.length ? nd.sections.map((s) => {
    const call = s.link && s.link.startsWith('echo://call/') ? s.link.slice(12).split('?')[0] : ''
    const t = s.link && s.link.includes('?t=') ? Number(s.link.split('?t=')[1]) : null
    return `<div class="br-sec"><button class="src" ${call ? `data-br-open="${esc(call)}"${t != null ? ` data-t="${t}"` : ''}` : extAttr(s.link)}>${kindMark(s)} ${esc(s.doc || s.source)}${s.date ? ` · ${esc(relDay(s.date))}` : ''}${t != null ? ` · ${clock(t)}` : ''}</button><div class="qt">${esc(s.text)}</div></div>`
  }).join('') : '<p class="set-sub">No sections in this scope.</p>'
  const layerName = layer === 'shared' ? 'Your practice' : ((tx.layers || []).find((l) => l.id === layer) || {}).name || layer
  return `<div class="wp br-panel">
    <div class="wp-top"><span class="wk-k">${isTerm ? esc(x.kind || 'term') : `topic · level ${x.level}`}</span><span class="set-sub">${esc(layerName)}</span></div>
    <h3 class="wp-name">${esc(name)}</h3>
    ${x.parent ? `<div class="set-sub">under ${esc(x.parent)}</div>` : ''}
    ${x.description ? `<p class="br-desc">${esc(x.description)}</p>` : ''}
    ${!isTerm && (x.synonyms || []).length ? `<div class="set-sub">Also called: ${esc(x.synonyms.join(', '))}</div>` : ''}
    ${isTerm && (x.synonyms || []).length ? `<div class="set-sub">Also called: ${esc(x.synonyms.join(', '))}</div>` : ''}
    ${isTerm && (x.variants || []).length ? `<div class="set-sub">Written or heard as: ${esc(x.variants.join(', '))}</div>` : ''}
    ${isTerm ? `<div class="set-sub">${esc(((tx.groups || []).find((g) => g.id === x.group) || {}).label || x.group)}${x.kind ? ` · ${esc(x.kind)}` : ''}${x.learned ? ' · learned from the minutes' : ''}</div>` : ''}
    ${(x.topics || []).length ? `<div class="br-where">Comes up in ${x.topics.map((t) => `<button class="link-btn" data-br-node="${esc(t.id)}">${esc(t.label)}</button>`).join(', ')}</div>` : ''}
    ${(x.terms || []).length ? `<div class="br-where">Its terms: ${x.terms.map((t) => `<button class="link-btn" data-br-node="${esc(t.id)}">${esc(t.label)}</button>`).join(', ')}</div>` : ''}
    <div class="br-where">${x.sections || 0} sections ${threadChips(x.threads)}</div>
    <div class="br-acts">${actions}</div>
    <div class="wp-h">Latest sections</div>${secs}
  </div>`
}
/** A source picked in the graph: its details in the right panel; opening it is the panel's Open. */
function selectBrainDoc(source) {
  S.br.doc = source; S.br.docData = null; S.br.node = null
  const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (eff.narrow) S.overlay = true
  Layout.show('brain-doc', known)
  layoutChanged()
  get(`api/brain/doc?source=${encodeURIComponent(source)}${brainQuery().replace('?', '&')}`).then((x) => { if (S.br.doc === source) { S.br.docData = x; render() } }).catch(() => {})
}
/** A Brain link, opened where it lives: a call in Echo (at its moment), a post or mail in Teams / Outlook, a file in its app. */
function openBrainLink(link) {
  if (link.startsWith('echo://call/')) {
    const [call, q] = link.slice(12).split('?')
    const t = q && q.startsWith('t=') ? Number(q.slice(2)) : null
    S.jump = t
    return go(`call/${call}/${t != null ? 'transcript' : 'minutes'}`)
  }
  if (link.startsWith('echo://file/')) return send({ cmd: 'open_brain_file', id: link.slice(12).split('?')[0] })
  if (/^https:\/\//i.test(link)) return send({ cmd: 'open_url', url: link })
}
const DOC_KIND = { call: 'call', channel: 'channel post', mail: 'mail', file: 'document' }
function brainDocPanel() {
  const x = S.br.docData
  if (!x || x.source !== S.br.doc) return '<div class="wp br-panel"><div class="skeleton"><i></i><i></i><i></i></div></div>'
  if (x.missing || x.error) return `<div class="wp-empty">${esc(x.error || 'This source is gone from the Brain.')}</div>`
  const openLabel = x.kind === 'call' ? 'Open the call' : x.kind === 'file' ? 'Open the document' : x.kind === 'mail' ? 'Open in Outlook' : 'Open in Teams'
  const open = x.link ? `<button class="btn primary" data-br-doc-open="${esc(x.link)}">${openLabel}</button>` : ''
  const threads = (x.threads || []).map((t) => t.id)
  const chip = (t) => `<button class="tok link" data-br-node="${esc(t.id)}" title="${t.sections} section${t.sections === 1 ? '' : 's'} here">${esc(t.label || t.id)}</button>`
  const terms = (x.topics || []).filter((t) => t.term).map(chip).join('')
  const topics = (x.topics || []).filter((t) => !t.term).map((t) => `<button class="tok link" data-br-node="${esc(t.id)}" title="${t.sections} section${t.sections === 1 ? '' : 's'} here">${esc(t.label || t.id)}</button>`).join('')
  const secs = (x.sections || []).map((s) => {
    const where = s.link && s.link.includes('?t=') ? clock(Number(s.link.split('?t=')[1])) : s.link && s.link.includes('page=') ? `page ${esc(s.link.split('page=')[1])}` : ''
    return `<div class="br-sec"><button class="src" ${s.link ? `data-br-doc-open="${esc(s.link)}"` : ''}>${esc(s.section || '')}${where && !String(s.section || '').includes(where) ? ` · ${where}` : ''}</button><div class="qt">${esc(s.text)}</div></div>`
  }).join('')
  const rel = (x.related || []).map((r) => `<button class="br-rel" data-graph-doc="${esc(r.source)}"><span class="br-kind">${kindMark(r)}</span><span class="br-rel-t">${esc(r.title || r.source)}</span><span class="set-sub">${r.date ? esc(relDay(r.date)) : ''}</span></button>`).join('')
  return `<div class="wp br-panel">
    <div class="wp-top"><span class="wk-k">${kindMark(x)} ${esc(DOC_KIND[x.kind] || x.kind || 'source')}</span><span class="set-sub">${x.date ? esc(relDay(x.date)) : ''}</span></div>
    <h3 class="wp-name">${esc(x.title || x.source)}</h3>
    ${x.rel ? `<div class="set-sub">${esc(x.rel)}${x.fidelity && x.fidelity !== 'full' ? ` · ${esc(x.fidelity)}` : ''}</div>` : ''}
    <div class="br-where">${x.sections_total} sections ${threadChips(threads.length ? threads : ['-'])}</div>
    <div class="br-acts">${open}</div>
    ${topics ? `<div class="wp-h">Topics</div><div class="br-topics">${topics}</div>` : ''}
    ${terms ? `<div class="wp-h">Terms</div><div class="br-topics">${terms}</div>` : ''}
    <div class="wp-h">Sections</div>${secs || '<p class="set-sub">No sections.</p>'}${x.sections_total > (x.sections || []).length ? `<p class="set-sub">${x.sections_total - x.sections.length} more — open it to read them all.</p>` : ''}
    ${rel ? `<div class="wp-h">Related sources</div><div class="br-rels">${rel}</div>` : ''}
  </div>`
}
PANEL_VIEWS.push({ id: 'brain-doc', label: 'Source', icon: ICON('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h4"/>'), area: 'right', when: () => S.view === 'brain' && !!S.br.doc, render: brainDocPanel })
PANEL_VIEWS.push({ id: 'brain-node', label: 'Topic', icon: ICON('<circle cx="12" cy="12" r="3"/><circle cx="5" cy="6" r="2"/><circle cx="19" cy="7" r="2"/><circle cx="7" cy="19" r="2"/><path d="M9.5 10.5 6.6 7.4M14.6 10.6l2.8-2.4M10.2 14.2l-2 3.3"/>'), area: 'right', when: () => S.view === 'brain' && !!S.br.node, render: brainNodePanel })

function brainEdit(body) {
  send({ cmd: 'brain_taxonomy', body })
  S.br.edit = null
  setTimeout(() => loadBrain().then(render), 350)
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-br-tab],[data-br-node],[data-br-accept],[data-br-fold],[data-br-rebuild],[data-br-split],[data-br-retag],[data-br-move],[data-dict-group],[data-br-snooze],[data-br-edit],[data-br-do],[data-br-op],[data-br-open],[data-graph-doc],[data-br-doc-open]')
  if (!t) return
  const d = t.dataset
  if (d.brTab) { S.br.tab = d.brTab; go(`brain/${d.brTab}`); return loadBrain().then(render) }
  if (d.brNode) {
    S.br.node = d.brNode; S.br.edit = null; S.br.nodeData = null; S.br.doc = null
    const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
    if (eff.narrow) S.overlay = true
    Layout.show('brain-node', known)
    layoutChanged()
    return loadBrainNode().then(render)
  }
  if (d.brAccept) return brainEdit({ op: 'accept', layer: d.brAccept })
  if (d.brFold) { const f = S.br.fold || {}; S.br.fold = { ...f, [d.brFold]: !f[d.brFold] }; return render() }
  if (d.brRebuild) { send({ cmd: 'brain_rebuild_topics', layer: d.brRebuild }); S.br.rebuilding = { ...(S.br.rebuilding || {}), [`taxonomy:${d.brRebuild}`]: true }; setTimeout(() => { S.br.rebuilding = {}; loadBrain().then(render) }, 1500); return render() }
  if (d.brSplit) { const [layer, label, asker] = JSON.parse(d.brSplit); send({ cmd: 'brain_split_topic', layer, label, for: asker || layer }); S.br.rebuilding = { ...(S.br.rebuilding || {}), [`split:${asker || layer}`]: true }; setTimeout(() => { S.br.rebuilding = {}; loadBrain().then(render) }, 1500); return render() }
  if (d.dictGroup !== undefined) { S.br.tg = d.dictGroup; return render() }
  if (d.brMove) { send({ cmd: 'brain_topic_move', key: d.brMove, do: d.do }); return setTimeout(() => loadBrain().then(render), 400) }
  if (d.brRetag) { send({ cmd: 'brain_retag_topics', layer: d.brRetag }); S.br.rebuilding = { ...(S.br.rebuilding || {}), [`retag:${d.brRetag}`]: true }; setTimeout(() => { S.br.rebuilding = {}; loadBrain().then(render) }, 1500); return render() }
  if (d.brSnooze) { send({ cmd: 'brain_topics_snooze', layer: d.brSnooze }); return setTimeout(() => loadBrain().then(render), 300) }
  if (d.brEdit) { S.br.edit = d.brEdit; render(); { const i = $('#br-rename'); if (i) { i.focus(); i.select() } } return }
  if (d.brOp) return brainEdit(JSON.parse(d.brOp))
  if (d.brDo) {
    const tx = (S.br.data || {}).taxonomy || {}
    const x = (tx.topics || []).find((y) => y.id === S.br.node) || (tx.terms || []).find((y) => y.id === S.br.node)
    if (!x) return
    const base = { layer: x.layer, term: !!x.name, [x.name ? 'name' : 'label']: x.name || x.label }
    if (d.brDo === 'rename') { const to = ($('#br-rename') || {}).value; if (to && to.trim()) { S.br.node = null; brainEdit({ ...base, op: 'rename', to: to.trim() }) } return }
    if (d.brDo === 'merge') { const [into, into_layer] = JSON.parse(($('#br-into') || {}).value || '[]'); S.br.node = null; return brainEdit({ ...base, op: 'merge', into, into_layer }) }
    if (d.brDo === 'scope') { S.br.node = null; return brainEdit({ ...base, op: 'scope', to: ($('#br-scope-to') || {}).value }) }
    if (d.brDo === 'define') {
      const list = (sel) => (($(sel) || {}).value || '').split(',').map((v) => v.trim()).filter(Boolean)
      const extra = base.term ? { variants: list('#br-spell'), group: ($('#br-group') || {}).value } : {}
      return brainEdit({ ...base, op: 'set', description: ($('#br-desc') || {}).value || '', synonyms: list('#br-syn'), ...extra })
    }
    if (d.brDo === 'split') return brainEdit({ ...base, op: 'split', alias: ($('#br-split') || {}).value })
    if (d.brDo === 'move') return brainEdit({ ...base, op: 'move', parent: ($('#br-parent') || {}).value || null })
  }
  if (d.brOpen) { S.jump = d.t ? Number(d.t) : null; return go(`call/${d.brOpen}/${d.t ? 'transcript' : 'minutes'}`) }
  if (d.graphDoc) return selectBrainDoc(d.graphDoc)
  if (d.brDocOpen) return openBrainLink(d.brDocOpen)
})
document.addEventListener('change', (e) => {
  const t = e.target
  if (t.matches && t.matches('[data-br-scope]')) { S.br.scope = t.value; S.br.node = null; S.br.data = null; render(); loadBrain().then(render) }
})
document.addEventListener('keydown', (e) => {
  if (e.target.id === 'br-rename' && e.key === 'Enter') { e.preventDefault(); const b = $('[data-br-do="rename"]'); if (b) b.click() }
  if (e.target.id === 'br-rename' && e.key === 'Escape') { S.br.edit = null; render() }
})
window.addEventListener('resize', () => { if (S.view === 'brain') GraphView.resize() })
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => { GraphView.retheme(); if (S.view === 'brain') render() })

/** A thread's topics and terms from the Brain (null when there is no taxonomy yet: the old glossary
 *  shows instead). Topics with their sections here; terms pinned here first, then by mentions. */
function threadBrain(p) {
  if (!brainOn()) return null
  const d = S.pj.tbrain
  if (!d || d.thread !== p.id || !d.taxonomy || !d.taxonomy.exists) return null
  const tx = d.taxonomy
  const topics = (tx.topics || []).filter((x) => x.sections).sort((a, b) => b.sections - a.sections).slice(0, 10)
  const seen = new Set()
  const terms = (tx.terms || []).filter((x) => x.sections || (x.pinned || []).includes(p.id))
    .sort((a, b) => ((b.pinned || []).includes(p.id) - (a.pinned || []).includes(p.id)) || ((b.layer !== 'shared') - (a.layer !== 'shared')) || (b.sections - a.sections))
    .filter((x) => { const k = x.name.toLowerCase(); if (seen.has(k)) return false; seen.add(k); return true }).slice(0, 30)
  const more = (tx.terms || []).filter((x) => x.sections || (x.pinned || []).includes(p.id)).length - terms.length
  return `<div class="tt-topics">${topics.map((x) => `<span class="chipt" title="${esc(x.description || '')}">${esc(x.label)} · ${x.sections}</span>`).join('') || '<span class="set-sub">No topics tagged here yet.</span>'}</div>
    <div class="tt-terms">${terms.map((x) => `<span class="tok" title="${esc([x.kind, x.description].filter(Boolean).join(' — '))}">${esc(x.name)}<small>${esc(x.kind)}${(x.pinned || []).includes(p.id) ? ' · pinned' : ''}</small></span>`).join('')}${addTok('bterm', 'A term for this thread — a system, an acronym, a name')}</div>
    <div class="pf-sub">From the Brain — ${esc(p.client_name || 'your practice')}'s layer and yours. Transcription spells these right on this thread's calls${more > 0 ? ` · ${more} more in <a href="#brain/terms">Brain → Terms</a>` : ''}.</div>`
}

/** Insights of a client or a thread (`brain/insights.rs`): what no single call shows, each citing two
 *  sources or more; to re-check when a cited source changed; Refresh runs one now (otherwise the app
 *  redoes a scope at most daily, after its calls changed). */
const insights = { key: null, data: null, at: 0 }
async function loadInsights(kind, id, force) {
  const k = `${kind}:${id}`
  if (!force && insights.key === k && Date.now() - insights.at < 20000) return
  insights.key = k; insights.at = Date.now()
  try { insights.data = await get(`api/brain/insights?${kind}=${encodeURIComponent(id)}`) } catch { insights.data = null }
  render()
}
const INSIGHT_KIND = { moved: ['Position moved', 'warn'], slipping: ['Keeps slipping', 'danger'], recurs: ['Comes back', 'info'], contradiction: ['Contradiction', 'accent'], open: ['Still open', 'mute'] }
function insightsBlock(kind, id, name) {
  if (!brainOn()) return ''
  const k = `${kind}:${id}`
  if (insights.key !== k) { loadInsights(kind, id); return '' }
  const d = insights.data
  if (!d || d.error) return ''
  const list = d.insights || []
  const when = d.at ? `checked ${esc(relDay(String(d.at).slice(0, 10)))}` : 'not run yet'
  const btn = d.running ? '<span class="set-sub">Reading…</span>' : `<button class="link-btn" data-insights-run="${esc(kind)}" data-id="${esc(id)}">${list.length ? 'Refresh' : 'Find insights'}</button>`
  const card = (x) => {
    const [label, cls] = INSIGHT_KIND[x.kind] || [x.kind, 'mute']
    const cites = x.citations.map((c) => {
      const call = c.link && c.link.startsWith('echo://call/') ? c.link.slice(12).split('?')[0] : ''
      const t = c.link && c.link.includes('?t=') ? Number(c.link.split('?t=')[1]) : null
      return `<button class="src" ${call ? `data-br-open="${esc(call)}"${t != null ? ` data-t="${t}"` : ''}` : extAttr(c.link)} title="“${esc(c.quote)}”">${kindMark(c)} ${esc(c.doc)}${c.date ? ` · ${esc(relDay(c.date))}` : ''}${t != null ? ` · ${clock(t)}` : ''}</button>`
    }).join('')
    return `<div class="ins${x.stale ? ' stale' : ''}"><span class="ins-k ${cls}">${esc(label)}${x.stale ? ' · to re-check' : ''}</span><div class="ins-t">${esc(x.text)}</div><div class="ins-c">${cites}</div></div>`
  }
  const empty = d.at ? '<p class="set-sub">Nothing across calls stands out yet.</p>' : `<p class="set-sub">Insights find what no single call shows — positions that moved, what keeps slipping or coming back, contradictions — each citing two sources. ${d.due ? 'Echo will look after its next quiet moment, or now:' : ''}</p>`
  return `<section class="set-sec ins-sec"><div class="ins-h"><h3>Insights</h3><span class="set-sub">across ${esc(name)}'s calls · each cites two sources or more · ${when}</span><span class="grow"></span>${btn}</div>${list.length ? `<div class="ins-grid">${list.map(card).join('')}</div>` : empty}</section>`
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-insights-run]')
  if (!b) return
  send({ cmd: 'brain_insights', kind: b.dataset.insightsRun, id: b.dataset.id })
  b.textContent = 'Reading…'; b.disabled = true
  const poll = (n) => setTimeout(() => loadInsights(b.dataset.insightsRun, b.dataset.id, true).then(() => { if (insights.data && insights.data.running && n < 60) poll(n + 1) }), 4000)
  poll(0)
})

/** "Mentioned also in" (the Brain): where else in the thread a work item comes up — near the words it
 *  came from, else about its title — leaving out the calls it already cites. */
const mentions = { key: null, data: null }
function mentionsBlock(n) {
  if (!brainOn() || !S.pj.sel || n.kind === 'epic') return ''
  const quotes = (n.history || []).filter((h) => h.quote && h.evidence).slice(0, 4).map((h) => [h.evidence, h.quote])
  const key = `${S.pj.sel}|${n.id}|${quotes.length}`
  if (mentions.key !== key) {
    mentions.key = key; mentions.data = null
    const q = `thread=${encodeURIComponent(S.pj.sel)}&title=${encodeURIComponent(n.title)}&quotes=${encodeURIComponent(JSON.stringify(quotes))}`
    get('api/brain/mentions?' + q).then((d) => { if (mentions.key === key) { mentions.data = d; render() } }).catch(() => { if (mentions.key === key) { mentions.data = { items: [] }; render() } })
    return ''
  }
  const items = (mentions.data && mentions.data.items) || []
  if (!items.length) return ''
  return `<div class="wp-h">Mentioned also in</div>${items.map((x) => {
    const call = x.link && x.link.startsWith('echo://call/') ? x.link.slice(12).split('?')[0] : ''
    const t = x.link && x.link.includes('?t=') ? Number(x.link.split('?t=')[1]) : null
    return `<div class="br-sec"><button class="src" ${call ? `data-br-open="${esc(call)}"${t != null ? ` data-t="${t}"` : ''}` : extAttr(x.link)}>${kindMark(x)} ${esc(x.doc || x.source)}${x.date ? ` · ${esc(relDay(x.date))}` : ''}${t != null ? ` · ${clock(t)}` : ''}</button><div class="qt">${esc(x.text)}</div></div>`
  }).join('')}`
}

// Claude about a thread or a client: the Echo-folder session (it has the Brain); the window tells it
// which client or thread "this" is (`<echo_view>`), so its searches are scoped.
document.addEventListener('click', (e) => {
  if (!(e.target.closest && e.target.closest('[data-claude-about]'))) return
  S.pj.menu = false
  claudePanel('claude-folder')
})

/** A call's projects in its header: chips, the suggestion bar, and the picker. */
function callProjects(c) {
  const chips = (c.projects || []).map((p) => `<button class="pj-chip" data-pj="goto" data-id="${esc(p.id)}" title="${esc(WHY[p.why] || p.why)}">${projIcon(p, 14)}${esc(p.name)}</button>`).join('')
  const sug = (c.suggested || [])[0]
  const bar = sug ? `<div class="pj-sug">${projIcon(sug, 16)}<span class="grow">Part of <b>${esc(sug.name)}</b>? <span class="set-sub">${esc(sug.why)}</span></span><button class="btn primary" data-pj="member" data-id="${esc(sug.id)}" data-call-name="${esc(c.name)}" data-how="include">Add</button><button class="btn ghost" data-pj="member" data-id="${esc(sug.id)}" data-call-name="${esc(c.name)}" data-how="exclude">Not this</button></div>` : ''
  const all = (S.pj.data && S.pj.data.projects) || []
  const mine = new Set((c.projects || []).map((p) => p.id))
  const picker = S.pj.picker ? `<div class="menu pj-menu" role="menu">${all.filter((p) => p.group !== 'closed').map((p) => `<button class="menu-item" role="menuitemcheckbox" aria-checked="${mine.has(p.id)}" data-pj="member" data-id="${esc(p.id)}" data-call-name="${esc(c.name)}" data-how="${mine.has(p.id) ? 'exclude' : 'include'}">${mine.has(p.id) ? '✓ ' : ''}${projIcon(p, 14)} ${esc(p.name)}</button>`).join('') || '<div class="menu-item set-sub">No threads yet</div>'}<div class="menu-sep" role="separator"></div><button class="menu-item" role="menuitem" data-pj="from-call" data-call-name="${esc(c.name)}">New thread from this call…</button></div>` : ''
  if (c.projects_error) return `<div class="pj-chips"><span class="set-sub">Projects: ${esc(c.projects_error)}</span></div>`
  return `<div class="pj-chips">${chips}<span class="pj-add-wrap"><button class="pj-chip add" data-pj="picker" aria-haspopup="menu" aria-expanded="${S.pj.picker}">+ Thread</button>${picker}</span></div>${bar}`
}

function projectSend(op, extra = {}) {
  send({ cmd: 'project', op, id: S.pj.sel, ...extra })
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-pj]')
  if (!b) {
    const inside = e.target.closest && (e.target.closest('.pj-add-wrap') || e.target.closest('.tok-in') || e.target.closest('.pf-v [autofocus]'))
    if ((S.pj.picker || S.pj.menu || S.pj.adding || S.pj.evPicker || S.pj.colMenu || S.pj.fieldsMenu || S.pj.viewMenu) && !inside) { S.pj.picker = false; S.pj.menu = false; S.pj.adding = null; S.pj.evPicker = null; S.pj.colMenu = null; S.pj.fieldsMenu = false; S.pj.viewMenu = false; render() }
    return
  }
  e.preventDefault()
  e.stopPropagation()
  const a = b.dataset.pj, d = b.dataset
  switch (a) {
    case 'open': return go(`project/${d.id}/${S.pj.tab}`)
    case 'xlink-open': S.pj.xlink = { node: d.node, q: '', results: [] }; return render()
    case 'xlink-close': S.pj.xlink = null; return render()
    case 'xlink': { const node = S.pj.xlink && S.pj.xlink.node; S.pj.xlink = null; return ledgerEdit({ op: 'xref', node, thread: d.thread, target: d.target, kind: d.kind, master: d.master || 'there' }) }
    case 'xunlink': return ledgerEdit({ op: 'unxref', node: d.node, thread: d.thread, target: d.target })
    case 'node': {
      S.pj.node = d.node || null
      S.pj.editField = null
      if (S.pj.node) { const known = panelViews(); const eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay); if (eff.narrow) S.overlay = true; Layout.show('work-item', known); return layoutChanged() }
      return render()
    }
    case 'open-only': return setView({ open: !wv().open })
    case 'view-pick': return chooseView(d.v)
    case 'view-menu': S.pj.viewMenu = !S.pj.viewMenu; render(); { const i = document.querySelector('[data-view-name]'); if (i) { i.focus(); i.select() } } return
    case 'view-new': case 'view-dup': {
      const bd = boardOf(), cur = wv()
      const id = 'v' + Date.now().toString(36)
      const base = a === 'view-dup' ? { ...bd.views.find((v) => v.id === cur.id), name: `${cur.name} copy` } : { name: `View ${bd.views.length + 1}`, layout: cur.layout }
      bd.views = [...bd.views, { ...base, id }]
      saveBoard(bd)
      chooseView(id)
      S.pj.viewMenu = true
      render(); { const i = document.querySelector('[data-view-name]'); if (i) { i.focus(); i.select() } } return
    }
    case 'view-del': { const bd = boardOf(); const cur = activeViewId(); bd.views = bd.views.filter((v) => v.id !== cur); S.pj.viewMenu = false; saveBoard(bd); return chooseView(bd.views[0].id) }
    case 'edit-field': S.pj.editField = d.field; render(); { const i = document.querySelector('.wp [autofocus]'); if (i) { i.focus(); if (i.showPicker && i.type === 'date') try { i.showPicker() } catch {} } } return
    case 'edit-title': S.pj.editField = 'title'; render(); { const i = document.querySelector('.wp-title'); if (i) { i.focus(); i.select() } } return
    case 'sync': send({ cmd: 'project', op: 'sync', id: S.pj.sel }); if (S.pj.work) S.pj.work.sync = { state: 'running', message: 'reading calls' }; return render()
    case 'ledger': return ledgerEdit(JSON.parse(d.body))
    case 'ledger-state': return S.pj.node && ledgerEdit({ op: 'edit', node: S.pj.node, set: { state: d.state } })
    case 'found': return go('found')
    case 'fields-menu': S.pj.fieldsMenu = !S.pj.fieldsMenu; return render()
    case 'col-menu': S.pj.colMenu = S.pj.colMenu === d.col ? null : d.col; render(); { const i = $('#bd-col-name'); if (i) i.focus() } return
    case 'col-set': {
      if (d.k === 'hidden') { S.pj.colMenu = null; return setView({ hidden: [...new Set([...wv().hidden, d.col])] }) }
      const bd = boardOf(); bd.columns = bd.columns.map((c) => c.id === d.col ? { ...c, [d.k]: d.v } : c); return saveBoard(bd)
    }
    case 'col-move': { const bd = boardOf(); const vis = bd.columns; const i = vis.findIndex((c) => c.id === d.col), j = i + Number(d.d); if (i < 0 || j < 0 || j >= vis.length) return; const cs = vis.slice(); [cs[i], cs[j]] = [cs[j], cs[i]]; bd.columns = cs; return saveBoard(bd) }
    case 'col-delete': { const bd = boardOf(); const c = bd.columns.find((x) => x.id === d.col); if (!c) return; if (c.state !== 'waiting' && !bd.columns.some((x) => x.id !== c.id && x.state === c.state && !x.hidden)) return window.echo.notice(`Keep a column for “${(STATE_CATS.find(([k]) => k === c.state) || [])[1] || c.state}” — its cards need somewhere to go.`); bd.columns = bd.columns.filter((x) => x.id !== c.id); S.pj.colMenu = null; return saveBoard(bd) }
    case 'col-add': { const bd = boardOf(); const id = 'c' + Date.now().toString(36); bd.columns = [...bd.columns, { id, name: 'New column', color: COL_COLORS[bd.columns.length % COL_COLORS.length], state: 'open' }]; S.pj.colMenu = id; saveBoard(bd); { const i = $('#bd-col-name'); if (i) { i.focus(); i.select() } } return }
    case 'col-unhide': { const bd = boardOf(); bd.columns = bd.columns.map((c) => ({ ...c, hidden: false })); bd.views = bd.views.map((v) => v.id === activeViewId() ? { ...v, hidden: [] } : v); return saveBoard(bd) }
    case 'ev-picker': S.pj.evPicker = S.pj.evPicker === d.event ? null : d.event; return render()
    case 'event-member': S.pj.evPicker = null; send({ cmd: 'project', op: 'event_member', id: d.id, event: d.event, how: d.how }); setTimeout(() => loadCalendar(true).then(render), 300); return render()
    case 'event-new': S.pj.evPicker = null; S.pj.fromEvent = { id: d.event, subject: d.subject }; send({ cmd: 'project', op: 'create', name: d.subject || 'New thread' }); return render()
    case 'client': S.pj.menu = false; S.pj.confirmDelete = false; return go(`client/${d.id}/${d.ctab || 'overview'}`)
    case 'ctab': return go(`client/${S.pj.client}/${d.tabId}`)
    case 'menu': S.pj.menu = !S.pj.menu; return render()
    case 'adding': S.pj.adding = d.what; render(); { const i = $(`#pj-add-${d.what}`); if (i) i.focus() } return
    case 'pf-edit': S.pj.pfEdit = d.what; render(); { const i = document.querySelector('.pf-v [autofocus]'); if (i) { i.focus(); if (i.select) try { i.select() } catch {} } } return
    case 'client-internal': return send({ cmd: 'project', op: 'client_set', id: S.pj.client, internal: d.on === 'true' })
    case 'client-list-remove': { const c = ((S.pj.data && S.pj.data.clients) || []).find((x) => x.id === S.pj.client); if (!c) return; return send({ cmd: 'project', op: 'client_set', id: c.id, [d.list]: (c[d.list] || []).filter((x) => x !== d.value) }) }
    case 'client-remove': S.pj.confirmDelete = false; send({ cmd: 'project', op: 'client_remove', id: d.id }); return go('projects')
    case 'new': go('found'); setTimeout(() => { const i = $('#pj-new'); if (i) i.focus() }, 60); return
    case 'create': { const i = $('#pj-new'); if (!i || !i.value.trim()) return i && i.focus(); send({ cmd: 'project', op: 'create', name: i.value.trim(), client: $('#pj-new-client').value }); i.value = ''; return }
    case 'found-accept': {
      const card = b.closest('.found-card')
      send({ cmd: 'project', op: 'found_accept', key: card.dataset.key, name: card.querySelector('.found-name').value, client: card.querySelector('.found-client').value })
      return
    }
    case 'found-reject': { const card = b.closest('.found-card'); send({ cmd: 'project', op: 'found_reject', key: card.dataset.key }); card.remove(); return }
    case 'goto': return go(`project/${d.id}/brief`)
    case 'goto-node': { S.pj.node = d.node; S.pj.wantNode = d.node; S.pj.inline = null; go(`project/${d.id}/work`); Layout.show('work-item', panelViews()); return layoutChanged() }
    case 'tab': return go(`project/${S.pj.sel}/${d.tabId}`)
    case 'toggle-closed': S.pj.showClosed = !S.pj.showClosed; return render()
    case 'fold': {
      pjFold(d.id); render()
      // The sidebar was drawn again: the keyboard stays on this chevron.
      const again = [...document.querySelectorAll('.pj-chev[data-pj="fold"]')].find((x) => x.dataset.id === d.id)
      if (again) again.focus()
      return
    }
    case 'stage': return projectSend('stage', { stage: d.stage })
    case 'icon': return projectSend('set', { field: 'icon', value: d.icon })
    case 'color': return projectSend('set', { field: 'color', value: d.color })
    case 'logo-remove': return projectSend('remove_logo')
    case 'client-logo-remove': return send({ cmd: 'project', op: 'client_logo', id: d.id })
    case 'rule-remove': return projectSend('rule_remove', { index: Number(d.index) })
    case 'source-add': projectSend('source_add', { kind: d.kind, source: d.source, name: d.name || undefined }); setTimeout(() => loadSourceState(true), 1500); return
    case 'source-remove': projectSend('source_remove', { kind: d.kind, source: d.source }); setTimeout(() => loadSourceState(true), 1500); return
    case 'source-folder': send({ cmd: 'source_folder_pick', id: S.pj.sel }); setTimeout(() => loadSourceState(true), 4000); return
    case 'rule-add': {
      const kind = $('#pj-rule-kind').value, val = $('#pj-rule-val').value.trim()
      const rule = kind === 'series' ? { kind, subject: val } : kind === 'people' ? { kind, domains: val ? val.split(/[ ,]+/).filter(Boolean) : [] } : kind === 'chat' ? { kind, id: val } : { kind, contains: val }
      if (kind !== 'people' && !val) return $('#pj-rule-val').focus()
      return projectSend('rule_add', { rule })
    }
    case 'term-add': { const t = $('#pj-term').value.trim(); if (t) projectSend('term_add', { term: t }); return }
    case 'term-remove': return projectSend('term_remove', { term: d.term })
    case 'person-add': { const name = $('#pj-person-name').value.trim(); if (!name) return $('#pj-person-name').focus(); return projectSend('person_add', { name, email: $('#pj-person-email').value.trim(), role: $('#pj-person-role').value.trim() }) }
    case 'person-remove': return projectSend('person_remove', { name: d.name, email: d.email })
    case 'brief-edit': S.pj.briefEdit = (S.pj.client ? S.pj.cdata && S.pj.cdata.brief : S.pj.detail && S.pj.detail.brief) || ''; render(); { const t = $('#pj-brief'); if (t) t.focus() } return
    case 'brief-cancel': S.pj.briefEdit = null; return render()
    case 'desc-edit': {
      const a = e.target.closest && e.target.closest('a[href]') // a link in it opens (https only), it doesn't edit
      if (a) { if (/^https:\/\//i.test(a.getAttribute('href'))) send({ cmd: 'open_url', url: a.getAttribute('href') }); return }
      const x = S.pj.work && S.pj.work.nodes && S.pj.work.nodes[S.pj.node]
      if (!x) return
      S.pj.descEdit = { node: x.id, text: x.description || '' }
      render()
      const t = $('#wp-desc'); if (t) { t.focus(); t.setSelectionRange(t.value.length, t.value.length) }
      return
    }
    case 'desc-cancel': S.pj.descEdit = null; return render()
    case 'desc-save': return saveDescription()
    case 'brief-save': return projectSaveBrief()
    case 'client-add': { const name = $('#pj-client-name').value.trim(); if (!name) return $('#pj-client-name').focus(); S.pj.newClient = false; send({ cmd: 'project', op: 'client_add', name, domains: $('#pj-client-domains').value }); S.pj.pendingClient = name; return render() }
    case 'client-cancel': S.pj.newClient = false; return render()
    case 'org-new': { const name = $('#pj-org-name').value.trim(); if (!name) return $('#pj-org-name').focus(); S.pj.newOrg = false; return projectSend('org_new', { name, domains: $('#pj-org-domains').value }) }
    case 'org-new-cancel': S.pj.newOrg = false; return render()
    case 'org-unlink': return projectSend('org_link', { org: d.org, on: false })
    case 'org-link': return projectSend('org_link', { org: d.org, on: true })
    case 'delete-ask': S.pj.menu = false; S.pj.confirmDelete = true; return render()
    case 'delete-cancel': S.pj.confirmDelete = false; return render()
    case 'delete': { const id = S.pj.sel; S.pj.confirmDelete = false; S.pj.sel = null; S.pj.detail = null; send({ cmd: 'project', op: 'delete', id }); return go('projects') }
    case 'picker': S.pj.picker = !S.pj.picker; return render()
    case 'member': S.pj.picker = false; send({ cmd: 'project', op: 'member', id: d.id || S.pj.sel, call: d.callName, how: d.how }); return render()
    case 'from-call': {
      S.pj.picker = false
      const c = S.call && S.call.name === d.callName ? S.call : null
      send({ cmd: 'project', op: 'create', name: (c && c.title) || 'New thread' })
      S.pj.fromCall = d.callName
      return render()
    }
  }
}, true)
document.addEventListener('change', (e) => {
  const el = e.target
  if (el.dataset && el.dataset.logoOp && el.files) { sendLogo(el.files[0], el.dataset.logoOp, el.dataset.id); el.value = ''; return }
  if (el.dataset && el.dataset.pjFilter) return setView({ [el.dataset.pjFilter]: el.value })
  if (el.dataset && el.dataset.ledgerSet && S.pj.node) {
    const k = el.dataset.ledgerSet, v = el.value.trim()
    S.pj.editField = null
    return ledgerEdit({ op: 'edit', node: S.pj.node, set: { [k]: v === '' && k !== 'title' ? null : v } })
  }
  if (el.dataset && el.dataset.ledgerAc && S.pj.node) return ledgerEdit({ op: 'ac_set', node: S.pj.node, ac: Number(el.dataset.ledgerAc), met: el.checked })
  if (el.dataset && el.dataset.boardGroup !== undefined) return setView({ group: el.value })
  if (el.dataset && el.dataset.boardField) { const f = wv().fields; return setView({ fields: el.checked ? [...new Set([...f, el.dataset.boardField])] : f.filter((x) => x !== el.dataset.boardField) }) }
  if (el.dataset && el.dataset.viewLayout !== undefined) return setView({ layout: el.value })
  if (el.dataset && el.dataset.viewName !== undefined) { const v = el.value.trim(); if (v) setView({ name: v }); return }
  if (el.dataset && el.dataset.colState) { const bd = boardOf(); const next = bd.columns.map((c) => c.id === el.dataset.colState ? { ...c, state: el.value } : c); if (['open', 'in_progress', 'blocked', 'done'].some((k) => !next.some((c) => c.state === k && !c.hidden))) { render(); return window.echo.notice('Every state needs a visible column.') } bd.columns = next; return saveBoard(bd) }
  if (el.dataset && el.dataset.colLimit) { const bd = boardOf(); const v = parseInt(el.value, 10); bd.columns = bd.columns.map((c) => c.id === el.dataset.colLimit ? { ...c, limit: v > 0 ? v : undefined } : c); return saveBoard(bd) }
  if (el.dataset && el.dataset.colName) { const bd = boardOf(); const v = el.value.trim(); if (!v) return; bd.columns = bd.columns.map((c) => c.id === el.dataset.colName ? { ...c, name: v } : c); return saveBoard(bd) }
  if (el.dataset && el.dataset.evSeries !== undefined && el.value) {
    S.pj.evPicker = null
    send({ cmd: 'project', op: 'rule_add', id: el.value, rule: { kind: 'series', subject: el.dataset.evSeries } })
    setTimeout(() => loadCalendar(true).then(render), 300)
    return render()
  }
  if (el.classList && el.classList.contains('found-merge') && el.value) {
    send({ cmd: 'project', op: 'found_merge', key: el.closest('.found-card').dataset.key, into: el.value })
    return
  }
  if (el.dataset && el.dataset.clientField) { S.pj.pfEdit = null; return send({ cmd: 'project', op: 'client_set', id: el.dataset.id, [el.dataset.clientField]: el.value.trim() }) }
  if (el.id === 'pj-org-pick') {
    S.pj.adding = null
    if (el.value === '__new') { S.pj.newOrg = true; return render() }
    if (el.value) projectSend('org_link', { org: el.value, on: true })
    return render()
  }
  if (!el.dataset || !el.dataset.pjField) return
  S.pj.pfEdit = null
  if (el.dataset.pjField === 'client' && el.value === '__new') { S.pj.newClient = true; return render() }
  send({ cmd: 'project', op: 'set', id: el.dataset.id, field: el.dataset.pjField, value: el.value })
})
document.addEventListener('keydown', (e) => {
  const el = e.target
  if (el.id === 'pj-new' && e.key === 'Enter' && el.value.trim()) { e.preventDefault(); return $('[data-pj="create"]').click() }
  if (el.dataset && el.dataset.add && (e.key === 'Enter' || e.key === 'Escape')) {
    e.preventDefault()
    const what = el.dataset.add, v = el.value.trim()
    S.pj.adding = null
    if (e.key === 'Escape') return render()
    const c = ((S.pj.data && S.pj.data.clients) || []).find((x) => x.id === S.pj.client)
    if (what === 'term' && v) projectSend('term_add', { term: v })
    if (what === 'mail' && v) { projectSend('source_add', { kind: 'mail', source: v }); setTimeout(() => loadSourceState(true), 1500) }
    if (what === 'exclude') { projectSend('source_exclude', { source: el.dataset.root, exclude: v.split(',').map((g) => g.trim()).filter(Boolean) }); setTimeout(() => loadSourceState(true), 1500) }
    // A term for this thread in the Brain (its client's layer, pinned to it): the taxonomy supersedes the glossary.
    if (what === 'bterm' && v && S.pj.sel) {
      const p = S.pj.detail || {}
      send({ cmd: 'brain_taxonomy', body: { op: 'add', term: true, name: v, layer: p.client || 'shared', threads: [S.pj.sel] } })
      setTimeout(() => loadThreadBrain().then(render), 400)
    }
    if (what === 'rule') {
      const kind = ($('#pj-add-kind') || {}).value || 'series'
      if (!v && kind !== 'people') return render()
      projectSend('rule_add', { rule: kind === 'series' ? { kind, subject: v } : kind === 'people' ? { kind, domains: v ? v.split(/[ ,]+/).filter(Boolean) : [] } : kind === 'chat' ? { kind, id: v } : { kind, contains: v } })
    }
    if (what === 'person' && v) { const [name, email, role] = v.split(',').map((x) => x.trim()); projectSend('person_add', { name, email: email || '', role: role || '' }) }
    if (what === 'alias' && v && c) send({ cmd: 'project', op: 'client_set', id: c.id, aliases: [...(c.aliases || []), v] })
    if (what === 'domain' && v && c) send({ cmd: 'project', op: 'client_set', id: c.id, domains: [...(c.domains || []), ...v.split(/[ ,]+/).filter(Boolean)] })
    if (what === 'cproject' && v && c) send({ cmd: 'project', op: 'create', name: v, client: c.id })
    return render()
  }
  if (el.dataset && el.dataset.clientField && e.key === 'Enter') { e.preventDefault(); return el.blur() }
  if (el.dataset && (el.dataset.colName || el.dataset.colLimit) && (e.key === 'Enter' || e.key === 'Escape')) { e.preventDefault(); el.blur(); S.pj.colMenu = null; return render() }
  if (el.dataset && el.dataset.viewName !== undefined && (e.key === 'Enter' || e.key === 'Escape')) { e.preventDefault(); el.blur(); S.pj.viewMenu = false; return render() }
  if (el.id === 'pj-term' && e.key === 'Enter') { e.preventDefault(); return $('[data-pj="term-add"]').click() }
  if (el.id === 'pj-rule-val' && e.key === 'Enter') { e.preventDefault(); return $('[data-pj="rule-add"]').click() }
  if (el.id === 'pj-f-name' && e.key === 'Enter') { e.preventDefault(); return el.blur() }
  if (el.classList && el.classList.contains('wp-title') && e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); return el.blur() }
  if (el.id === 'wp-desc') {
    if (e.key === 'Escape') { e.preventDefault(); S.pj.descEdit = null; return render() }
    if ((MAC ? e.metaKey : e.ctrlKey) && e.key.toLowerCase() === 's') { e.preventDefault(); return saveDescription() }
  }
  if (el.id === 'pj-brief') {
    if (e.key === 'Escape') { e.preventDefault(); S.pj.briefEdit = null; return render() }
    if ((MAC ? e.metaKey : e.ctrlKey) && e.key.toLowerCase() === 's') { e.preventDefault(); return projectSaveBrief() }
  }
})
document.addEventListener('input', (e) => {
  if (e.target.id === 'pj-brief') S.pj.briefEdit = e.target.value
  if (e.target.id === 'wp-desc' && S.pj.descEdit) S.pj.descEdit.text = e.target.value
  if (e.target.id === 'pj-rule-val' || e.target.id === 'pj-rule-kind') return
})
document.addEventListener('change', (e) => {
  if (e.target.id === 'pj-rule-kind') {
    const ph = { series: "A meeting's subject", people: 'Domains (empty: the client\'s)', subject: 'Words of the subject', chat: '19:…@thread.v2' }
    const v = $('#pj-rule-val'); if (v) v.placeholder = ph[e.target.value] || ''
  }
})
// A work item's field opened for a change and left as it was (or Esc): back to reading it.
document.addEventListener('focusout', (e) => {
  if (!(e.target.closest && e.target.closest('.wp') && e.target.dataset.ledgerSet)) return
  setTimeout(() => { if (S.pj.editField && !(document.activeElement && document.activeElement.closest && document.activeElement.closest('.wp') && document.activeElement.dataset.ledgerSet)) { S.pj.editField = null; render() } }, 150)
})
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && S.pj.editField && e.target.closest && e.target.closest('.wp')) { e.preventDefault(); S.pj.editField = null; render() }
})
// An image dropped on a logo tile.
document.addEventListener('dragover', (e) => { const t = e.target.closest && e.target.closest('.pj-logo-drop'); if (t) { e.preventDefault(); t.classList.add('over') } })
document.addEventListener('dragleave', (e) => { const t = e.target.closest && e.target.closest('.pj-logo-drop'); if (t) t.classList.remove('over') })
document.addEventListener('drop', (e) => {
  const t = e.target.closest && e.target.closest('.pj-logo-drop')
  if (!t) return
  e.preventDefault()
  t.classList.remove('over')
  sendLogo(e.dataTransfer && e.dataTransfer.files[0], t.dataset.dropOp, t.dataset.id)
})
/** A work item's description, as typed (empty: none). Shown at once; the ledger pins it as yours. */
function saveDescription() {
  const de = S.pj.descEdit
  if (!de) return
  const text = de.text.trim(), x = S.pj.work && S.pj.work.nodes && S.pj.work.nodes[de.node]
  S.pj.descEdit = null
  if (x && (x.description || '') !== text) {
    x.description = text || null; x.description_html = null
    ledgerEdit({ op: 'edit', node: de.node, set: { description: text || null } })
  }
  render()
}
function projectSaveBrief() {
  const text = S.pj.briefEdit
  if (S.pj.client) {
    send({ cmd: 'project', op: 'client_brief', id: S.pj.client, text })
    if (S.pj.cdata) S.pj.cdata.brief = text
    S.pj.briefEdit = null
    render()
    return setTimeout(() => loadProject().then(render), 300)
  }
  projectSend('brief', { text })
  if (S.pj.detail) S.pj.detail.brief = text
  S.pj.briefEdit = null
  render()
}

// ── ⌘K palette: actions in context. Its actions are the window's own — each runs as the click on
// the button that does it elsewhere (`press`), so a palette action never differs from the button.
// The target is the open call, the selected calendar event, or nothing (Echo's own actions). ───
const PAL = { open: false, query: '', sel: 0, target: null, items: [] }
/** Run a window action the way its button does: a click on an element with these `data-` values. */
function press(data) {
  const b = document.createElement('button')
  for (const [k, v] of Object.entries(data)) b.dataset[k] = v
  b.hidden = true
  document.body.appendChild(b)
  b.click()
  b.remove()
}
/** What the palette acts on now. */
function paletteTarget() {
  if (S.view === 'calendar' && S.cal.selected && S.cal.data) {
    const e = (S.cal.data.events || []).find((x) => x.id === S.cal.selected)
    if (e) return { kind: 'event', e, label: e.subject || 'Event' }
  }
  if (S.view === 'calls' && S.selected && S.call && !S.call.error) return { kind: 'call', c: S.call, label: S.call.title }
  return null
}
/** Every action for the target, Echo's own, and where to go. `disabled`: why it can't run now. */
function paletteItems(target) {
  const items = []
  const add = (group, label, run, extra = {}) => items.push({ group, label, run, ...extra })
  const st = S.state || {}
  if (target && target.kind === 'call') {
    const c = target.c, busy = c.notes_run && (c.notes_run.state === 'queued' || c.notes_run.state === 'running')
    const transcribed = !['recording', 'transcribing'].includes(c.status)
    for (const [id, label] of callTabs({ recording: c.status === 'recording', chat: (c.chat || []).length, frames: (c.frames || []).length, actions: Array.isArray(c.actions) ? c.actions.length : null })) {
      add('This call', `Open ${label}`, () => press({ tab: id }), { keywords: 'tab show' })
    }
    add('This call', c.notes ? 'Write minutes again' : 'Write minutes', () => press({ cmd: 'write-notes' }), { keywords: 'notes update claude', disabled: !transcribed ? 'after the transcript is ready' : busy ? 'minutes are being written' : null })
    add('This call', 'Ask Claude about this call', () => press({ cmd: 'ask-claude' }), { keywords: 'question terminal session' })
    add('This call', 'Add a note', () => { Layout.show('notes', panelViews()); layoutChanged(); setTimeout(() => { const el = $('#note-input'); if (el) el.focus() }, 50) }, { keywords: 'my notes comment' })
    add('This call', 'Show the call folder', () => press({ cmd: 'reveal' }), { keywords: 'finder files' })
    add('This call', 'Open minutes in browser', () => press({ cmd: 'open-minutes' }), { disabled: c.notes ? null : 'no minutes yet' })
    add('This call', 'Delete this call…', () => press({ cmd: 'ask-delete' }), { keywords: 'remove trash' })
  }
  if (target && target.kind === 'event') {
    const e = target.e, es = eventState(e, new Date()), kind = eventAction(e, es)
    if (kind === 'open-notes') add('This event', 'Open its notes', () => press({ call: e.recorded.name }))
    if (kind === 'scheduled') add('This event', "Don't record it", () => press({ unschedule: e.id }))
    if (kind === 'join') add('This event', 'Join and record', () => press({ join: e.id }))
    if (kind === 'join' || kind === 'record') add('This event', es.live ? 'Record it' : 'Record when it starts', () => press({ schedule: e.id }))
    if (kind === 'import') add('This event', 'Import Teams transcript', () => press({ calImport: e.id }), { keywords: 'missed unrecorded transcript teams', disabled: e.import && e.import.busy ? 'importing now' : null })
    if (!kind) add('This event', 'Record it', () => {}, { disabled: e.cancelled ? 'cancelled' : e.declined ? 'you declined it' : es.past ? 'it is over' : !e.online ? 'not a Teams meeting' : 'not possible' })
  }
  // Echo's own
  if (st.recording) add('Echo', 'Stop recording…', () => press({ cmd: 'stop-ask' }), { keywords: 'end', hint: MAC ? '⇧⌘R' : 'Ctrl+Shift+R' })
  else if (st.armed) add('Echo', 'Cancel recording the next call', () => press({ cmd: 'stop' }), { keywords: 'disarm' })
  else add('Echo', st.in_call ? 'Record this call' : 'Record the next Teams call', () => press({ cmd: 'record' }), { keywords: 'start arm' })
  add('Echo', 'Claude · Echo folder', () => claudePanel('claude-folder'), { keywords: 'terminal session ask' })
  add('Echo', 'Vocabulary', () => press({ panelShow: 'vocabulary' }), { keywords: 'terms names words dictionary transcription learned glossary' })
  add('Echo', 'Show or hide Inbox', () => press({ panelFlip: 'needs' }), { keywords: 'needs you notifications attention decide review', hint: NY_HINT })
  for (const i of nyOpen().slice(0, 8)) add('Inbox', i.title, nyShow, { keywords: i.why })
  add('Echo', 'Show or hide My notes', () => press({ panelFlip: 'notes' }), { disabled: S.view === 'calls' && S.selected ? null : 'open a call first' })
  add('Echo', 'Show or hide the bottom panel', () => { Layout.toggle('bottom'); layoutChanged() })
  add('Echo', 'Check for updates', () => press({ act: 'check_updates' }))
  add('Echo', 'Ask for permissions', () => press({ act: 'ask_permissions' }), { keywords: 'microphone screen accessibility health' })
  // Go to
  for (const a of shownActivities()) add('Go to', a.label, () => goActivity(a.id))
  for (const p of (S.pj.data && S.pj.data.projects) || []) {
    const st = ProjectArt.stage(p.stage)
    add('Threads', p.name, () => go(`project/${p.id}/brief`), { keywords: `thread project ${p.client_name || ''} ${st.label} ${st.group}`, hint: st.label })
  }
  for (const x of Plugins.palette) add(x.group || 'Plugins', x.label, () => Promise.resolve().then(() => x.run(target)).catch((err) => { S.notice = `${x.label}: ${(err && err.message) || err}`; render() }), { keywords: x.keywords || '' })
  for (const [id, label] of SETTINGS_SECTIONS) add('Go to', `Settings: ${label}`, () => go(`settings/${id}`), { keywords: 'preferences' })
  for (const c of S.calls || []) {
    const d = c.started_at ? new Date(c.started_at) : null
    add('Calls', c.title, () => go(`call/${c.name}/${S.tab}`), { keywords: `${(c.people || []).join(' ')} ${d ? dayLabel(d) : ''}`, hint: d ? dayLabel(d) : '' })
  }
  return items
}
function paletteOpen() {
  PAL.open = true; PAL.query = ''; PAL.sel = 0; PAL.target = paletteTarget()
  let el = $('#palette')
  if (!el) { el = document.createElement('div'); el.id = 'palette'; document.body.appendChild(el) }
  el.innerHTML = `<div class="pal-back" data-pal-close></div><div class="pal" role="dialog" aria-label="Actions">
    <div class="pal-head">${ICON('<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>')}<input id="pal-q" autocomplete="off" spellcheck="false" placeholder="Type an action, a call, a person…" aria-label="Action"><span id="pal-target"></span></div>
    <div class="pal-list" id="pal-list" role="listbox"></div>
    <div class="pal-foot"><span><kbd>↵</kbd> run</span><span><kbd>↑</kbd><kbd>↓</kbd> choose</span><span><kbd>esc</kbd> close</span>${PAL.target ? '<span><kbd>⌫</kbd> on an empty box: drop the target</span>' : ''}</div></div>`
  paletteDraw()
  $('#pal-q').focus()
}
function paletteClose() { PAL.open = false; const el = $('#palette'); if (el) el.remove() }
function paletteDraw() {
  const t = PAL.target
  $('#pal-target').innerHTML = t ? `<button class="pal-chip" data-pal-untarget title="Act on Echo itself instead">On: ${esc(t.label.length > 32 ? t.label.slice(0, 31) + '…' : t.label)} ×</button>` : ''
  const all = paletteItems(t)
  // Calls show only when something is typed (the list would be every call).
  PAL.items = Palette.grouped(Palette.rank(PAL.query.trim() ? all : all.filter((x) => x.group !== 'Calls'), PAL.query))
  if (PAL.sel >= PAL.items.length) PAL.sel = Math.max(0, PAL.items.length - 1)
  let group = null
  $('#pal-list').innerHTML = PAL.items.map((it, i) => {
    const head = it.group !== group ? `<div class="pal-group">${esc(it.group)}</div>` : ''
    group = it.group
    return `${head}<button class="pal-item${i === PAL.sel ? ' on' : ''}${it.disabled ? ' off' : ''}" role="option" aria-selected="${i === PAL.sel}" aria-disabled="${!!it.disabled}" data-pal-i="${i}"><span class="pal-label">${esc(it.label)}</span><span class="pal-hint">${esc(it.disabled ? it.disabled : it.hint || '')}</span></button>`
  }).join('') || '<div class="pal-empty">Nothing matches.</div>'
  const on = $('#pal-list .pal-item.on'); if (on) on.scrollIntoView({ block: 'nearest' })
}
function paletteRun(i) {
  const it = PAL.items[i]
  if (!it || it.disabled) return
  paletteClose()
  it.run()
}
document.addEventListener('click', (e) => { if (e.target.closest && e.target.closest('[data-pal-open]')) paletteOpen() })
document.addEventListener('input', (e) => { if (e.target.id === 'pal-q') { PAL.query = e.target.value; PAL.sel = 0; paletteDraw() } })
document.addEventListener('click', (e) => {
  if (!PAL.open) return
  const b = e.target.closest && e.target.closest('[data-pal-i],[data-pal-close],[data-pal-untarget]')
  if (!b) return
  e.stopPropagation()
  if (b.dataset.palClose !== undefined) return paletteClose()
  if (b.dataset.palUntarget !== undefined) { PAL.target = null; PAL.sel = 0; paletteDraw(); return $('#pal-q').focus() }
  paletteRun(Number(b.dataset.palI))
}, true)
document.addEventListener('keydown', (e) => {
  const mod = MAC ? e.metaKey : e.ctrlKey
  // ⌘K (Ctrl+K, Ctrl+Shift+P on Windows) — not while a terminal has the keys (Claude Code's own Ctrl+K).
  if (((mod && !e.shiftKey && e.key.toLowerCase() === 'k') || (!MAC && e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'p')) && !(e.target.closest && e.target.closest('.xterm'))) {
    e.preventDefault(); e.stopPropagation()
    return PAL.open ? paletteClose() : paletteOpen()
  }
  // ⇧⌘R / Ctrl+Shift+R: stop the recording (it asks first).
  if (mod && e.shiftKey && e.key.toLowerCase() === 'r' && S.state && S.state.recording && !(e.target.closest && e.target.closest('.xterm'))) {
    e.preventDefault(); e.stopPropagation(); return press({ cmd: 'stop-ask' })
  }
  // ⇧⌘I / Ctrl+Shift+U: show or hide Inbox (Ctrl+Shift+I is WebView2's DevTools on Windows).
  if (mod && e.shiftKey && e.key.toLowerCase() === NY_KEY.toLowerCase() && !(e.target.closest && e.target.closest('.xterm'))) {
    e.preventDefault(); e.stopPropagation(); return press({ panelFlip: 'needs' })
  }
  if (!PAL.open || e.target.id !== 'pal-q') return
  if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); return paletteClose() }
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); const n = PAL.items.length; if (n) PAL.sel = (PAL.sel + (e.key === 'ArrowDown' ? 1 : n - 1)) % n; return paletteDraw() }
  if (e.key === 'Enter') { e.preventDefault(); e.stopPropagation(); return paletteRun(PAL.sel) }
  if (e.key === 'Backspace' && !PAL.query && PAL.target) { PAL.target = null; PAL.sel = 0; return paletteDraw() }
}, true)

// ── render + events ────────────────────────────────────────────────────────────────────────
let lastKey = ''
let lastPrint = ''
/** Replace an element's content only when its HTML changed (keeps focus, selection, scroll and
 *  a frame's page where nothing changed). */
function patch(el, html) {
  if (el && el._html !== html) { el.innerHTML = html; el._html = html }
}

// ── what the user sees, for Echo's Claude terminals (`view.rs`: a UserPromptSubmit hook adds it to
// each prompt, so "this" means what is open or selected here). Sent when it changes, and every minute.
let viewSent = '', viewTimer = null, frameSel = ''
function viewState() {
  const act = ACTIVITIES.find((a) => a.id === S.view)
  const v = { activity: act ? act.label : S.view }
  const c = S.view === 'calls' && S.call && !S.call.error ? S.call : null
  if (c) {
    const d = c.started_at ? new Date(c.started_at) : null
    v.call = { name: c.name, title: c.title, tab: (CALL_TABS.find((t) => t.id === S.tab) || {}).label || S.tab,
      when: d ? d.toLocaleString([], { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '',
      status: c.status === 'recording' ? 'recording now' : c.status === 'transcribing' ? 'being transcribed' : '',
      threads: (c.projects || []).map((p) => p.name).join(', ') }
  }
  if (S.view === 'projects') {
    if (S.pj.found) v.filter = 'Found for you (threads Echo suggests from calls in none)'
    else if (S.pj.client) { const cl = projClient(S.pj.client); v.client = { id: S.pj.client, name: cl ? cl.name : S.pj.client, tab: S.pj.ctab } }
    else if (S.pj.sel) { const p = projById(S.pj.sel); v.thread = { id: S.pj.sel, name: p ? p.name : S.pj.sel, tab: S.pj.tab } }
  }
  if (S.view === 'tasks') v.filter = `Tasks across threads${S.tk.project ? ` · only ${(projById(S.tk.project) || {}).name || S.tk.project}` : ''}`
  if (S.view === 'settings') v.filter = `Settings · ${S.setSection}`
  if (S.view === 'brain') v.filter = `Brain · ${S.br.tab}${S.br.scope ? ` · scoped to ${S.br.scope}` : ''}`
  if (S.view === 'calendar') {
    v.filter = `Calendar · ${S.cal.mode}`
    const e = S.cal.selected && S.cal.data && (S.cal.data.events || []).find((x) => x.id === S.cal.selected)
    if (e) v.event = { subject: e.subject, when: e.start ? new Date(e.start).toLocaleString([], { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '' }
  }
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay)
  const n = S.pj.node && S.pj.work && S.pj.work.nodes && S.pj.work.nodes[S.pj.node]
  if (n && [eff.right, eff.bottom].some((p) => p.shown && p.active === 'work-item')) {
    v.item = { id: n.id, title: n.title, kind: n.kind === 'task' && n.type && n.type !== 'action' ? n.type : n.kind, state: n.state, thread: (projById(S.pj.sel) || {}).name || S.pj.sel }
    // What a record holds (an answer, the decision, a response) or the work's description.
    if (n.description) Object.assign(v.item, { what: (RECORD_TEXT[n.kind] || RECORD_TEXT.story).word, text: n.description })
    if (n.kind === 'decision' && n.decided_by) v.item.decided_by = n.decided_by
    if (n.kind === 'risk') Object.assign(v.item, { owner: n.internal_owner || undefined, likelihood: n.likelihood || undefined, impact: n.impact || undefined })
  }
  v.panel = ['right', 'bottom'].filter((a) => eff[a].shown && eff[a].active).map((a) => (PANEL_VIEWS.find((x) => x.id === eff[a].active) || {}).label).filter(Boolean).join(', ')
  const s = window.getSelection && window.getSelection()
  const inTerm = s && s.anchorNode && s.anchorNode.parentElement && s.anchorNode.parentElement.closest('.xterm')
  const page = s && !inTerm ? String(s).replace(/\s+/g, ' ').trim() : ''
  if (page) v.selection = { text: page.slice(0, 400), where: S.view === 'calls' && S.tab === 'transcript' ? 'transcript' : 'page' }
  else if (frameSel && c && S.tab === 'minutes') v.selection = { text: frameSel.slice(0, 400), where: 'minutes' }
  return v
}
function reportView(now) {
  clearTimeout(viewTimer)
  viewTimer = setTimeout(() => {
    const v = JSON.stringify(viewState())
    if (v !== viewSent || now) { viewSent = v; send({ cmd: 'view_state', view: JSON.parse(v) }) }
  }, now ? 0 : 400)
}
document.addEventListener('selectionchange', () => reportView())
setInterval(() => { if (document.visibilityState === 'visible') reportView(true) }, 60000)

function render() {
  if (plTyping()) { S.renderOnBlur = true; return }
  reportView()
  // Animate entering a view / call / tab, never a background refresh.
  const key = `${S.view}|${S.selected}|${S.tab}`
  const entering = key !== lastKey
  lastKey = key
  const focused = document.activeElement
  const focus = focused && focused.id
  const caret = focus && focused.selectionStart
  const scroll = $('#pane') ? $('#pane').scrollTop : 0
  // The live transcript follows the call while it is scrolled to the end.
  const lt = $('#live-transcript')
  const ltStick = !lt || lt.scrollTop + lt.clientHeight >= lt.scrollHeight - 40, ltTop = lt ? lt.scrollTop : 0
  // The live notes keep where they were scrolled to (an update re-renders them).
  const ls = $('.live-scroll'), lsTop = ls ? ls.scrollTop : 0
  const termFocus = focusedTerm()
  const app = $('#app')
  // The workbench: fixed areas, each replaced only when its HTML changed (`patch`).
  if (!$('#r-top')) {
    app.innerHTML = `<div class="wb${MAC ? ' mac' : ''}" id="wb"><header class="topbar" id="r-top"></header><div class="actbar" id="r-act"></div>
      <div class="primary" id="r-primary"></div><aside class="panel right" id="r-right" aria-label="Right panel"></aside>
      <section class="panel bottom" id="r-bottom" aria-label="Bottom panel"></section><footer class="statusbar" id="r-status"></footer></div><div id="r-over"></div>`
  }
  const wb = $('#wb')
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay)
  wb.classList.toggle('has-right', eff.right.shown)
  wb.classList.toggle('right-over', eff.right.overlay)
  wb.classList.toggle('has-bottom', eff.bottom.shown)
  wb.classList.toggle('bottom-folded', eff.bottom.folded)
  wb.style.setProperty('--right-w', `${eff.right.size}px`)
  wb.style.setProperty('--bottom-h', `${eff.bottom.size}px`)
  patch($('#r-top'), `<div class="pill-slot">${pill()}</div>`)
  patch($('#r-act'), actbar())
  patch($('#r-status'), statusbar())
  patch($('#r-right'), panel('right', eff))
  patch($('#r-bottom'), panel('bottom', eff))
  patch($('#r-over'), `<div class="toasts" role="status" aria-live="polite">${toasts()}</div>` + (S.lightbox ? `<div class="lightbox" data-close="1"><img src="${esc(S.lightbox)}" alt="Shared screen"></div>` : ''))
  // The primary area in the activity's layout; switching layouts starts it afresh.
  const primary = $('#r-primary')
  const layout = (ACTIVITIES.find((a) => a.id === S.view) || ACTIVITIES[0]).layout
  if (primary.dataset.layout !== layout || primary.dataset.view !== S.view) {
    primary.innerHTML = layout === 'list-detail' ? '<div class="list-detail" id="r-ld"><div class="region" id="r-list"></div><main class="main" id="r-main"></main></div>' : ''
    primary._html = undefined
    primary.dataset.layout = layout
    primary.dataset.view = S.view
  }
  if (S.view === 'settings') {
    patch(primary, `<main class="main">${noticesBar()}${settingsView()}</main>`)
  } else if (S.view === 'brain') {
    patch(primary, `<main class="main">${noticesBar()}${brainView()}</main>`)
    drawGraph()
    drawPeople()
  } else if (S.view === 'tasks') {
    patch(primary, `<main class="main">${noticesBar()}${tasksView()}</main>`)
  } else if (S.view === 'projects') {
    patch(primary, `<main class="main">${noticesBar()}${projectsView()}</main>`)
    drawOrgLinks()
  } else if (S.view === 'calendar') {
    const grid = $('#calgrid'), keep = grid ? grid.scrollTop : null
    patch(primary, `<main class="main">${noticesBar()}${calendarView()}</main>`)
    const g = $('#calgrid')
    // First open: around now, or 08:00 on a week without today.
    if (g && g !== grid) g.scrollTop = keep != null ? keep : Math.max(0, ($('.now') ? $('.now').offsetTop - 120 : 8 * Number(g.dataset.hour)))
  } else {
    // The list, the call's header and its pane — on the Minutes tab, the pane's bar and the frame
    // apart (the frame reloads only when the call or its minutes change; detaching it would reload
    // it too).
    $('#r-ld').classList.toggle('list-open', S.listOpen)
    patch($('#r-list'), list())
    const d = doc(), main = $('#r-main')
    if (d.plain !== undefined) {
      main.dataset.mode = 'plain'
      patch(main, noticesBar() + d.plain)
    } else {
      if (main.dataset.mode !== 'doc') {
        main.dataset.mode = 'doc'
        patch(main, '<header class="doc-head" id="r-head"></header><div class="pane" id="pane"></div>')
      }
      patch($('#r-head'), d.head)
      const pane = $('#pane')
      if (d.frame !== undefined) {
        if (pane.dataset.mode !== 'frame') {
          pane.dataset.mode = 'frame'
          pane.classList.add('framed')
          patch(pane, '<div id="minutes-bar"></div><div class="minutes-slot" id="minutes-slot"></div>')
        }
        patch($('#minutes-bar'), d.bar)
        patch($('#minutes-slot'), d.frame)
      } else {
        if (pane.dataset.mode === 'frame') { pane.dataset.mode = ''; pane.classList.remove('framed') }
        patch(pane, d.pane)
      }
    }
  }
  // An element that was replaced gets its focus back (one left alone keeps its own selection).
  const again = focus && $('#' + focus)
  if (again && again !== focused) { again.focus(); try { again.setSelectionRange(caret, caret) } catch {} }
  const pane = $('#pane')
  let jumped = false
  if (pane) {
    const j = $('#jump')
    if (S.jump != null && j) { j.scrollIntoView({ block: 'center' }); S.jump = null; jumped = true }
    else pane.scrollTop = scroll
  }
  const lt2 = $('#live-transcript')
  if (lt2 && !jumped) lt2.scrollTop = ltStick ? lt2.scrollHeight : ltTop
  const ls2 = $('.live-scroll')
  if (ls2) ls2.scrollTop = lsTop
  if (entering) { const p = $('.pane') || $('.grid-wrap'); if (p) { p.classList.remove('enter'); void p.offsetWidth; p.classList.add('enter') } }
  placeDetail()
  pushNotes(false)
  attachTerms(termFocus)
  const g = $('#calgrid')
  if (g) g.addEventListener('scroll', placeDetail, { passive: true })
  lastPrint = fingerprint()
}

/** Put the event card next to the event it belongs to: right of it if there is room, else
 *  left; level with it, kept inside the window. */
function placeDetail() {
  const card = $('.detail'), ev = $('.ev[aria-selected="true"]')
  if (!card || !ev) return
  const r = ev.getBoundingClientRect(), w = card.offsetWidth, h = card.offsetHeight, gap = 8, pad = 12
  const right = r.right + gap + w <= window.innerWidth - pad
  const left = right ? r.right + gap : Math.max(pad, r.left - gap - w)
  const top = Math.min(Math.max(pad, r.top), Math.max(pad, window.innerHeight - h - pad))
  card.style.left = `${left}px`
  card.style.right = 'auto'
  card.style.top = `${top}px`
}

/** What the screen depends on: re-render only when it changes (keeps selection and scroll). */
// (the minute too: the pill's "meeting soon" comes and goes with the clock)
const fingerprint = () => JSON.stringify([S.state, S.calls, S.call, S.agenda, S.cal.data, S.cal.selected, S.settings, S.pj.data, S.pj.detail, S.pj.foundData, S.pj.work, Math.floor(Date.now() / 60000)])

document.addEventListener('click', (e) => {
  // Any click closes the call's ⋯ menu (its own button toggles it).
  const menuWas = S.menuOpen
  if (menuWas && !(e.target.closest && e.target.closest('[data-cmd="call-menu"]'))) S.menuOpen = false
  const t = e.target.closest('[data-me-edit],[data-me-save],[data-me-cancel],[data-setsec],[data-set],[data-act],[data-lang-del],[data-vocab-del],[data-reclang-set],[data-layout-move],[data-layout-reset],[data-term-output],[data-term-start],[data-note-edit],[data-edit-kind],[data-note-kind],[data-note-done],[data-note-del],[data-panel-flip],[data-panel-show],[data-panel-move],[data-panel-toggle],[data-go],[data-call],[data-tab],[data-cmd],[data-at],[data-toggle],[data-frame],[data-close],[data-copy],[data-arm],[data-join],[data-ev],[data-calmode],[data-calnav],[data-calclose],[data-cal-import],[data-schedule],[data-unschedule],[data-ask],[data-signin],[data-signin-cancel],[data-signin-accept],[data-signin-check],a[data-ext]')
  if (!t) { if (menuWas) render(); return }
  // Sign-ins (`signin.rs`): shown at once (starting / stopping / checking) until the next poll
  // brings the app's state.
  const busySignin = (id, state) => { const p = profileOf(id); if (p) { p.signin = { state }; render() } }
  if (t.dataset.signin) { send({ cmd: 'sign_in', profile: t.dataset.signin }); return busySignin(t.dataset.signin, 'starting') }
  if (t.dataset.signinCancel) { send({ cmd: 'sign_in_cancel', profile: t.dataset.signinCancel }); return busySignin(t.dataset.signinCancel, 'stopping') }
  if (t.dataset.signinAccept) { send({ cmd: 'accept_account', profile: t.dataset.signinAccept, account: t.dataset.account }); return busySignin(t.dataset.signinAccept, 'checking') }
  if (t.dataset.signinCheck) { send({ cmd: 'check_sign_in', profile: t.dataset.signinCheck }); return busySignin(t.dataset.signinCheck, 'checking') }
  if (t.dataset.ext !== undefined) { e.preventDefault(); send({ cmd: 'open_url', url: t.getAttribute('href') }); return }
  // Settings
  if (t.dataset.meEdit !== undefined) { S.meEdit = (S.settings && S.settings.me_profile && S.settings.me_profile.text) || ''; render(); const ta = $('#me-profile'); if (ta) ta.focus(); return }
  if (t.dataset.meCancel !== undefined) { S.meEdit = null; return render() }
  if (t.dataset.meSave !== undefined) return saveMeProfile()
  if (t.dataset.setsec) return go(`settings/${t.dataset.setsec}`)
  if (t.dataset.set) {
    let val; try { val = JSON.parse(t.dataset.val) } catch { val = t.dataset.val }
    if (S.settings && t.dataset.set.startsWith('learned_')) {
      S.settings.learned = (S.settings.learned || []).filter((x) => x.term !== val)
      if (t.dataset.set === 'learned_always') S.settings.vocabulary = [...(S.settings.vocabulary || []), val]
    } else if (S.settings) S.settings[t.dataset.set] = val // shown at once; the next snapshot confirms it
    if (t.dataset.set === 'live_notes' && S.state && S.state.ai) S.state.ai.live_notes = val
    if (t.dataset.set === 'default_profile' && S.state && S.state.ai) S.state.ai.default = val
    if (t.dataset.set === 'brain_enabled') {
      if (S.state) S.state.brain = !!val
      if (S.settings && S.settings.brain) S.settings.brain.enabled = !!val
      setTimeout(() => loadSettings().then(render), 400)
    }
    send({ cmd: 'settings_set', key: t.dataset.set, value: val })
    return render()
  }
  if (t.dataset.act) { send({ cmd: 'settings_action', action: t.dataset.act, arg: t.dataset.arg || '' }); setTimeout(() => loadSettings().then(render), 1500); return }
  if (t.dataset.langDel) { const l = (S.settings.languages || []).filter((x) => x !== t.dataset.langDel); S.settings.languages = l; send({ cmd: 'settings_set', key: 'languages', value: l }); return render() }
  if (t.dataset.vocabDel) { const v = (S.settings.vocabulary || []).filter((x) => x !== t.dataset.vocabDel); S.settings.vocabulary = v; send({ cmd: 'settings_set', key: 'vocabulary', value: v }); return render() }
  if (t.dataset.reclangSet !== undefined) { setRecordLang(t.dataset.reclangSet); return render() }
  if (t.dataset.layoutMove) { Layout.move(t.dataset.layoutMove, t.dataset.to, PANEL_VIEWS); return layoutChanged() }
  if (t.dataset.layoutReset !== undefined) { Layout.load({}); return layoutChanged() }
  if (t.dataset.termOutput) { S.termOutput = S.termOutput || {}; S.termOutput[t.dataset.termOutput] = !S.termOutput[t.dataset.termOutput]; return render() }
  if (t.dataset.termStart) {
    const sel = t.closest('.panel-body') && t.closest('.panel-body').querySelector('[data-term-profile]')
    if (sel) S.folderProfile = sel.value
    return termStart(t.dataset.termStart, t.dataset.termKey, { call: t.dataset.termCall, profile: sel ? sel.value : undefined })
  }
  if (t.dataset.noteEdit !== undefined) {
    const i = Number(t.dataset.noteEdit), b = S.call && NoteDoc.parse(NotesSync.view(S.call.name).draft)[i]
    if (!b || !b.note || S.closing) return
    // Clicking a note's text to select it for copying isn't an edit.
    const sel = window.getSelection && window.getSelection()
    if (sel && !sel.isCollapsed && t.classList.contains('nt-text')) return
    // Another note edited meanwhile is saved first, as its Save would.
    if (S.noteEdit && S.noteEdit.i !== i) commitPendingEdit()
    S.noteEdit = { i, text: b.text, kind: b.kind, call: S.call.name }
    render()
    const ta = $('#note-edit'); if (ta) { ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length) }
    return
  }
  if (t.dataset.editKind) { S.noteEdit.kind = t.dataset.editKind; render(); const ta = $('#note-edit'); if (ta) ta.focus(); return }
  if (t.dataset.noteKind) { S.noteKind = t.dataset.noteKind; render(); const el = $('#note-input'); if (el) el.focus(); return }
  if (t.dataset.noteDone !== undefined) { const b = NoteDoc.parse(NotesSync.view(S.call.name).draft)[Number(t.dataset.noteDone)]; return editNotes((text) => NoteDoc.change(text, Number(t.dataset.noteDone), { done: !(b && b.done) })) }
  if (t.dataset.noteDel !== undefined) return editNotes((text) => NoteDoc.change(text, Number(t.dataset.noteDel), null))
  if (t.dataset.panelFlip) {
    // A view's own button: show it, or close its panel when it is what the panel shows.
    const id = t.dataset.panelFlip, known = panelViews(), a = Layout.area(id, known)
    const eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
    const open = a && eff[a].shown && eff[a].active === id
    if (eff.narrow && a === 'right') { S.overlay = !open; if (!open) Layout.show(id, known); return render() }
    if (open) Layout.toggle(a); else Layout.show(id, known)
    return layoutChanged()
  }
  if (t.dataset.panelShow) { Layout.show(t.dataset.panelShow, panelViews()); return layoutChanged() }
  if (t.dataset.panelMove) { Layout.move(t.dataset.panelMove, t.dataset.to, panelViews()); return layoutChanged() }
  if (t.dataset.panelToggle) {
    if (S.overlay && t.dataset.panelToggle === 'right') { S.overlay = false; return render() } // a narrow window's overlay closes; the saved layout stays
    Layout.toggle(t.dataset.panelToggle); return layoutChanged()
  }
  if (t.dataset.go) return goActivity(t.dataset.go)
  if (t.dataset.ev !== undefined) { S.cal.selected = S.cal.selected === t.dataset.ev ? null : t.dataset.ev; return render() }
  if (t.dataset.calclose) { S.cal.selected = null; return render() }
  if (t.dataset.calmode) { S.cal.mode = t.dataset.calmode; S.cal.offset = 0; S.cal.data = null; go(`calendar/${S.cal.mode}`); return loadCalendar().then(render) }
  if (t.dataset.calnav !== undefined) { const n = Number(t.dataset.calnav); S.cal.offset = n === 0 ? 0 : S.cal.offset + n; S.cal.data = null; render(); return loadCalendar().then(render) }
  if (t.dataset.calImport) { send({ cmd: 'calendar_import', id: t.dataset.calImport }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.calImport); if (e) e.import = { busy: true }; return render() }
  if (t.dataset.schedule) { send({ cmd: 'schedule', id: t.dataset.schedule, languages: recordLang(), profile: pickedProfile() }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.schedule); if (e) e.scheduled = true; return render() }
  if (t.dataset.unschedule) { send({ cmd: 'unschedule', id: t.dataset.unschedule }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.unschedule); if (e) e.scheduled = false; return render() }
  if (t.dataset.call) { S.listOpen = false; return go(`call/${t.dataset.call}/${S.tab}`) }
  if (t.dataset.tab) return go(`call/${S.selected}/${t.dataset.tab}`)
  if (t.dataset.at) { S.jump = Number(t.dataset.at); S.find = ''; return go(`call/${S.selected}/transcript`) }
  if (t.dataset.frame) { S.lightbox = t.dataset.frame; return render() }
  if (t.dataset.close) { S.lightbox = null; return render() }
  if (t.dataset.copy) { send({ cmd: 'copy', text: t.dataset.copy }); t.textContent = 'Copied'; return }
  if (t.dataset.toggle) {
    const a = S.call && S.call.actions && S.call.actions.find((x) => x.id === t.dataset.toggle)
    if (a) { a.status = t.dataset.status; render() } // optimistic; the file follows (or a notice says why not)
    return send({ cmd: 'toggle_action', call: S.selected, id: t.dataset.toggle, status: t.dataset.status })
  }
  if (t.dataset.arm !== undefined) return send({ cmd: 'arm_meeting', id: t.dataset.arm, languages: recordLang(), profile: pickedProfile() })
  if (t.dataset.join !== undefined) return send({ cmd: 'join_meeting', id: t.dataset.join, languages: recordLang(), profile: pickedProfile() })
  if (t.dataset.ask) { send({ cmd: 'live_ask', call: S.selected, kind: t.dataset.ask }); return liveBusy() }
  switch (t.dataset.cmd) {
    case 'record': return send({ cmd: 'record', languages: recordLang(), profile: pickedProfile() })
    case 'stop': S.stopAsk = false; render(); return send({ cmd: 'stop' })
    case 'stop-ask': S.stopAsk = true; return render()
    case 'stop-keep': S.stopAsk = false; return render()
    case 'list-toggle': S.listOpen = !S.listOpen; return render()
    case 'call-menu': S.menuOpen = !S.menuOpen; return render()
    case 'claude-panel': return claudePanel()
    case 'ask-claude': return claudePanel('claude-call')
    case 'notes-raw': S.notesRaw = true; return render()
    case 'notes-timeline': S.notesRaw = false; return render()
    case 'note-flag': return addNote('flag')
    case 'note-edit-save': return saveNoteEdit()
    case 'note-edit-cancel': S.noteEdit = null; return render()
    case 'reveal': render(); return send({ cmd: 'reveal', call: S.selected })
    case 'show-log': return send({ cmd: 'show_log', call: S.selected })
    case 'notes-now': { send({ cmd: 'notes_now', call: S.selected }); if (S.call && S.call.notes_run) { S.call.notes_run.now = true; S.call.notes_run.message = 'Starting now…' } return render() }
    case 'write-notes': {
      // The run reads the user's notes: they are saved first, or it doesn't start.
      const call = S.selected
      const pid = S.notesProfile && S.notesProfile.call === call && S.notesProfile.id ? S.notesProfile.id : null
      // A note typed or edited and not added yet is part of them.
      if (commitPendingNotes()) render()
      NotesSync.flush(call).then(() => {
        send({ cmd: 'write_notes', call, profile: pid || undefined })
        if (S.call && S.selected === call) { S.call.notes_run = { ...(S.call.notes_run || {}), state: 'queued', had_minutes: !!S.call.notes, model: profileName(pid || (S.call.live && S.call.live.profile)), message: 'Starting…' }; render() }
      }, (err) => { S.notice = `Your notes couldn't be saved (${err.message}), so the minutes weren't started.`; render() })
      return
    }
    case 'open-minutes': render(); return send({ cmd: 'open_minutes', call: S.selected })
    case 'reveal-minutes': render(); return send({ cmd: 'reveal_minutes', call: S.selected })
    case 'stamp': return stampNotes()
    case 'notes-retry': NotesSync.save(S.selected); return render()
    case 'kept-ok': NotesSync.clearKept(S.selected); return render()
    case 'notice-ok': S.notice = null; return render()
    case 'close-retry': S.closeError = null; render(); return send({ cmd: 'close_request' })
    case 'close-now': return send({ cmd: 'close_now' })
    case 'live-pause': return send({ cmd: 'live_pause', call: S.selected })
    case 'live-resume': return send({ cmd: 'live_resume', call: S.selected })
    case 'ask-delete': S.confirmDelete = true; return render()
    case 'cancel-delete': S.confirmDelete = false; return render()
    case 'delete': {
      const name = S.selected
      S.confirmDelete = false; S.selected = null; S.call = null
      // A note being typed or edited for it goes with it.
      if (sameCall(S.noteDraftCall, name)) { S.noteDraft = ''; S.noteAt = null; S.noteDraftCall = null }
      if (S.noteEdit && sameCall(S.noteEdit.call, name)) S.noteEdit = null
      S.calls = (S.calls || []).filter((c) => c.name !== name)
      send({ cmd: 'delete', call: name })
      return go('calls')
    }
  }
})

// The user's notes: saved 1 s after the last keystroke, and when the editor loses focus (a tab
// or a call chosen, the window closing). No render while typing (`tick`, `refresh`).
let notesTimer = null
const saveNotesSoon = (call) => { clearTimeout(notesTimer); notesTimer = setTimeout(() => { NotesSync.save(call); showNotesState() }, 1000) }
/** The editor shows the draft NotesSync holds — a tray note taken in by a refresh, or appended by a
 *  save — at once, even while it has focus (no render then): the next keystroke must not save
 *  over what it didn't show. The caret and selection stay. */
function syncEditor() {
  const ta = $('#my-notes')
  if (!ta || !S.call || String(S.call.name).slice(0, 17) !== String(ta.dataset.notesCall).slice(0, 17)) return
  const draft = NotesSync.view(ta.dataset.notesCall).draft
  if (ta.value === draft) return
  const at = ta.selectionStart, end = ta.selectionEnd
  ta.value = draft
  try { ta.setSelectionRange(Math.min(at, draft.length), Math.min(end, draft.length)) } catch { /* not focused */ }
}

/** The editor's (or the timeline's) "Saved / Saving… / Not saved yet" line, without re-rendering it. */
function showNotesState() {
  const el = $('#notes-state'), on = $('#my-notes') || $('.notes-tl')
  const call = on && on.dataset.notesCall
  if (el && call) el.textContent = notesState(NotesSync.view(call), pendingNote(call))
}
/** The Time button: `[m:ss] ` (time into the call; tray → Add a note… stamps `[mm:ss]`) at the caret. */
function stampNotes() {
  const ta = $('#my-notes')
  if (!ta || !S.call || !S.call.started_at || ta.readOnly || ta.dataset.notesCall !== S.call.name) return
  const call = ta.dataset.notesCall
  const stamp = `[${clock((Date.now() - new Date(S.call.started_at)) / 1000)}] `
  const at = ta.selectionStart ?? ta.value.length
  ta.setRangeText(stamp, at, ta.selectionEnd ?? at, 'end')
  ta.focus()
  NotesSync.edit(call, ta.value)
  saveNotesSoon(call)
  showNotesState()
}
// Panels resize by their inner edge: the size follows the pointer (no render while dragging), and
// is kept with the layout when the drag ends; a double-click puts it back to its default.
document.addEventListener('mousedown', (e) => {
  const h = e.target.closest && e.target.closest('[data-resize]')
  if (!h || e.button !== 0) return
  e.preventDefault()
  const area = h.dataset.resize, wb = $('#wb')
  const sizeAt = (ev) => area === 'right'
    ? Math.min(Math.max(220, window.innerWidth - ev.clientX), window.innerWidth - 360)
    : Math.min(Math.max(160, window.innerHeight - 24 - ev.clientY), window.innerHeight - 200)
  let size = null
  document.body.classList.add(area === 'right' ? 'resizing-x' : 'resizing-y')
  let frame = 0
  const move = (ev) => {
    size = sizeAt(ev)
    wb.style.setProperty(area === 'right' ? '--right-w' : '--bottom-h', `${size}px`)
    // A terminal in the panel follows the new size (once a frame).
    if (!frame) frame = requestAnimationFrame(() => { frame = 0; for (const t of Terms.values()) termFit(t) })
  }
  const up = () => {
    document.removeEventListener('mousemove', move); document.removeEventListener('mouseup', up)
    document.body.classList.remove('resizing-x', 'resizing-y')
    if (size != null) { Layout.resize(area, Math.round(size)); layoutChanged() }
  }
  document.addEventListener('mousemove', move); document.addEventListener('mouseup', up)
})
document.addEventListener('dblclick', (e) => {
  const h = e.target.closest && e.target.closest('[data-resize]')
  if (h) { Layout.resize(h.dataset.resize, null); layoutChanged() }
})

// macOS: the top strip is the window's title bar (transparent, `window.rs`): dragging it moves the
// window, a double click zooms it — anywhere but on its controls.
const onTitleBar = (e) => MAC && e.button === 0 && e.target.closest && e.target.closest('#r-top') && !e.target.closest('button,select,label,a,input,.pill-note')
document.addEventListener('mousedown', (e) => { if (onTitleBar(e) && e.detail === 1) { e.preventDefault(); send({ cmd: 'window_drag' }) } })
document.addEventListener('dblclick', (e) => { if (onTitleBar(e)) send({ cmd: 'window_zoom' }) })

// The Time button keeps the editor focused (and its caret where it was).
document.addEventListener('mousedown', (e) => { if (e.target.closest && e.target.closest('[data-cmd="stamp"]')) e.preventDefault() })
document.addEventListener('focusout', (e) => {
  if (e.target.id !== 'my-notes') return
  clearTimeout(notesTimer)
  NotesSync.save(e.target.dataset.notesCall)
  if (S.renderOnBlur) { S.renderOnBlur = false; setTimeout(render) } else showNotesState()
})

/** The minutes frame shows the user's notes under their lines (its `EMBED_JS`): sent when it is
 *  ready and whenever they change — never written into the page or its file. */
let pushedNotes = ''
function pushNotes(force) {
  const f = $('#minutes-frame')
  if (!f || !f.contentWindow || !S.call) return
  const notes = noteMarks(S.call).map((b) => ({ t: b.t, kind: b.kind, text: b.text }))
  const key = S.call.name + JSON.stringify(notes)
  if (!force && key === pushedNotes) return
  pushedNotes = key
  f.contentWindow.postMessage({ echo: 'notes', notes }, '*')
}

/** The open call's timed notes, for the transcript's margin. */
function noteMarks(c) {
  if (!c || !c.name) return []
  return NoteDoc.parse(NotesSync.view(c.name).draft).filter((b) => b.note && b.t != null)
}

// ── comment on what was said: select words in a transcript → Comment → a note at that moment,
// with the words as its quote. The popover lives outside the regions render() replaces.
function selPop() {
  let el = $('#sel-pop')
  if (!el) { el = document.createElement('div'); el.id = 'sel-pop'; document.body.appendChild(el) }
  return el
}
function hideSelPop() { S.selNote = null; const el = $('#sel-pop'); if (el) el.remove() }
function showSelPop() {
  const n = S.selNote, el = selPop()
  el.className = 'sel-pop' + (n.open ? ' open' : '')
  el.style.left = `${Math.max(8, Math.min(n.x, window.innerWidth - (n.open ? 380 : 200)))}px`
  el.style.top = `${Math.max(8, Math.min(n.y, window.innerHeight - (n.open ? 170 : 80)))}px`
  const at = n.t != null ? NoteDoc.stamp(n.t) : null
  const chip = at ? `<span class="at">${esc(at)}</span> ` : ''
  const quote = n.quote ? `“${esc(n.quote.length > 160 ? n.quote.slice(0, 157) + '…' : n.quote)}”` : ''
  const where = `${at ? `at ${esc(at)}` : 'as a general note'}${n.where ? ` · on “${esc(n.where)}”` : ''}`
  el.innerHTML = n.open
    ? `<div class="sel-quote">${chip}${quote}</div>
       <textarea id="sel-note" rows="2" placeholder="Your note — ? question, ! decision, [] to-do" aria-label="Your note"></textarea>
       <div class="sel-acts"><span class="grow">Goes to My notes ${where}</span><button class="btn ghost" data-sel="cancel">Cancel</button><button class="btn primary" data-sel="save">Add note</button></div>`
    : n.menu
      ? `<div class="ctx-menu" role="menu"><button class="menu-item" role="menuitem" data-sel="open">Add note${at ? ` at ${esc(at)}` : ''}…</button>${n.sel ? '<button class="menu-item" role="menuitem" data-sel="copy">Copy</button>' : ''}</div>`
      : '<button class="btn primary" data-sel="open">Comment</button>'
  if (n.open) { const ta = $('#sel-note'); if (ta) ta.focus() }
}
function commentSelection() {
  const n = S.selNote, ta = $('#sel-note')
  if (!n || !ta || !S.call) return
  const typed = NoteDoc.typed(ta.value, 'note')
  if (!typed.text && !n.quote) return
  const at = typed.t !== undefined ? typed.t : n.t
  editNotes((text) => NoteDoc.add(text, { kind: typed.kind, t: at == null ? null : Math.floor(at), text: typed.text, quote: n.quote || null }))
  hideSelPop()
  window.getSelection && window.getSelection().removeAllRanges()
  // Show where it went (the notes panel, if it is closed).
  const known = panelViews(), a = Layout.area('notes', known)
  const eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (a && !(eff[a].shown && eff[a].active === 'notes')) { if (eff.narrow) S.overlay = true; Layout.show('notes', known); layoutChanged() }
}
document.addEventListener('mouseup', (e) => {
  if (e.button !== 0 || (e.target.closest && e.target.closest('#sel-pop'))) return // a right-click has its own menu
  setTimeout(() => {
    const sel = window.getSelection && window.getSelection()
    if (!sel || sel.isCollapsed || !S.call || S.call.status === undefined) { if (S.selNote && !S.selNote.open) hideSelPop(); return }
    const node = sel.anchorNode && (sel.anchorNode.nodeType === 1 ? sel.anchorNode : sel.anchorNode.parentElement)
    const turn = node && node.closest && node.closest('.transcript .turn')
    const quote = sel.toString().replace(/\s+/g, ' ').trim()
    if (!turn || !quote) return
    const r = sel.getRangeAt(0).getBoundingClientRect()
    S.selNote = { t: Number(turn.dataset.t) || 0, quote: quote.slice(0, 300), x: Math.max(8, r.left), y: r.bottom + 6, open: false }
    showSelPop()
  })
})
document.addEventListener('mousedown', (e) => {
  if (S.selNote && !(e.target.closest && e.target.closest('#sel-pop'))) hideSelPop()
})
// Right-click a turn of a transcript: Add note at its time (about the words selected in it, if any).
document.addEventListener('contextmenu', (e) => {
  const turn = e.target.closest && e.target.closest('.transcript .turn')
  if (!turn || !S.call) return
  e.preventDefault()
  const sel = window.getSelection && window.getSelection()
  const inside = sel && !sel.isCollapsed && turn.contains(sel.anchorNode)
  const quote = inside ? sel.toString().replace(/\s+/g, ' ').trim().slice(0, 300) : ''
  S.selNote = { t: Number(turn.dataset.t) || 0, quote, x: e.clientX, y: e.clientY, open: false, menu: true, sel: !!quote }
  showSelPop()
})
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('#sel-pop [data-sel]')
  if (!b) return
  if (b.dataset.sel === 'open') { S.selNote.open = true; return showSelPop() }
  if (b.dataset.sel === 'cancel') return hideSelPop()
  if (b.dataset.sel === 'copy') { send({ cmd: 'copy', text: S.selNote.quote }); return hideSelPop() }
  if (b.dataset.sel === 'save') return commentSelection()
})

// The minutes page in its frame: a time opens the transcript there, an https link the browser.
// Only from that frame, and only these two shapes.
window.addEventListener('message', (e) => {
  const f = $('#minutes-frame')
  if (!f || e.source !== f.contentWindow) return
  const d = e.data
  if (!d || typeof d !== 'object') return
  if (d.echo === 'at' && typeof d.t === 'number' && Number.isFinite(d.t) && d.t >= 0) { S.jump = d.t; S.find = ''; go(`call/${S.selected}/transcript`) }
  // The page is ready: it gets the user's notes to show under their lines.
  else if (d.echo === 'ready') pushNotes(true)
  // Right-click on a line of the minutes: Add note (at its time, about its words, under its section).
  else if (d.echo === 'note' && typeof d.quote === 'string' && Number.isFinite(d.x) && Number.isFinite(d.y)) {
    const t = typeof d.t === 'number' && Number.isFinite(d.t) && d.t >= 0 ? d.t : null
    const r = f.getBoundingClientRect()
    S.selNote = { t, quote: d.quote.slice(0, 300), where: typeof d.section === 'string' ? d.section.slice(0, 80) : '', x: r.left + d.x, y: r.top + d.y, open: false, menu: true }
    showSelPop()
  }
  else if (d.echo === 'open' && typeof d.url === 'string' && /^https:\/\//i.test(d.url)) send({ cmd: 'open_url', url: d.url })
  // Words selected in the minutes: what "this" means to Claude (`viewState`).
  else if (d.echo === 'sel' && typeof d.text === 'string') { frameSel = d.text.slice(0, 400); reportView() }
})

document.addEventListener('input', (e) => {
  // The call the editor was drawn for, whatever is selected by now.
  if (e.target.id === 'my-notes') { const call = e.target.dataset.notesCall; NotesSync.edit(call, e.target.value); saveNotesSoon(call); showNotesState(); return }
  if (e.target.id === 'q') { S.query = e.target.value; render() }
  if (e.target.id === 'set-q') { S.setQuery = e.target.value; render() }
  if (e.target.id === 'learned-q') { S.learnedQuery = e.target.value; render() }
  if (e.target.id === 'me-profile') S.meEdit = e.target.value // no render while typing
  if (e.target.id === 'find') { S.find = e.target.value; render() }
  if (e.target.id === 'ask') S.liveDraft = e.target.value // no render: typing must not be disturbed
  // The note box: the call's time is taken at the first key (what was just said), not at Enter.
  if (e.target.id === 'note-edit' && S.noteEdit) { S.noteEdit.text = e.target.value; showNotesState(); return }
  if (e.target.id === 'note-input') {
    S.noteDraft = e.target.value
    if (!S.noteDraft) { S.noteAt = null; S.noteDraftCall = null } else {
      if (S.noteDraftCall == null) S.noteDraftCall = S.call && S.call.name
      if (S.noteAt == null) S.noteAt = callNow(S.call)
    }
    showNotesState()
  }
})

document.addEventListener('change', (e) => {
  if (e.target.matches('[data-lang-add]') && e.target.value) {
    const l = [...(S.settings.languages || []), e.target.value]
    S.settings.languages = l; send({ cmd: 'settings_set', key: 'languages', value: l }); return render()
  }
  if (e.target.id === 'me-name' || e.target.id === 'me-email') {
    const me = { name: ($('#me-name') || {}).value || '', email: ($('#me-email') || {}).value || '' }
    S.settings.me = me; send({ cmd: 'settings_set', key: 'me', value: me }); return
  }
  if (e.target.matches('[data-reclang]')) { setRecordLang(e.target.value); render() }
  // A profile chosen for the next recording or for these notes: a sign-in starts if it is plainly
  // signed out (any choice, of any kind, supersedes an earlier one's check).
  if (e.target.matches('[data-recprofile]')) { S.recordProfile = e.target.value; send({ cmd: 'sign_in_if_needed', profile: e.target.value, purpose: 'record' }); render() }
  if (e.target.matches('[data-notesprofile]')) { S.notesProfile = { call: S.selected, id: e.target.value }; send({ cmd: 'sign_in_if_needed', profile: e.target.value, purpose: `notes:${S.selected}` }); render() }
  if (e.target.matches('[data-callprofile]')) {
    send({ cmd: 'set_profile', call: S.selected, profile: e.target.value })
    if (S.call && S.call.live) { S.call.live.profile = e.target.value; render() }
  }
})

document.addEventListener('toggle', (e) => {
  if (e.target.matches && e.target.matches('details[data-prep]')) S.prepOpen[e.target.dataset.prep] = e.target.open
}, true)

document.addEventListener('submit', (e) => {
  if (e.target.matches('[data-vocab-form]')) {
    e.preventDefault()
    const term = ($('#vocab-new').value || '').trim()
    if (!term) return
    const v = [...new Set([...(S.settings.vocabulary || []), term])]
    S.settings.vocabulary = v; send({ cmd: 'settings_set', key: 'vocabulary', value: v }); return render()
  }
  if (!e.target.matches('[data-askform]')) return
  e.preventDefault()
  const text = S.liveDraft.trim()
  if (!text) return
  send({ cmd: 'live_ask', call: S.selected, kind: 'ask', text })
  S.liveDraft = ''
  liveBusy()
})

/** A question was sent: show it as pending until the next poll brings the app's state. */
function liveBusy() {
  const st = S.call && S.call.live && S.call.live.status
  if (st) st.queued = (st.queued || 0) + 1
  render()
}

document.addEventListener('keydown', (e) => {
  // A terminal gets every key; ⌘Esc / Ctrl+Esc gives focus back to the page.
  if (e.target.closest && e.target.closest('.xterm')) {
    if (e.key === 'Escape' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); e.target.blur() }
    return
  }
  if (e.target.id === 'note-input') {
    if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); return addNote() }
    if (e.key === 'Escape') { S.noteDraft = ''; S.noteAt = null; S.noteDraftCall = null; e.target.value = ''; showNotesState(); return }
    return
  }
  if (e.target.id === 'note-edit') {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); return saveNoteEdit() }
    if (e.key === 'Escape') { S.noteEdit = null; return render() }
    return
  }
  if (e.target.id === 'me-profile') {
    if (e.key === 's' && (MAC ? e.metaKey : e.ctrlKey)) { e.preventDefault(); return saveMeProfile() }
    if (e.key === 'Escape') { S.meEdit = null; return render() }
    return
  }
  if (e.target.id === 'sel-note' && e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); return commentSelection() }
  if (e.key === 'Escape' && S.selNote) return hideSelPop()
  if (e.key === 'Escape' && S.lightbox) { S.lightbox = null; return render() }
  if (e.key === 'Escape' && (S.stopAsk || S.listOpen || S.menuOpen || S.overlay)) { S.stopAsk = false; S.listOpen = false; S.menuOpen = false; S.overlay = false; return render() }
  if ((MAC ? e.metaKey : e.ctrlKey) && e.key === ',') { e.preventDefault(); return goActivity('settings') }
  if ((MAC ? e.metaKey : e.ctrlKey) && e.key === '`') { e.preventDefault(); return claudePanel() }
  // ⌘1…9 / Ctrl+1…9: the activities, in the bar's order.
  if ((MAC ? e.metaKey : e.ctrlKey) && !e.altKey && !e.shiftKey && /^[1-9]$/.test(e.key)) {
    const a = shownActivities()[Number(e.key) - 1]
    if (a) { e.preventDefault(); return goActivity(a.id) }
  }
  if (S.view !== 'calls' || !S.calls || !S.calls.length || /input|textarea|select/i.test(e.target.tagName)) return
  if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
  e.preventDefault()
  const i = S.calls.findIndex((c) => c.name === S.selected)
  const n = Math.min(S.calls.length - 1, Math.max(0, i + (e.key === 'ArrowDown' ? 1 : -1)))
  go(`call/${S.calls[n].name}/${S.tab}`)
})

window.addEventListener('resize', () => render())

window.addEventListener('hashchange', async () => {
  route()
  render()
  if (S.view === 'calendar' && !S.cal.data) { await loadCalendar(); render() }
  if (S.view === 'settings') { await loadSettings(); render() }
  if (S.view === 'projects') { await loadProjects(); await loadProject(); render() }
  if (S.view === 'tasks') { await loadTasks(); render() }
  if (S.view === 'brain') { await loadBrain(); render() }
  if (S.view === 'calls' && S.selected && !S.call) { await loadCall(S.selected); render() }
})

// ── refresh: cheap local polling (the files change under us during a call) ─────────────────
let ticks = 0
async function tick() {
  ticks++
  const before = JSON.stringify(S.state)
  await loadState()
  const changed = JSON.stringify(S.state) !== before
  if (changed || ticks % 3 === 0) await loadCalls()
  if (ticks % 30 === 0) await loadAgenda() // a meeting about to start (the pill)
  if (S.view === 'settings' ? ticks % 2 === 0 : ticks % 60 === 0) await loadSettings() // a download's progress; Settings' badge
  // While a transcript is imported, every few seconds: the call shows once it is there.
  if (S.view === 'calendar' && (ticks % 30 === 0 || (ticks % 3 === 0 && ((S.cal.data && S.cal.data.events) || []).some((e) => e.import && e.import.busy)) || (ticks % 5 === 0 && S.cal.data && S.cal.data.checking_transcripts))) await loadCalendar()
  if (ticks % 10 === 1 || (ticks % 3 === 0 && (NY.busy.size || Object.keys((NY.data || {}).busy || {}).length))) await loadAttention()
  if (ticks % (S.view === 'projects' ? 5 : 15) === 0) { await loadProjects(); if (S.view === 'projects') await loadProject() }
  if (S.view === 'tasks' && ticks % 15 === 0) { await loadTasks(); if (S.pj.node) await loadWork() }
  // The selected call is fetched again whenever anything it shows changed (`/rev`: its files,
  // the live notes' and the minutes run's state, when the minutes come), whatever its status.
  if (S.selected) {
    let rev = null
    try { rev = (await get('api/call/' + encodeURIComponent(S.selected) + '/rev')).rev } catch { /* retitled: loadCall finds it */ }
    if (changed || !S.call || rev !== S.call.rev) await loadCall(S.selected)
  }
  // Typing a question doesn't hold back the live notes (its text and caret survive a render); a
  // text selection, an open menu or a focused search field does (until it ends). The user's notes
  // being typed hold back every render until the editor is left.
  if (typingNotes()) { if (fingerprint() !== lastPrint) S.renderOnBlur = true; return }
  const sel = window.getSelection && window.getSelection()
  const busy = document.querySelector('input:focus:not(#ask), select:focus') || (sel && !sel.isCollapsed)
  if (fingerprint() !== lastPrint && (!busy || changed)) render()
}
const typingNotes = () => document.activeElement && (document.activeElement.id === 'my-notes' || document.activeElement.id === 'me-profile')

window.echo = {
  async refresh() {
    await loadCalls(); if (S.selected) await loadCall(S.selected); if (S.view === 'calendar') await loadCalendar(true); if (S.view === 'settings') await loadSettings()
    await loadProjects(); if (S.view === 'projects') await loadProject()
    if (S.view === 'tasks') { await loadTasks(); if (S.pj.node) await loadWork() }
    if (typingNotes()) S.renderOnBlur = true
    else render()
  },
  openCall(name) { go(`call/${name}/minutes`) },
  /** A project the page just created: open it on its Profile. */
  async projectCreated(id) {
    // From a calendar event: that event is in it, and its meeting series is its first rule.
    const fe = S.pj.fromEvent
    S.pj.fromEvent = null
    if (fe) {
      send({ cmd: 'project', op: 'event_member', id, event: fe.id, how: 'include' })
      if (fe.subject) send({ cmd: 'project', op: 'rule_add', id, rule: { kind: 'series', subject: fe.subject } })
    }
    // From a call: that call is in it, and its meeting series is its first rule.
    const from = S.pj.fromCall
    S.pj.fromCall = null
    if (from) {
      send({ cmd: 'project', op: 'member', id, call: from, how: 'include' })
      const title = S.call && S.call.name === from && S.call.meeting_subject
      if (title) send({ cmd: 'project', op: 'rule_add', id, rule: { kind: 'series', subject: title } })
    }
    await loadProjects()
    go(`project/${id}/profile`)
  },
  /** A client the page just added from a project's Profile: that project's client. */
  clientCreated(id) { if (S.pj.sel) send({ cmd: 'project', op: 'set', id: S.pj.sel, field: 'client', value: id }) },
  /** The app's answer to a save of the user's notes (`save_my_notes`). */
  saved(r) {
    NotesSync.saved(r)
    // The editor gets a tail the tray appended meanwhile, without a render while it is typed in.
    syncEditor()
    if ((r.error || r.kept) && !typingNotes()) render()
    else showNotesState()
  },
  /** A message for a moment (an action that couldn't be done). */
  notice(text) {
    S.notice = text
    // An action that couldn't be done: the decision clicked a moment ago (the last one) gets its buttons back.
    if (NY.last && Date.now() - NY.last.at < 10000) { NY.busy.delete(NY.last.key); NY.last = null }
    clearTimeout(window._noticeTimer)
    window._noticeTimer = setTimeout(() => { S.notice = null; render() }, 7000)
    render()
  },
  /** The app started a terminal (`pty_open`): follow it. */
  ptyOpened(r) {
    const e = r.key && Terms.get(r.key)
    if (!e) return
    e.starting = false
    e.signin = r.ok ? null : r.signin || null
    e.error = r.ok ? null : String(r.error || 'It could not start.')
    if (!r.ok) { e.exited = true; e.term.write(`\r\n\x1b[31m${String(r.error || 'It could not start.').replace(/\n/g, '\r\n')}\x1b[0m\r\n`); return render() }
    e.id = r.id; e.from = 0; e.exited = false; e.cols = 0
    get('api/sessions').then((l) => { S.sessions = new Set(l) }, () => {})
    termPoll(e)
    render()
    e.term.focus()
  },
  /** The leave the editors were frozen for didn't happen (cancelled, or the update or folder
   *  change failed): writable again. */
  thaw() {
    S.closing = false
    S.closeError = null // Echo asked, and the user decided: no banner for it
    const ta = $('#my-notes')
    if (ta) ta.readOnly = false
    if (!typingNotes()) render()
  },
  /** The window is closing (`close`), or Echo quitting, updating, moving its data folder: every
   *  editor read-only, every unsaved note saved, then `close_ready` says for handshake `n` whether
   *  it worked. */
  prepareClose(n, close) {
    // A note typed in the box or edited in place without Enter is saved too, in its own call.
    const committed = commitPendingNotes()
    S.closing = true
    const ta = $('#my-notes')
    if (ta) ta.readOnly = true
    if (committed && !typingNotes()) render() // the note in the list, the box empty and read-only
    clearTimeout(notesTimer)
    // Saved: the editors stay read-only until Echo is gone, or `thaw` says it stays.
    NotesSync.flushAll().then(
      () => send({ cmd: 'close_ready', ok: true, n }),
      (err) => {
        // A close explains itself here (Try again / Close without saving); for anything else Echo
        // asks the user, and the editors stay read-only until it goes or `thaw`s them.
        if (close) {
          S.closing = false
          S.closeError = err.message
          const now = $('#my-notes')
          if (now) now.readOnly = false
        }
        render()
        send({ cmd: 'close_ready', ok: false, n })
      },
    )
  },
  async selftest() {
    const out = { selftest: true, base: location.href }
    try {
      out.state = (await fetch('api/state')).status
      const calls = await get('api/calls'); out.calls = calls.length
      if (calls.length) {
        const d = await get('api/call/' + encodeURIComponent(calls[0].name)); out.segments = (d.segments || []).length; out.notes = d.notes ? d.notes.length : 0
        // A call with minutes: its page is served, and the Minutes tab (open on it) shows it in a frame.
        if (d.notes) { out.minutes_page = (await fetch('minutes/' + encodeURIComponent(d.name))).status; out.minutes_frame = S.selected !== d.name || S.tab !== 'minutes' || !!document.querySelector('#minutes-frame') }
      }
      out.forbidden = (await fetch('file/x/notes.md')).status
      // The terminals' xterm.js is served and loaded.
      out.xterm = (await fetch('assets/vendor/xterm.js')).status === 200 && typeof window.Terminal === 'function' && !!window.FitAddon
      // A terminal end to end: the app starts a shell, a command typed here runs, its output comes back.
      out.terminal = out.xterm && await new Promise((resolve) => {
        const key = 'selftest-term', e = termEntry(key)
        let over = false
        const done = (ok) => { if (over) return; over = true; clearInterval(iv); clearInterval(check); if (e.id != null) send({ cmd: 'pty_close', id: e.id }); e.id = null; Terms.delete(key); e.el.remove(); resolve(ok) }
        const iv = setInterval(() => { if (e.id != null) { clearInterval(iv); send({ cmd: 'pty_input', id: e.id, data: 'echo self$((40+2))test\r' }) } }, 50)
        const check = setInterval(() => {
          const b = e.term.buffer.active
          for (let i = 0; i < b.length; i++) { const l = b.getLine(i); if (l && l.translateToString().includes('self42test')) return done(true) }
        }, 100)
        setTimeout(() => done(false), 8000)
        send({ cmd: 'pty_open', kind: 'shell', key, cols: 80, rows: 24 })
      })
      out.rendered = !!(document.querySelector('#r-top .pill') && document.querySelector('#r-act .act') && document.querySelector('#r-primary .main'))
      // A sandboxed frame like the minutes page's: it says it ran, then posts to the window's IPC
      // directly — the app must not get that (it checks, and reports `frame_blocked`).
      out.frame_ready = await new Promise((resolve) => {
        const f = document.createElement('iframe')
        f.setAttribute('sandbox', 'allow-scripts')
        f.style.display = 'none'
        const done = (ok) => { window.removeEventListener('message', on); f.remove(); resolve(ok) }
        // (`frame_sees_ipc`: whether the frame could reach the handler at all — then only `from_page` stops it.)
        const on = (e) => { if (e.source === f.contentWindow && e.data && e.data.echo === 'ready') { out.frame_sees_ipc = e.data.ipc === true; setTimeout(() => done(true), 500) } }
        window.addEventListener('message', on)
        setTimeout(() => done(false), 4000)
        f.src = 'selftest/frame'
        document.body.appendChild(f)
      })
      out.ok = out.state === 200 && out.forbidden === 403 && out.xterm && out.terminal && out.rendered && out.frame_ready && out.minutes_page !== 404 && out.minutes_frame !== false
    } catch (e) { out.ok = false; out.error = String(e) }
    send(out)
  },
}

;(async () => {
  route()
  render()
  await Promise.all([loadState(), loadCalls(), loadAgenda(), loadLayout(), loadTerms(), loadSettings(), loadProjects()])
  await loadPluginClients()
  if (S.view === 'projects') await loadProject()
  if (S.view === 'tasks') await loadTasks()
  if (S.view === 'brain') await loadBrain()
  if (S.view === 'calls' && !S.selected && S.calls && S.calls.length) S.selected = S.calls[0].name, history.replaceState(null, '', `#call/${S.selected}/minutes`)
  if (S.selected) await loadCall(S.selected)
  if (S.view === 'calendar') await loadCalendar()
  render()
  setInterval(tick, 2000)
  setInterval(tickClocks, 1000)
})()
