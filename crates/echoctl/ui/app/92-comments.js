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
  else if (d.echo === 'link' && typeof d.link === 'string' && /^echo:\/\//i.test(d.link)) window.echo.openLink(d.link)
  // Words selected in the minutes: what "this" means to Claude (`viewState`).
  else if (d.echo === 'sel' && typeof d.text === 'string') { frameSel = d.text.slice(0, 400); reportView() }
})

document.addEventListener('input', (e) => {
  // The call the editor was drawn for, whatever is selected by now.
  if (e.target.id === 'my-notes') { const call = e.target.dataset.notesCall; NotesSync.edit(call, e.target.value); saveNotesSoon(call); showNotesState(); return }
  if (e.target.id === 'q') {
    S.query = e.target.value
    clearTimeout(S.qT)
    // A new search starts at the top of its results.
    S.qT = setTimeout(async () => { await loadCalls(true); const at = e.target.selectionStart; render(); const rows = $('.rows'); if (rows) rows.scrollTop = 0; const i = $('#q'); if (i) { i.focus(); i.setSelectionRange(at, at) } }, 200)
  }
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
  // The account chip: makes the chosen profile the default, as Settings' Make default does (a
  // signed-out one then shows its sign-in under the Record pill, nothing starts by itself).
  if (e.target.matches('[data-ai-default]')) {
    const val = e.target.value
    if (S.state && S.state.ai) S.state.ai.default = val
    if (S.settings) S.settings.default_profile = val
    send({ cmd: 'settings_set', key: 'default_profile', value: val }); render()
  }
  // A profile chosen for these notes: a sign-in starts if it is plainly signed out (any choice,
  // of any kind, supersedes an earlier one's check).
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
  if (e.key === 'Escape' && S.todayClose) { S.todayClose = null; return render() }
  if (e.key === 'Escape' && S.needsDlg) { S.needsDlg = null; return render() }
  if (e.key === 'Escape' && (S.stopAsk || S.listOpen || S.menuOpen || S.laterOpen || S.convAsk || S.overlay || S.barOver)) { S.barOver = false; S.convAsk = null; S.stopAsk = false; S.listOpen = false; S.menuOpen = false; S.laterOpen = false; S.overlay = false; return render() }
  if ((MAC ? e.metaKey : e.ctrlKey) && e.key === ',') { e.preventDefault(); return goActivity('settings') }
  if ((MAC ? e.metaKey : e.ctrlKey) && e.key === '`') { e.preventDefault(); return claudePanel() }
  if ((MAC ? e.metaKey : e.ctrlKey) && !e.shiftKey && !e.altKey && e.key === '\\') { e.preventDefault(); return flipBar() }
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
  if (S.view === 'today') { await loadToday(); render() }
  if (S.view === 'brain') { await loadBrain(); render() }
  if (S.view === 'calls' && S.selected && !S.call) { await loadCall(S.selected); render() }
})

