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
  setSection: 'general', // Settings' open section
  noteDraft: '', // the note being typed in My notes' box
  noteAt: null, // the call's time when it was started (seconds), while the call records
  noteKind: 'note', // note | question | decision | todo
  selNote: null, // a comment on words selected in the transcript: {t, quote, x, y, open}
  noteEdit: null, // the note being edited in place: {i (its block), text, kind}
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
  return { parse, line, add, change, typed, stamp }
})()
// end of the notes block

// ── workbench: what the window is made of, and where each view goes ────────────────────────
// The window is fixed areas (activity bar, primary view, right and bottom panels) filled from
// these lists; Echo's own views are entries like a plugin's will be. No DOM in here:
// `workbench.test.mjs` drives it from node.
const ICON = (d) => `<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`
/** Activities (the activity bar, in order). `layout`: `list-detail` (a list beside the item) or `single`. */
const ACTIVITIES = [
  { id: 'calls', label: 'Calls', layout: 'list-detail', icon: ICON('<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>') },
  { id: 'calendar', label: 'Calendar', layout: 'single', icon: ICON('<rect x="3.5" y="5" width="17" height="15" rx="2.5"/><path d="M3.5 10h17M8 3v4M16 3v4"/>') },
  { id: 'settings', label: 'Settings', layout: 'single', bottom: true, icon: ICON('<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>') },
]
/** A call's tabs, in order. `x` = what the call has: `{live, chat, frames, actions}` (counts, or
 *  null for actions not written yet). */
const CALL_TABS = [
  { id: 'live', label: 'Live', when: (x) => x.live },
  { id: 'minutes', label: 'Minutes' },
  { id: 'transcript', label: 'Transcript' },
  { id: 'chat', label: 'Chat', when: (x) => x.chat > 0, count: (x) => x.chat },
  { id: 'screens', label: 'Screens', when: (x) => x.frames > 0, count: (x) => x.frames },
  { id: 'actions', label: 'Actions', count: (x) => x.actions ?? '' },
]
/** The tabs a call shows: `[id, label, count ('' = none)]`. */
const callTabs = (x) => CALL_TABS.filter((t) => !t.when || t.when(x)).map((t) => [t.id, t.label, t.count ? t.count(x) : ''])
/** What a calendar event's card offers, first match wins: `e` the event, `st` its timing
 *  (`{past, live, recordable}`). The card draws each kind (`eventDetail`). */
const EVENT_ACTIONS = [
  { kind: 'open-notes', when: (e) => !!e.recorded },
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
  const sm = h.match(/^settings(?:\/([a-z]+))?$/)
  if (sm) { S.view = 'settings'; if (sm[1]) S.setSection = sm[1]; return }
  S.view = 'calls'
  const m = h.match(/^call\/([^/]+)(?:\/(\w+))?$/)
  if (m) {
    if (S.selected !== m[1]) { S.selected = m[1]; S.call = null; S.confirmDelete = false; S.find = '' }
    if (m[2]) S.tab = m[2]
  }
}
const go = (hash) => { if (location.hash.slice(1) !== hash) location.hash = hash; else render() }
/** An activity, where the user left it (the open call and tab; the calendar's day or week). */
const goActivity = (id) => go(id === 'calendar' ? `calendar/${S.cal.mode}` : id === 'settings' ? `settings/${S.setSection}` : (S.selected ? `call/${S.selected}/${S.tab}` : 'calls'))

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
    const badge = a.id === 'settings' && settingsWarnings() ? `<span class="act-badge" aria-label="${settingsWarnings()} to fix">${settingsWarnings()}</span>` : ''
    return `<button class="act" data-go="${a.id}"${S.view === a.id ? ' aria-current="page"' : ''} title="${esc(a.label)} (${a.id === 'settings' ? MOD + ',' : MOD + (i + 1)})">${a.icon}<span>${esc(a.label)}</span>${badge}</button>`
  }
  const top = ACTIVITIES.filter((a) => !a.bottom), bottom = ACTIVITIES.filter((a) => a.bottom)
  return `<nav class="acts" aria-label="Activities">${top.map(btn).join('')}<span class="acts-grow"></span>${bottom.map((a) => btn(a, ACTIVITIES.indexOf(a))).join('')}</nav>`
}

function statusbar() {
  const a = ai(), p = profileOf(a.default)
  const parts = []
  if (p) parts.push(`<span title="${esc(profileWhere(p))}">AI: ${esc(p.name)}</span>`)
  if (a.live_notes) parts.push('<span>Live notes on</span>')
  // The terminal panel: a toggle here, where VS Code keeps it.
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
      const meta = [d ? hm(d) : '', c.minutes ? `${c.minutes} min` : '', c.people.length ? c.people.slice(0, 2).join(', ') + (c.people.length > 2 ? ` +${c.people.length - 2}` : '') : ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
      return `${head}<button class="row" role="option" data-call="${esc(c.name)}" aria-selected="${c.name === S.selected}">
        ${statusIcon(c.status)}
        <span class="t">${esc(c.title)}</span>
        <span class="m">${meta}</span>
      </button>`
    }).join('')
  }
  return `<section class="list">
    <div class="list-head"><h1>Calls</h1><input class="search" id="q" placeholder="Search titles and people" value="${esc(S.query)}" aria-label="Search calls"></div>
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
  const live = hasLive(c)
  const tabs = callTabs({ live, chat: chat.length, frames: frames.length, actions: actions ? actions.length : null })
  // While a call records (or is transcribed), open it on Live once; once its minutes are written,
  // on Minutes once. The user's own choice stays.
  if (live && S.autoTab !== c.name && (c.status === 'recording' || c.status === 'transcribing')) { S.autoTab = c.name; S.tab = 'live' }
  if (S.autoTab === c.name && S.tab === 'live' && c.notes && c.notes_run && c.notes_run.state === 'done') { S.autoTab = 'done:' + c.name; S.tab = 'minutes' }
  if (!tabs.some((t) => t[0] === S.tab)) S.tab = 'minutes'
  const people = c.people && c.people.length ? `<span class="people">${esc(c.people.slice(0, 6).join(', '))}${c.people.length > 6 ? ` and ${c.people.length - 6} more` : ''}</span>` : ''
  const head = `${listToggle()}<div class="head-row">
        <div style="min-width:0">
          <h2 class="selectable">${esc(c.title)}</h2>
          <div class="facts">${d ? `<span>${esc(d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }))}, ${esc(hm(d))}</span>` : ''}${c.minutes ? `<span>${c.minutes} min</span>` : ''}${people}</div>
        </div>
        <div class="doc-actions">${callMenu(c)}</div>
      </div>
      <div class="tabs" role="tablist">${tabs.map(([id, label, n]) => `<button class="tab" role="tab" data-tab="${id}" aria-selected="${S.tab === id}">${label}${n !== '' ? `<span class="n">${n}</span>` : ''}</button>`).join('')}</div>${notices()}`
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
function notices() {
  const n = S.notice ? `<div class="notice"><span class="grow">${esc(S.notice)}</span><button class="btn ghost" data-cmd="notice-ok">OK</button></div>` : ''
  const c = S.closeError ? `<div class="notice error"><span class="grow">Your notes couldn't be saved: ${esc(S.closeError)}</span><button class="btn" data-cmd="close-retry">Try again</button><button class="btn ghost danger" data-cmd="close-now">Close without saving</button></div>` : ''
  return n + c
}

function noticesBar() {
  const n = notices()
  return n ? `<div class="notices">${n}</div>` : ''
}

function confirmBox(c) {
  return `<div class="confirm"><span>Delete "${esc(c.title)}"? It goes to the trash in the data folder for 30 days.</span>
    <button class="btn" data-cmd="cancel-delete">Keep</button><button class="btn primary" style="background:var(--danger);border-color:var(--danger)" data-cmd="delete">Delete</button></div>`
}

/** Transcript turns: consecutive lines of one speaker joined; `q` highlights, `jump` (seconds)
 *  marks the turn to scroll to. */
function turnsHtml(segs, q, jump, marks = []) {
  const turns = []
  for (const s of segs) {
    const last = turns[turns.length - 1]
    if (last && last.who === speaker(s) && s.start - last.end < 8) { last.text += ' ' + s.text; last.end = s.end }
    else turns.push({ who: speaker(s), me: speaker(s) === 'Me', spk: s.speaker_id, start: s.start, end: s.end, lang: s.lang, text: s.text })
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
      <div><div class="who">${esc(t.who)}${t.lang ? `<span class="lang">${esc(t.lang)}</span>` : ''}</div><p>${esc(t.text)}</p></div></div>`
  }).join('')}</div>`
}

function tabBody(c, segs, chat, frames, actions) {
  if (S.tab === 'transcript') {
    if (!segs.length) return c.status === 'recording'
      ? `<p style="color:var(--text-3)">Listening. The live transcript appears here as people speak.</p>`
      : `<p style="color:var(--text-3)">${c.status === 'transcribing' ? 'Transcribing on this computer. This takes about a quarter of the call length.' : 'No transcript for this call.'}</p>`
    return `<div class="from-bar"><input class="search" id="find" placeholder="Find in transcript" value="${esc(S.find)}" style="max-width:280px" aria-label="Find in transcript"></div>
      ${turnsHtml(segs, S.find.trim().toLowerCase(), S.jump, noteMarks(c))}`
  }
  if (S.tab === 'live') return liveTab(c, segs)
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
    if (!actions.length) return `<p style="color:var(--text-3)">No action items in this call.</p>`
    return `<div class="todo">${actions.map((a) => {
      const done = a.status === 'done'
      // By its id, with the status wanted: a list rewritten meanwhile can't make it another item.
      return `<div class="item${done ? ' done' : ''}">
        <button class="check" role="checkbox" aria-checked="${done}" data-toggle="${esc(a.id || '')}" data-status="${done ? 'todo' : 'done'}" aria-label="${done ? 'Mark to do' : 'Mark done'}"${a.id ? '' : ' disabled'}></button>
        <div><div class="task">${esc(a.task)}</div><div class="meta">
          <span class="owner${a.owner === 'Me' ? ' me' : ''}">${esc(a.owner || 'Unassigned')}</span>
          ${a.due ? `<span class="due">Due ${esc(a.due)}</span>` : ''}
          ${a.confidence === 'low' ? '<span class="uncertain">Uncertain</span>' : ''}
        </div></div>
        ${typeof a.t === 'number' ? `<button class="btn ghost" data-at="${a.t}">${clock(a.t)}</button>` : '<span></span>'}
      </div>`
    }).join('')}</div>`
  }
  return minutesTab(c)
}

// ── Notes: the user's own notes, an editor over my-notes.md (saved by NotesSync) ─────────────
const notesState = (v) => v.saving ? 'Saving…' : v.error ? `Couldn't save: ${v.error}` : v.dirty ? 'Not saved yet' : 'Saved'
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
    <div class="nt-bar"><span class="saved-state" id="notes-state">${esc(notesState(v))}</span><button class="icon-btn" data-cmd="notes-raw" title="Edit as text (my-notes.md)" aria-label="Edit as text">${ICON('<path d="M8 7l-5 5 5 5M16 7l5 5-5 5"/>')}</button></div>
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
  const ed = S.noteEdit
  if (!ed) return
  S.noteEdit = null
  const text = ed.text.replace(/\s+/g, ' ').trim()
  if (!text && ed.kind !== 'flag') return render() // emptied: delete is the × beside it
  editNotes((all) => NoteDoc.change(all, ed.i, { text, kind: ed.kind }))
}

/** Add what is typed in the note box (or a flag) at its moment. */
function addNote(kindOverride) {
  const c = S.call
  if (!c) return
  const typed = kindOverride ? { kind: kindOverride, text: '' } : NoteDoc.typed(S.noteDraft, S.noteKind)
  if (!kindOverride && !typed.text) return
  const t = kindOverride ? callNow(c) : typed.t !== undefined ? typed.t : (S.noteAt != null ? S.noteAt : callNow(c))
  editNotes((text) => NoteDoc.add(text, { kind: typed.kind, t: t == null ? null : Math.floor(t), text: typed.text }))
  if (!kindOverride) { S.noteDraft = ''; S.noteAt = null }
  render()
  const el = $('#note-input'); if (el) el.focus()
}
PANEL_VIEWS.push({ id: 'notes', label: 'My notes', icon: ICON('<path d="M5 4h10l4 4v12H5z"/><path d="M15 4v4h4M8 12h8M8 16h6"/>'), area: 'right', when: () => S.view === 'calls', render: notesPanel })
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
  Terms.set(key, e)
  termHolder().appendChild(el)
  return e
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
function minutesBar(c) {
  const run = c.notes_run
  const log = c.has_log ? '<button class="btn ghost" data-cmd="show-log">Show log</button>' : ''
  let status = ''
  if (run && run.state === 'running') status = `<span class="grow">Updating the minutes…</span>${log}`
  else if (run && run.state === 'queued') status = `<span class="grow">${esc(run.message || 'The update waits its turn.')}</span>`
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
    return box('Minutes are waiting their turn', `<p>${esc(run.message || 'They start as soon as the model is free.')}</p><p class="alt">With <b>${esc(run.model || '')}</b>. You can close this window.</p>`, '', true)
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

function liveHead(c) {
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
  return `<div class="live-head">${model}${where}<span class="live-state ${esc(state)}">${esc(line)}</span>${draftBy}<span class="grow"></span>${pause}</div>
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
  return `${n.now ? `<p class="live-now"><b>Now</b> ${esc(n.now)}</p>` : ''}
    ${sec('Decisions', n.decisions, (x) => `<li>${esc(x.text)}${atLink(x.t)}</li>`)}
    ${sec('Action items', n.actions, (a) => `<li><span class="owner${a.owner === 'Me' ? ' me' : ''}">${esc(a.owner)}</span> ${esc(a.task)}${atLink(a.t)}</li>`)}
    ${sec('Open questions', n.questions, (x) => `<li>${esc(x.text)}${atLink(x.t)}</li>`)}
    ${empty ? '<p class="muted">Nothing decided, owed or open yet.</p>' : ''}
    ${gaps ? `<p class="muted">Not covered: ${esc(gaps)}</p>` : ''}`
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
  const st = (c.live || {}).status || {}
  const busy = !!st.asking || st.queued > 0
  const dis = busy ? ' disabled' : ''
  return `<div class="live">
    ${liveHead(c)}
    <div class="live-grid">
      <section class="live-transcript" id="live-transcript" aria-label="Live transcript">${segs.length ? turnsHtml(segs, '', null, noteMarks(c)) : `<p class="muted">Listening. The transcript appears here as people speak.</p>`}</section>
      <section class="live-side" aria-label="Live notes">
        ${prepCard(c)}
        <div class="live-notes doc">${liveNotes(c)}</div>
        <div class="live-ask">
          <div class="live-buttons"><button class="btn" data-ask="missed"${dis}>What did I miss?</button><button class="btn" data-ask="question"${dis}>Suggest a question</button></div>
          <form class="ask-row" data-askform="1"><input id="ask" class="search" placeholder="Ask about this call…" value="${esc(S.liveDraft)}" autocomplete="off" maxlength="500" aria-label="Ask about this call"><button class="btn primary" type="submit"${dis}>Ask</button></form>
        </div>
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
        return `<button class="ev call" style="${style}" data-call="${esc(x.c.name)}"><b>${esc(x.c.title)}</b><small><span class="tag rec">Recorded</span> ${esc(hm(new Date(x.s)))}</small></button>`
      }
      const { ev: e, st } = x
      const cls = ['ev', e.online ? 'online' : '', e.recorded ? 'recorded' : '', e.scheduled ? 'scheduled' : '', st.past ? 'past' : '', e.declined ? 'declined' : '', e.cancelled ? 'cancelled' : ''].filter(Boolean).join(' ')
      const tag = e.recorded ? '<span class="tag rec">Recorded</span>' : e.scheduled ? '<span class="tag will">Will record</span>' : e.no_notetakers ? '<span class="tag no">No notetakers</span>' : ''
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
  }
  const kind = eventAction(e, st)
  const acts = kind ? ACTS[kind]() : ''
  const why = e.recorded ? '' : e.no_notetakers ? 'The organizer asked for no notetakers.' : e.declined ? 'You declined this meeting.' : e.cancelled ? 'This meeting was cancelled.' : !e.online ? 'Not a Teams meeting.' : st.past ? 'This meeting is over and was not recorded.' : ''
  return `<aside class="detail">
    <button class="btn ghost close" data-calclose="1" aria-label="Close">Close</button>
    <h2 class="selectable">${esc(e.subject)}</h2><div class="when">${esc(when)}</div>
    <dl>${e.organizer ? `<dt>Organizer</dt><dd>${esc(e.organizer)}</dd>` : ''}${e.people ? `<dt>People</dt><dd>${e.people}</dd>` : ''}${e.location ? `<dt>Where</dt><dd>${esc(e.location)}</dd>` : ''}${e.recorded ? `<dt>Recording</dt><dd>${STATUS[e.recorded.status] || esc(e.recorded.status)}${e.recorded.minutes ? `, ${e.recorded.minutes} min` : ''}</dd>` : ''}</dl>
    ${acts ? `<div class="acts">${acts}</div>` : ''}${why ? `<p class="why">${esc(why)}</p>` : ''}
    ${e.recap && e.recap.length ? `<div><b style="font-size:12px">Last time</b><ul>${e.recap.map((r) => `<li>${esc(r)}</li>`).join('')}</ul></div>` : ''}
  </aside>`
}

// ── Settings: every setting in one place (the tray keeps quick toggles). Plain values save at once
// (`settings_set`, the value shown, never a toggle of a stale one); flows with a dialog, a sign-in or
// a download run as the tray's own actions (`settings_action`). ────────────────────────────────
const SETTINGS_SECTIONS = [
  ['general', 'General'], ['recording', 'Recording'], ['transcription', 'Transcription'], ['ai', 'AI & minutes'],
  ['calendar', 'Calendar & chat'], ['vocabulary', 'Vocabulary'], ['me', 'About me'], ['layout', 'Layout'], ['health', 'Health'],
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
  const sec = SETTINGS_SECTIONS.some(([id]) => id === S.setSection) ? S.setSection : 'general'
  const q = (S.setQuery || '').trim().toLowerCase()
  const nav = SETTINGS_SECTIONS.filter(([, l]) => !q || l.toLowerCase().includes(q)).map(([id, label]) => {
    const warn = id === 'health' && settingsWarnings() ? `<span class="set-warn">⚠ ${settingsWarnings()}</span>` : ''
    return `<button class="set-nav${id === sec ? ' on' : ''}" data-setsec="${id}">${esc(label)}${warn}</button>`
  }).join('')
  const body = !st ? `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
    : st.config_error ? `<p class="error">Echo's settings (config.json) can't be read: ${esc(st.config_error)}</p>`
    : (SETTINGS_BODY[sec] || SETTINGS_BODY.general)(st)
  return `<div class="settings"><aside class="set-side"><h1>Settings</h1><input class="search" id="set-q" placeholder="Search settings" value="${esc(S.setQuery || '')}" aria-label="Search settings">${nav}</aside>
    <div class="set-main">${body}<p class="set-foot">Saved as you change it.</p></div></div>`
}

/** The profile the minutes read (me.md): shown rendered (the minutes' own safe Markdown), edited in place. */
function meProfileCard(mp) {
  const head = (ctl) => `<div class="set-row"><div><b>Profile for the minutes</b><div class="set-sub">Your role, your projects, how you like notes written — the minutes read it.</div></div><div class="set-ctl">${ctl}</div></div>`
  if (S.meEdit != null) {
    return `<section class="set-sec"><div class="set-card">${head('<button class="btn ghost" data-me-cancel>Cancel</button><button class="btn primary" data-me-save>Save</button>')}
      <div class="me-edit"><textarea id="me-profile" spellcheck="true" aria-label="Your profile (Markdown)">${esc(S.meEdit)}</textarea><div class="set-sub">Markdown: # for headings, - for lists. ⌘S / Ctrl+S saves, Esc cancels.</div></div></div></section>`
  }
  const body = mp && mp.text.trim() ? `<div class="doc me-doc">${mp.html}</div>` : '<div class="me-doc set-sub">Nothing yet — tell the minutes who you are.</div>'
  return `<section class="set-sec"><div class="set-card">${head('<button class="btn" data-me-edit>Edit</button>')}${body}</div></section>`
}

/** The terms the minutes learned: each with the meetings and people it is used again for. */
function learnedCard(learned) {
  const q = (S.learnedQuery || '').trim().toLowerCase()
  const shown = learned.filter((x) => !q || x.term.toLowerCase().includes(q) || x.subjects.some((s) => s.includes(q)) || x.people.some((p) => p.includes(q)))
    .sort((a, b) => a.term.localeCompare(b.term))
  const list = (xs, n = 3) => xs.length > n ? `${xs.slice(0, n).map(esc).join(', ')} +${xs.length - n}` : xs.map(esc).join(', ')
  const rows = shown.map((x) => setRow(esc(x.term),
    !x.subjects.length && !x.people.length ? 'Used only when a meeting\'s invite names it (from the older, shared list).'
      : [x.subjects.length ? `Meetings: ${list(x.subjects)}` : '', x.people.length ? `People: ${list(x.people)}` : '', x.calls ? `from ${x.calls} call${x.calls === 1 ? '' : 's'}` : ''].filter(Boolean).join(' · '),
    `<button class="btn" data-set="learned_always" data-val='${esc(JSON.stringify(x.term))}' title="Use it in every call">Always</button><button class="btn ghost" data-set="learned_remove" data-val='${esc(JSON.stringify(x.term))}' title="Forget it">Remove</button>`))
  const head = `<div class="set-row"><div><b>Learned from calls</b><div class="set-sub">${learned.length} term${learned.length === 1 ? '' : 's'} the minutes learned — each used again only in calls of the same meeting, or with mostly the same people.</div></div><div class="set-ctl"><input class="search set-input" id="learned-q" placeholder="Filter terms, meetings, people" value="${esc(S.learnedQuery || '')}" aria-label="Filter learned terms"></div></div>`
  return `<section class="set-sec"><div class="set-card">${head}${rows.join('') || (learned.length ? '<div class="set-row"><div class="set-sub">Nothing matches.</div></div>' : '')}</div></section>`
}

const SETTINGS_BODY = {
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
        setRow('Use a server', srv.client ? `Connected to ${esc(srv.client)}; this computer's model takes over when it's down.` : "Send each utterance to another Echo on your network; this computer's model takes over when it's down.", srv.client ? actBtn('disconnect_server', 'Disconnect') : actBtn('connect_server', 'Connect…')),
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
        setRow('Live notes during calls', "A running summary and the Live tab's questions, with the call's profile.", sw('live_notes', a.live_notes, 'Live notes')),
        setRow('Minutes model', 'The model that writes minutes for the default profile.', actBtn('notes_model', 'Change…')),
        setRow('Where Claude Code runs for minutes', st.notes_in ? `<code>${esc(st.notes_in)}</code> — its .claude/settings.json applies.` : 'The data folder.', actBtn('notes_choose', 'Choose…') + (st.notes_in ? actBtn('notes_anywhere', 'Use the data folder') : '')),
        setRow('Also write a minutes page file', "An HTML file in the call's folder, for sharing.", sw('minutes_page', st.minutes_page, 'Minutes page file')),
      ])
  },
  calendar(st) {
    return `<h2>Calendar &amp; chat</h2><p class="set-lead">Read-only, through Echo's own Microsoft 365 sign-in.</p>` + setCard('Microsoft 365', [
      setRow('Account', st.calendar ? `Signed in as ${esc(st.calendar)}` : 'Not connected: the calendar shows only what your tray knows about today.', st.calendar ? actBtn('calendar_sync', 'Sync now') + actBtn('calendar_logout', 'Sign out') : actBtn('calendar_login', 'Sign in…', '', true)),
      setRow('Meeting chat', "Each recorded meeting's Teams chat, kept with the call.", sw('capture_chat', st.capture_chat, 'Meeting chat')),
      setRow('App registration', st.graph_custom ? "Your organisation's own app." : 'The default (CodeMie). Your IT can register another.', actBtn('calendar_client_id', 'Change…')),
    ])
  },
  vocabulary(st) {
    const v = st.vocabulary || []
    const chips = v.map((t) => `<span class="chip">${esc(t)}<button data-vocab-del="${esc(t)}" aria-label="Remove ${esc(t)}">×</button></span>`).join('')
    return `<h2>Vocabulary</h2><p class="set-lead">Names and terms the transcription should hear right.</p>` + setCard('', [
      setRow('Always', `Used in every call.<div class="chips">${chips || '<span class="set-sub">None yet.</span>'}</div><form data-vocab-form class="vocab-add"><input class="search" id="vocab-new" placeholder="Add a term" maxlength="50" aria-label="Add a term"><button class="btn" type="submit">Add</button></form>`, ''),
    ]) + learnedCard(st.learned || [])
  },
  me(st) {
    const me = st.me || {}
    return `<h2>About me</h2><p class="set-lead">So the minutes know which voice is you, and write to you.</p>` + setCard('', [
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
    return `<h2>Health</h2><p class="set-lead">What Echo needs from this computer.</p>` + setCard('Permissions', rows) +
      setCard('', [setRow('Ask again', 'macOS asks for each permission Echo is missing.', actBtn('ask_permissions', 'Ask for permissions') + actBtn('recheck', 'Check again'))])
  },
}

// ── render + events ────────────────────────────────────────────────────────────────────────
let lastKey = ''
let lastPrint = ''
/** Replace an element's content only when its HTML changed (keeps focus, selection, scroll and
 *  a frame's page where nothing changed). */
function patch(el, html) {
  if (el && el._html !== html) { el.innerHTML = html; el._html = html }
}

function render() {
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
  patch($('#r-over'), S.lightbox ? `<div class="lightbox" data-close="1"><img src="${esc(S.lightbox)}" alt="Shared screen"></div>` : '')
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
  if (pane) {
    const j = $('#jump')
    if (S.jump != null && j) { j.scrollIntoView({ block: 'center' }); S.jump = null }
    else pane.scrollTop = scroll
  }
  const lt2 = $('#live-transcript')
  if (lt2) lt2.scrollTop = ltStick ? lt2.scrollHeight : ltTop
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
const fingerprint = () => JSON.stringify([S.state, S.calls, S.call, S.agenda, S.cal.data, S.cal.selected, S.settings, Math.floor(Date.now() / 60000)])

document.addEventListener('click', (e) => {
  // Any click closes the call's ⋯ menu (its own button toggles it).
  const menuWas = S.menuOpen
  if (menuWas && !(e.target.closest && e.target.closest('[data-cmd="call-menu"]'))) S.menuOpen = false
  const t = e.target.closest('[data-me-edit],[data-me-save],[data-me-cancel],[data-setsec],[data-set],[data-act],[data-lang-del],[data-vocab-del],[data-reclang-set],[data-layout-move],[data-layout-reset],[data-term-output],[data-term-start],[data-note-edit],[data-edit-kind],[data-note-kind],[data-note-done],[data-note-del],[data-panel-flip],[data-panel-show],[data-panel-move],[data-panel-toggle],[data-go],[data-call],[data-tab],[data-cmd],[data-at],[data-toggle],[data-frame],[data-close],[data-copy],[data-arm],[data-join],[data-ev],[data-calmode],[data-calnav],[data-calclose],[data-schedule],[data-unschedule],[data-ask],[data-signin],[data-signin-cancel],[data-signin-accept],[data-signin-check],a[data-ext]')
  if (!t) { if (menuWas) render(); return }
  // Sign-ins (`signin.rs`): shown at once (starting / stopping / checking) until the next poll
  // brings the app's state.
  const busySignin = (id, state) => { const p = profileOf(id); if (p) { p.signin = { state }; render() } }
  if (t.dataset.signin) { send({ cmd: 'sign_in', profile: t.dataset.signin }); return busySignin(t.dataset.signin, 'starting') }
  if (t.dataset.signinCancel) { send({ cmd: 'sign_in_cancel', profile: t.dataset.signinCancel }); return busySignin(t.dataset.signinCancel, 'stopping') }
  if (t.dataset.signinAccept) { send({ cmd: 'accept_account', profile: t.dataset.signinAccept, account: t.dataset.account }); return busySignin(t.dataset.signinAccept, 'checking') }
  if (t.dataset.signinCheck) { send({ cmd: 'check_sign_in', profile: t.dataset.signinCheck }); return busySignin(t.dataset.signinCheck, 'checking') }
  if (t.dataset.ext) { e.preventDefault(); send({ cmd: 'open_url', url: t.getAttribute('href') }); return }
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
    S.noteEdit = { i, text: b.text, kind: b.kind }
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
    case 'write-notes': {
      // The run reads the user's notes: they are saved first, or it doesn't start.
      const call = S.selected
      const pid = S.notesProfile && S.notesProfile.call === call && S.notesProfile.id ? S.notesProfile.id : null
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

/** The editor's "Saved / Saving…" line, without re-rendering the editor. */
function showNotesState() {
  const el = $('#notes-state'), ta = $('#my-notes')
  if (el && ta) el.textContent = notesState(NotesSync.view(ta.dataset.notesCall))
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
  if (e.target.id === 'note-edit' && S.noteEdit) { S.noteEdit.text = e.target.value; return }
  if (e.target.id === 'note-input') {
    S.noteDraft = e.target.value
    if (!S.noteDraft) S.noteAt = null
    else if (S.noteAt == null) S.noteAt = callNow(S.call)
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
    if (e.key === 'Escape') { S.noteDraft = ''; S.noteAt = null; e.target.value = ''; return }
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
    const a = ACTIVITIES[Number(e.key) - 1]
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
  if (S.view === 'calendar' && ticks % 30 === 0) await loadCalendar()
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
    if (typingNotes()) S.renderOnBlur = true
    else render()
  },
  openCall(name) { go(`call/${name}/minutes`) },
  /** The app's answer to a save of the user's notes (`save_my_notes`). */
  saved(r) {
    NotesSync.saved(r)
    // The editor gets a tail the tray appended meanwhile, without a render while it is typed in.
    syncEditor()
    if ((r.error || r.kept) && !typingNotes()) render()
    else showNotesState()
  },
  /** A message for a moment (an action that couldn't be done). */
  notice(text) { S.notice = text; render() },
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
    S.closing = true
    const ta = $('#my-notes')
    if (ta) ta.readOnly = true
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
  await Promise.all([loadState(), loadCalls(), loadAgenda(), loadLayout(), loadTerms(), loadSettings()])
  if (S.view === 'calls' && !S.selected && S.calls && S.calls.length) S.selected = S.calls[0].name, history.replaceState(null, '', `#call/${S.selected}/minutes`)
  if (S.selected) await loadCall(S.selected)
  if (S.view === 'calendar') await loadCalendar()
  render()
  setInterval(tick, 2000)
  setInterval(tickClocks, 1000)
})()
