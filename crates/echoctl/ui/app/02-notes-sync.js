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
     *  or the fetch began before a save was answered (it may show the file from before), or a note is
     *  being typed (`typing`: it goes on the notes shown, and a save based on them keeps another editor's
     *  version beside, #130) — unless the file only grew at its end (a note from the tray or the CLI, #276). */
    seen(name, text, epoch, typing) {
      const c = of(name)
      if (epoch !== undefined && epoch !== c.epoch) return
      if (typing && c.synced !== null && !(c.draft === c.synced && text.startsWith(c.synced))) return
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

