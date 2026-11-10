// ── refresh: cheap local polling (the files change under us during a call) ─────────────────
let ticks = 0
async function tick() {
  // A window nobody sees (minimised, hidden, covered by the call app) fetches and draws nothing: it catches up when shown.
  if (document.hidden) return
  ticks++
  const before = JSON.stringify(S.state)
  await loadState()
  const changed = JSON.stringify(S.state) !== before
  if (changed || ticks % 3 === 0) await loadCalls()
  if (ticks % 30 === 0) await loadAgenda() // a meeting about to start (the pill)
  if (ticks % 15 === 0 && [...Terms.values()].some((e) => e.id != null && !e.exited)) await loadPtyInfo() // Restart to update
  if (S.view === 'settings' ? ticks % 2 === 0 : ticks % 60 === 0) await loadSettings() // a download's progress; Settings' badge
  // While a transcript is imported, every few seconds: the call shows once it is there.
  if (S.view === 'calendar' && (ticks % 30 === 0 || (ticks % 3 === 0 && ((S.cal.data && S.cal.data.events) || []).some((e) => e.import && e.import.busy)) || (ticks % 5 === 0 && S.cal.data && S.cal.data.checking_transcripts))) await loadCalendar()
  if (ticks % 10 === 1 || (ticks % 3 === 0 && (NY.busy.size || Object.keys((NY.data || {}).busy || {}).length))) await loadAttention()
  if (ticks % (S.view === 'projects' ? 5 : 15) === 0) { await loadProjects(); if (S.view === 'projects') await loadProject() }
  if (S.view === 'tasks' && ticks % 15 === 0) { await loadTasks(); if (S.pj.node) await loadWork() }
  // The selected call is fetched again whenever anything it shows changed (`/rev`: its files,
  // the live notes' and the minutes run's state, when the minutes come), whatever its status.
  // A call that isn't there (an old link, a deleted call) is asked for once, not every tick (#150).
  if (S.selected && (!callGone() || ticks % 15 === 0)) {
    let rev = null
    try { rev = (await get('api/call/' + encodeURIComponent(S.selected) + '/rev')).rev } catch { /* retitled: loadCall finds it */ }
    if (changed || !S.call || rev !== S.call.rev) await loadCall(S.selected, callGone()) // a gone call's retry: no list reload
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
  /** A template saved (its id) or taken away (null): the list read again; a new one becomes the one edited; the one
   *  edited takes its kind from the list (Edited once Echo has the file). */
  templateSaved(id, created, which) {
    loadTemplates(true).then(() => {
      if (which === 'sk') {
        const rm = TPL.skRemoving
        TPL.skRemoving = null
        if (created && id) { TPL.skSel = id; TPL.skNew = null }
        else if (id === null && rm) TPL.skSel = rm.del ? null : rm.id
        return render()
      }
      if (which === 'mt') {
        const mts = (TPL.data && TPL.data.meeting_types) || [], rm = TPL.mtRemoving
        TPL.mtRemoving = null
        const pick = created && id ? id : id === null && rm && !rm.del ? rm.id : id === null ? null : TPL.mtSel
        const x = mts.find((y) => y.id === pick)
        if (created || id === null) { TPL.mtSel = x ? x.id : null; TPL.mtDraft = x ? { ...x } : null }
        else if (x && TPL.mtDraft) TPL.mtDraft.kind = x.kind
        return render()
      }
      const list = (TPL.data && TPL.data.templates) || []
      const rm = TPL.removing
      TPL.removing = null
      if (created && id) { const x = list.find((y) => y.id === id); if (x) { TPL.sel = x.id; TPL.draft = tplDraft(x); TPL.scrollTo = true } }
      else if (id === null && rm) {
        // Deleted: nothing chosen. Reset: Echo's own again, in the editor.
        const x = !rm.del && list.find((y) => y.id === rm.id)
        TPL.sel = x ? x.id : null; TPL.draft = x ? tplDraft(x) : null
      } else if (TPL.draft && TPL.draft.id) { const x = list.find((y) => y.id === TPL.draft.id); if (x) TPL.draft.kind = x.kind }
      render()
    })
  },
  async refresh() {
    await loadCalls(); if (S.selected) await loadCall(S.selected); if (S.view === 'calendar') await loadCalendar(true); if (S.view === 'settings') await loadSettings()
    await loadProjects(); if (S.view === 'projects') await loadProject()
    if (S.view === 'tasks') { await loadTasks(); if (S.pj.node) await loadWork() }
    if (S.view === 'today') await loadToday()
    if (typingNotes()) S.renderOnBlur = true
    else render()
  },
  openCall(name) { go(`call/${name}/minutes`) },
  /** Windows, Echo's own title bar: the window was maximised or restored. */
  maximized(on) { if (S.maximized !== on) { S.maximized = on; render() } },
  /** Windows: Echo's own title bar as saved (after a load, or a switch whose save failed: the switch goes back). */
  ownTitleBar(on) {
    if (S.state) S.state.own_title_bar = on
    if (S.settings) S.settings.own_title_bar = on
    render()
  },
  /** An echo:// link (a call at a moment, a thread, a client, a person, a mail): where Echo keeps it, or its mail in Outlook. */
  async openLink(link) {
    let t
    try { t = await get(`api/link?u=${encodeURIComponent(link)}`) } catch { return }
    if (t.error) return window.echo.notice(`Echo can't open that link: ${t.error}`)
    if (t.url) return send({ cmd: 'open_url', url: t.url })
    S.jump = t.jump != null ? t.jump : null
    // A work item: its thread's Work tab with its details open.
    if (t.node) { S.pj.wantNode = t.node; S.pj.node = t.node; S.pj.inline = null }
    go(t.hash)
    // The route changes on hashchange, after this: the details panel exists only once it is the Work tab.
    if (t.node) setTimeout(() => { Layout.show('work-item', panelViews()); layoutChanged() }, 300)
    if (t.person) setTimeout(() => showPerson(t.person), 300)
  },
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
    // The layout's pending save goes first (IPC keeps order: it is written before close_ready), #300.
    flushLayout()
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
      const calls = await get('api/calls'); out.calls = calls.calls.length
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
  await Promise.all([loadState(), loadCalls(), loadAgenda(), loadLayout(), loadTerms(), loadSettings(), loadProjects(), loadAttention()])
  await loadPluginClients()
  if (S.view === 'projects') await loadProject()
  if (S.view === 'tasks') await loadTasks()
  if (S.view === 'today') await loadToday()
  if (S.view === 'brain') await loadBrain()
  if (S.view === 'calls' && !S.selected && !S.upSel && !S.fromLink && S.calls && S.calls.length) S.selected = S.calls[0].name, replaceUrl(`#call/${S.selected}/minutes`)
  if (S.selected) await loadCall(S.selected)
  if (S.view === 'calendar') await loadCalendar()
  render()
  if (!MAC) send({ cmd: 'window_sync' }) // Windows: the title bar and the maximised state (`EchoWindow::tell_page`)
  setInterval(tick, 2000)
  setInterval(() => { if (!document.hidden) tickClocks() }, 1000)
  document.addEventListener('visibilitychange', () => { if (!document.hidden) { tickClocks(); tick() } })
})()
