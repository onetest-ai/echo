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
NY_ICON['plugin-update'] = NY_ICON.update
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
  // The same place again (another plugin picked meanwhile): read the hash again, not just redraw.
  if (t.dataset.nyGo) { if (decodeURIComponent(location.hash.slice(1)) === t.dataset.nyGo) { route(); return render() } return go(t.dataset.nyGo) }
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

