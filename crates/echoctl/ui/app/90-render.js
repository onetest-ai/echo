// ── render + events ────────────────────────────────────────────────────────────────────────
let lastKey = ''
let lastPrint = ''
/** Replace an element's content only when its HTML changed (keeps focus, selection, scroll and
 *  a frame's page where nothing changed). */
function patch(el, html) {
  if (!el || el._html === html) return
  // A region drawn again keeps where its scrolled lists were (the calls list loading its next page), down and across
  // (the work board scrolled sideways to a column).
  const kept = [...el.querySelectorAll('.rows, [data-keep-scroll]')].map((x) => [x.scrollTop, x.scrollLeft])
  el.innerHTML = html; el._html = html
  if (kept.some(([t, l]) => t > 0 || l > 0)) [...el.querySelectorAll('.rows, [data-keep-scroll]')].forEach((x, i) => { if (kept[i]) { if (kept[i][0]) x.scrollTop = kept[i][0]; if (kept[i][1]) x.scrollLeft = kept[i][1] } })
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
  if (S.view === 'brain') v.filter = `Knowledge · ${S.br.tab}${S.br.scope ? ` · scoped to ${S.br.scope}` : ''}`
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

/** Draws the page. A render asked for while one runs (a field it removes fires blur or change, whose handler renders)
 *  waits for it to end: two at once would replace a region the other is replacing. */
function render() {
  if (render.busy) { setTimeout(render); return }
  render.busy = true
  try { renderNow() } finally { render.busy = false }
}
function renderNow() {
  if (plTyping()) { S.renderOnBlur = true; return }
  reportView()
  // Send to Claude's dialog: over whichever view the task was in (Today, a thread's Work, Tasks).
  {
    let h = document.getElementById('sc-holder')
    if (!h) { h = document.createElement('div'); h.id = 'sc-holder'; document.body.appendChild(h) }
    const html = sendClaudeDialog()
    if (h._html !== html) {
      // The field being typed in stays the one with the caret (a redraw replaces it).
      const a = h.contains(document.activeElement) ? document.activeElement : null
      const keep = a && a.id ? { id: a.id, s: a.selectionStart, e: a.selectionEnd } : null
      const opening = !h._html
      h.innerHTML = html; h._html = html
      const back = keep && h.querySelector('#' + keep.id)
      if (back) { ScKeys.restoring = true; try { back.focus() } finally { ScKeys.restoring = false } try { back.setSelectionRange(keep.s, keep.e) } catch { /* not a text field */ } }
      else if (html && opening) { const w = h.querySelector('#sc-q') || h.querySelector('#sc-words'); if (w) w.focus() }
    }
  }
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
  // My notes' list keeps its place when the panel is drawn again (a live update, the clock in the note box's hint).
  const nl = $('#nt-list')
  const nlWas = nl && { call: nl.closest('[data-notes-call]').dataset.notesCall, top: nl.scrollTop, height: nl.scrollHeight, client: nl.clientHeight }
  const termFocus = focusedTerm()
  const app = $('#app')
  // The workbench: fixed areas, each replaced only when its HTML changed (`patch`).
  if (!$('#r-top')) {
    app.innerHTML = `<div class="wb${MAC ? ' mac' : ''}" id="wb"><header class="topbar" id="r-top"></header><div id="r-ann"></div><div class="actbar" id="r-act"></div>
      <div class="primary" id="r-primary"></div><aside class="panel right" id="r-right" aria-label="Right panel"></aside>
      <section class="panel bottom" id="r-bottom" aria-label="Bottom panel"></section><footer class="statusbar" id="r-status"></footer></div><div id="r-dlg"></div><div id="r-over"></div>`
  }
  const wb = $('#wb')
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay, S.barOver)
  wb.classList.toggle('own', ownBar())
  wb.classList.toggle('bar-open', eff.bar.open && !eff.bar.over)
  wb.classList.toggle('bar-over', eff.bar.over)
  wb.classList.toggle('has-right', eff.right.shown)
  wb.classList.toggle('right-over', eff.right.overlay)
  wb.classList.toggle('has-bottom', eff.bottom.shown)
  wb.classList.toggle('bottom-folded', eff.bottom.folded)
  wb.style.setProperty('--right-w', `${eff.right.size}px`)
  wb.style.setProperty('--bottom-h', `${eff.bottom.size}px`)
  patch($('#r-top'), `${barButton()}<span class="top-grow"></span><div class="pill-slot">${pill()}</div>${topEnd()}${captionButtons()}`)
  patch($('#r-act'), actbar())
  patch($('#r-status'), statusbar())
  patch($('#r-right'), panel('right', eff))
  patch($('#r-bottom'), panel('bottom', eff))
  patch($('#r-ann'), announcement())
  patch($('#r-dlg'), todayDialogs())
  todayDialogsKeep()
  patch($('#r-over'), `<div class="toasts" role="status" aria-live="polite" aria-label="Notices">${toasts()}</div>` + (S.lightbox ? `<div class="lightbox" data-close="1"><img src="${esc(S.lightbox)}" alt="Shared screen"></div>` : ''))
  // The primary area in the activity's layout; switching layouts starts it afresh.
  const primary = $('#r-primary')
  const layout = (ACTIVITIES.find((a) => a.id === S.view) || ACTIVITIES[0]).layout
  if (primary.dataset.layout !== layout || primary.dataset.view !== S.view) {
    primary.innerHTML = layout === 'list-detail' ? '<div class="list-detail" id="r-ld"><div class="region" id="r-list"></div><main class="main" id="r-main"></main></div>' : ''
    primary._html = undefined
    primary.dataset.layout = layout
    primary.dataset.view = S.view
  }
  const pact = ACTIVITIES.find((a) => a.id === S.view && a.plugin)
  if (pact) {
    let html = ''
    try { html = pact.render() } catch (e) { html = `<p class="error">${esc(pact.label)}: ${esc(String((e && e.message) || e))}</p>` }
    patch(primary, `<main class="main plug-act">${html}</main>`)
    try { if (pact.after) pact.after(primary) } catch (e) { console.error(e) }
  } else {
    // Echo's own views draw themselves (their activity's `draw`); Calls, and a view no activity draws, is the list beside the call.
    const act = ACTIVITIES.find((a) => a.id === S.view && a.draw)
    if (act) act.draw(primary)
    else drawCalls()
  }
  fillTranscripts()
  tickClocks()
  // An element that was replaced gets its focus back (one left alone keeps its own selection).
  const again = focus && $('#' + focus)
  if (again && again !== focused) { again.focus(); try { again.setSelectionRange(caret, caret) } catch {} }
  const pane = $('#pane')
  let jumped = false
  if (pane) {
    const j = $('#jump')
    // Scrolled to once; its turn stays marked through later draws of this call (#370: thread data arriving redrew the
    // transcript and lost it).
    if (S.jump != null && j) { j.scrollIntoView({ block: 'center' }); S.jumpShown = { call: S.selected, t: S.jump }; S.jump = null; jumped = true }
    // The mark ends with a search, or once the user leaves that call's transcript.
    else if (S.jumpShown && (S.find.trim() || S.selected !== S.jumpShown.call || S.tab !== 'transcript')) S.jumpShown = null
    else pane.scrollTop = scroll
  }
  const lt2 = $('#live-transcript')
  if (lt2 && !jumped) lt2.scrollTop = ltStick ? lt2.scrollHeight : ltTop
  const nl2 = $('#nt-list')
  if (nl2 && nl2 !== nl) {
    const top = notesScroll(nlWas, { call: nl2.closest('[data-notes-call]').dataset.notesCall, height: nl2.scrollHeight })
    if (top != null) nl2.scrollTop = top
  }
  if (entering) { const p = $('.pane') || $('.grid-wrap'); if (p) { p.classList.remove('enter'); void p.offsetWidth; p.classList.add('enter') } }
  placeDetail()
  placeColMenu()
  pushNotes(false)
  attachTerms(termFocus)
  const g = $('#calgrid')
  if (g) g.addEventListener('scroll', placeDetail, { passive: true })
  lastPrint = fingerprint()
}

/** Put the event card next to the event it belongs to, inside the calendar's grid (never over the activity bar, the
 *  calendar's own head — Day, Week — or another panel): right of it if there is room, else left; neither (a block
 *  across the day), over the block's right part; level with it (#338). */
function placeDetail() {
  const card = $('.detail'), ev = $('.ev[aria-selected="true"]')
  if (!card || !ev) return
  const pane = (ev.closest('#calgrid') || ev.closest('#r-primary') || document.body).getBoundingClientRect()
  const r = ev.getBoundingClientRect(), w = card.offsetWidth, h = card.offsetHeight, gap = 8, pad = 12
  const minX = pane.left + pad, maxX = pane.right - pad - w
  // A grid narrower than the card: kept inside the window instead.
  const left = maxX < minX ? Math.max(pad, Math.min(r.right - w, window.innerWidth - pad - w))
    : r.right + gap <= maxX ? r.right + gap : r.left - gap - w >= minX ? r.left - gap - w : maxX
  const bottom = Math.min(pane.bottom, window.innerHeight)
  let top = Math.min(Math.max(pane.top + pad, r.top), Math.max(pane.top + pad, bottom - h - pad))
  // A grid shorter than the card (a short window): its bottom kept inside the window, as far up as it must.
  if (top + h > window.innerHeight - pad) top = Math.max(pad, window.innerHeight - h - pad)
  card.style.left = `${left}px`
  card.style.right = 'auto'
  card.style.top = `${top}px`
}

// ── fingerprint: what the screen depends on ──────────────────────────────────────────────────
/** What the screen depends on: re-render only when it changes (keeps selection and scroll). */
// (the Inbox too: its badge and open panel; the minute too: the pill's "meeting soon" comes and goes with the clock)
const fingerprint = () => JSON.stringify([S.state, S.calls, S.call, S.agenda, S.cal.data, S.cal.selected, S.settings, S.pj.data, S.pj.detail, S.pj.foundData, S.pj.work, S.pj.series, S.up, S.upPrep, S.today, NY.data, Math.floor(Date.now() / 60000)])
// end of the fingerprint block

// A thread's meeting series (Profile → Meetings): open one; set a field.
function seriesSet(el, field, value) {
  const ed = el.closest('[data-series-key]')
  if (!ed) return
  send({ cmd: 'project', op: 'series_set', key: ed.dataset.seriesKey, subject: ed.dataset.seriesSubject, field, value })
  // The calendar shows it from its own data: read it again.
  if (S.view === 'calendar') setTimeout(() => loadCalendar(true).then(render), 300)
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-series-set],[data-series-unlink]')
  if (!t) return
  if (t.dataset.seriesUnlink) return send({ cmd: 'project', op: 'series_unlink', id: S.pj.sel, key: t.dataset.seriesUnlink })
  // The choice already made: nothing to change (Don't record would erase a rule it only stands for).
  if (t.getAttribute('aria-pressed') === 'true') return
  let v
  try { v = JSON.parse(t.dataset.val) } catch { return }
  seriesSet(t, t.dataset.seriesSet, v)
})
document.addEventListener('change', (e) => {
  const t = e.target.closest && e.target.closest('[data-series-field]')
  if (!t) return
  const f = t.dataset.seriesField
  seriesSet(t, f, f === 'languages' ? (t.value ? [t.value] : []) : t.value || null)
})

// ── clicks: which handler a click on the page runs (no DOM: `clicks.test.mjs`) ───────────────────────────────
/** The page's click handlers, in precedence: `{ sel, area, when(t, e), run(t, e) }` (`area`: the part of the window it
 *  belongs to). */
const CLICKS = []
/** What a click is for: the nearest of its target's ancestors (the target first) that a handler's `sel` matches, and
 *  the first handler, in CLICKS' order, whose `when` holds for that element (`c` null: none does). Null: no ancestor matches. */
const clickFor = (target, e) => {
  const t = target.closest(CLICKS.map((c) => c.sel).join(','))
  return t ? { t, c: CLICKS.find((c) => c.when(t, e)) || null } : null
}
// end of the clicks block

// Sign-ins (`signin.rs`): shown at once (starting / stopping / checking) until the next poll
// brings the app's state.
const busySignin = (id, state) => { const p = profileOf(id); if (p) { p.signin = { state }; render() } }
CLICKS.push(
  { sel: '[data-signin]', area: 'settings', when: (t) => t.dataset.signin, run: (t) => { send({ cmd: 'sign_in', profile: t.dataset.signin }); return busySignin(t.dataset.signin, 'starting') } },
  { sel: '[data-signin-cancel]', area: 'settings', when: (t) => t.dataset.signinCancel, run: (t) => { send({ cmd: 'sign_in_cancel', profile: t.dataset.signinCancel }); return busySignin(t.dataset.signinCancel, 'stopping') } },
  { sel: '[data-signin-accept]', area: 'settings', when: (t) => t.dataset.signinAccept, run: (t) => { send({ cmd: 'accept_account', profile: t.dataset.signinAccept, account: t.dataset.account }); return busySignin(t.dataset.signinAccept, 'checking') } },
  { sel: '[data-signin-check]', area: 'settings', when: (t) => t.dataset.signinCheck, run: (t) => { send({ cmd: 'check_sign_in', profile: t.dataset.signinCheck }); return busySignin(t.dataset.signinCheck, 'checking') } },
  { sel: 'a[data-ext]', area: 'page', when: (t) => t.dataset.ext !== undefined, run: (t, e) => { e.preventDefault(); send({ cmd: 'open_url', url: t.getAttribute('href') }); return } },
  // A plugin's work row's page (https only), in the browser once the user confirms it (its host asked first).
  { sel: '[data-open-url]', area: 'page', when: (t) => /^https:\/\//.test(t.dataset.openUrl || ''), run: (t, e) => { e.preventDefault(); return workRowOpen(t.dataset.openUrl) } },
  { sel: '[data-open-url-no]', area: 'page', when: (t) => t.dataset.openUrlNo !== undefined, run: (t, e) => { e.preventDefault(); S.wrOpen = null; return render() } },
  // Settings
  { sel: '[data-me-edit]', area: 'settings', when: (t) => t.dataset.meEdit !== undefined, run: (t) => { S.meEdit = (S.settings && S.settings.me_profile && S.settings.me_profile.text) || ''; render(); const ta = $('#me-profile'); if (ta) ta.focus(); return } },
  { sel: '[data-me-cancel]', area: 'settings', when: (t) => t.dataset.meCancel !== undefined, run: (t) => { S.meEdit = null; return render() } },
  { sel: '[data-me-save]', area: 'settings', when: (t) => t.dataset.meSave !== undefined, run: (t) => saveMeProfile() },
  { sel: '[data-setsec]', area: 'settings', when: (t) => t.dataset.setsec, run: (t) => go(`settings/${t.dataset.setsec}`) },
  { sel: '[data-set]', area: 'settings', when: (t) => t.dataset.set, run: (t) => {
    let val; try { val = JSON.parse(t.dataset.val) } catch { val = t.dataset.val }
    return setSetting(t.dataset.set, val)
  } },
  { sel: '[data-announce]', area: 'shell', when: (t) => t.dataset.announce !== undefined, run: (t) => { const a = S.state.announcement; S.state.announcement = null; send({ cmd: 'announcement', id: a.id }); if (t.dataset.announce) send({ cmd: 'settings_action', action: t.dataset.announce, arg: t.dataset.arg || '' }); return render() } },
  { sel: '[data-act]', area: 'settings', when: (t) => t.dataset.act, run: (t) => { send({ cmd: 'settings_action', action: t.dataset.act, arg: t.dataset.arg || '' }); setTimeout(() => loadSettings().then(render), 1500); return } },
  { sel: '[data-lang-del]', area: 'settings', when: (t) => t.dataset.langDel, run: (t) => { const l = (S.settings.languages || []).filter((x) => x !== t.dataset.langDel); S.settings.languages = l; send({ cmd: 'settings_set', key: 'languages', value: l }); return render() } },
  { sel: '[data-vocab-del]', area: 'settings', when: (t) => t.dataset.vocabDel, run: (t) => { const v = (S.settings.vocabulary || []).filter((x) => x !== t.dataset.vocabDel); S.settings.vocabulary = v; send({ cmd: 'settings_set', key: 'vocabulary', value: v }); return render() } },
  { sel: '[data-theme-set]', area: 'settings', when: (t) => t.dataset.themeSet !== undefined, run: (t) => Theme.set(t.dataset.themeSet) },
  { sel: '[data-layout-move]', area: 'settings', when: (t) => t.dataset.layoutMove, run: (t) => { Layout.move(t.dataset.layoutMove, t.dataset.to, PANEL_VIEWS); return layoutChanged() } },
  { sel: '[data-layout-reset]', area: 'settings', when: (t) => t.dataset.layoutReset !== undefined, run: (t) => { Layout.load({}); return layoutChanged() } },
  { sel: '[data-term-output]', area: 'terminals', when: (t) => t.dataset.termOutput, run: (t) => { S.termOutput = S.termOutput || {}; S.termOutput[t.dataset.termOutput] = !S.termOutput[t.dataset.termOutput]; return render() } },
  // A Claude panel's restart / new conversation (#44): asked first while it runs.
  { sel: '[data-br-pclosed]', area: 'brain', when: (t) => t.dataset.brPclosed !== undefined, run: (t) => { S.br.pClosed = !S.br.pClosed; return render() } },
  { sel: '[data-conv-keep]', area: 'terminals', when: (t) => t.dataset.convKeep !== undefined, run: (t) => { S.convAsk = null; return render() } },
  { sel: '[data-conv],[data-conv-go]', area: 'terminals', when: (t) => t.dataset.conv || t.dataset.convGo !== undefined, run: (t) => {
    const extra = t.dataset.termCall ? { call: t.dataset.termCall } : { profile: S.folderProfile || ai().default || undefined }
    const key = t.dataset.termKey
    const v = t.dataset.convGo !== undefined ? t.dataset.convGo : t.dataset.conv === 'restart' ? '' : t.dataset.conv
    const e = Terms.get(key)
    // A running session: asked first (the confirmation's own button goes on).
    if (t.dataset.convGo === undefined && e && e.id != null && !e.exited) { S.convAsk = { key, session: v }; return render() }
    S.convAsk = null
    return termRestart(t.dataset.termKind, key, extra, v)
  } },
  { sel: '[data-term-start]', area: 'terminals', when: (t) => t.dataset.termStart, run: (t) => {
    const sel = t.closest('.panel-body') && t.closest('.panel-body').querySelector('[data-term-profile]')
    if (sel) S.folderProfile = sel.value
    return termStart(t.dataset.termStart, t.dataset.termKey, { call: t.dataset.termCall, profile: sel ? sel.value : undefined })
  } },
  { sel: '[data-note-edit]', area: 'notes', when: (t) => t.dataset.noteEdit !== undefined, run: (t) => {
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
  } },
  { sel: '[data-edit-kind]', area: 'notes', when: (t) => t.dataset.editKind, run: (t) => { S.noteEdit.kind = t.dataset.editKind; render(); const ta = $('#note-edit'); if (ta) ta.focus(); return } },
  { sel: '[data-note-kind]', area: 'notes', when: (t) => t.dataset.noteKind, run: (t) => { S.noteKind = t.dataset.noteKind; render(); const el = $('#note-input'); if (el) el.focus(); return } },
  { sel: '[data-note-done]', area: 'notes', when: (t) => t.dataset.noteDone !== undefined, run: (t) => { const b = NoteDoc.parse(NotesSync.view(S.call.name).draft)[Number(t.dataset.noteDone)]; return editNotes((text) => NoteDoc.change(text, Number(t.dataset.noteDone), { done: !(b && b.done) })) } },
  { sel: '[data-note-del]', area: 'notes', when: (t) => t.dataset.noteDel !== undefined, run: (t) => editNotes((text) => NoteDoc.change(text, Number(t.dataset.noteDel), null)) },
  { sel: '[data-panel-flip]', area: 'shell', when: (t) => t.dataset.panelFlip, run: (t) => {
    // A view's own button: show it, or close its panel when it is what the panel shows.
    const id = t.dataset.panelFlip, known = panelViews(), a = Layout.area(id, known)
    const eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
    const open = a && eff[a].shown && eff[a].active === id
    // The Inbox opened: read again (a run may have ended since the last poll, #121).
    if (id === 'needs' && !open) loadAttention().then(render)
    if (eff.narrow && a === 'right') { S.overlay = !open; if (!open) Layout.show(id, known); return render() }
    if (open) Layout.toggle(a); else Layout.show(id, known)
    return layoutChanged()
  } },
  { sel: '[data-panel-show]', area: 'shell', when: (t) => t.dataset.panelShow, run: (t) => { Layout.show(t.dataset.panelShow, panelViews()); return layoutChanged() } },
  { sel: '[data-panel-move]', area: 'shell', when: (t) => t.dataset.panelMove, run: (t) => { Layout.move(t.dataset.panelMove, t.dataset.to, panelViews()); return layoutChanged() } },
  { sel: '[data-panel-toggle]', area: 'shell', when: (t) => t.dataset.panelToggle, run: (t) => {
    if (S.overlay && t.dataset.panelToggle === 'right') {
      if (Layout.effective(window.innerWidth, window.innerHeight, panelViews()).narrow) return closeOverlay()
      S.overlay = false // widened since: the docked panel's Close closes it below
    }
    Layout.toggle(t.dataset.panelToggle); return layoutChanged()
  } },
  { sel: '[data-go]', area: 'shell', when: (t) => t.dataset.go, run: (t) => { S.barOver = false; return goActivity(t.dataset.go) } },
  { sel: '[data-ev]', area: 'calendar', when: (t) => t.dataset.ev !== undefined, run: (t) => { S.cal.selected = S.cal.selected === t.dataset.ev ? null : t.dataset.ev; return render() } },
  { sel: '[data-calclose]', area: 'calendar', when: (t) => t.dataset.calclose, run: (t) => { S.cal.selected = null; return render() } },
  { sel: '[data-calmode]', area: 'calendar', when: (t) => t.dataset.calmode, run: (t) => { S.cal.mode = t.dataset.calmode; S.cal.offset = 0; S.cal.data = null; go(`calendar/${S.cal.mode}`); return loadCalendar().then(render) } },
  { sel: '[data-calnav]', area: 'calendar', when: (t) => t.dataset.calnav !== undefined, run: (t) => { const n = Number(t.dataset.calnav); S.cal.offset = n === 0 ? 0 : S.cal.offset + n; S.cal.data = null; render(); return loadCalendar().then(render) } },
  { sel: '[data-cal-import]', area: 'calendar', when: (t) => t.dataset.calImport, run: (t) => { send({ cmd: 'calendar_import', id: t.dataset.calImport }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.calImport); if (e) e.import = { busy: true }; return render() } },
  { sel: '[data-schedule]', area: 'calendar', when: (t) => t.dataset.schedule, run: (t) => { send({ cmd: 'schedule', id: t.dataset.schedule, languages: recordLang() }); const e = ((S.cal.data && S.cal.data.events) || []).find((x) => x.id === t.dataset.schedule); if (e) e.scheduled = true; if (S.view === 'today') setTimeout(() => loadToday().then(render), 400); return render() } },
  { sel: '[data-unschedule]', area: 'calendar', when: (t) => t.dataset.unschedule, run: (t) => { send({ cmd: 'unschedule', id: t.dataset.unschedule }); const e = ((S.cal.data && S.cal.data.events) || []).find((x) => x.id === t.dataset.unschedule); if (e) e.scheduled = false; if (S.view === 'today') setTimeout(() => loadToday().then(render), 400); return render() } },
  { sel: '[data-call]', area: 'calls', when: (t) => t.dataset.call, run: (t) => { S.listOpen = false; return go(`call/${t.dataset.call}/${t.dataset.callTab || S.tab}`) } },
  { sel: '[data-up-fold]', area: 'calls', when: (t) => t.dataset.upFold !== undefined, run: (t) => { S.upOpen = !S.upOpen; return render() } },
  { sel: '[data-up]', area: 'calls', when: (t) => t.dataset.up, run: (t) => { S.listOpen = false; return go(`upcoming/${t.dataset.up}`) } },
  { sel: '[data-today-import]', area: 'today', when: (t) => t.dataset.todayImport, run: (t) => {
    send({ cmd: 'calendar_import', id: t.dataset.todayImport })
    // Read again until it is in: the import runs in the background (about a minute).
    let n = 0
    const poll = () => loadToday().then(() => { render(); const x = ((S.today && S.today.day) || []).find((d) => d.event === t.dataset.todayImport); if (x && !x.call && (!x.import || x.import.busy) && ++n < 60) setTimeout(poll, 3000) })
    setTimeout(poll, 600)
    return
  } },
  { sel: '[data-today-retry]', area: 'today', when: (t) => t.dataset.todayRetry, run: (t) => {
    send({ cmd: 'write_notes', call: t.dataset.todayRetry })
    setTimeout(() => loadToday().then(render), 800)
    return
  } },
  { sel: '[data-needs-open]', area: 'today', when: (t) => t.dataset.needsOpen !== undefined && t.dataset.needsOpen !== null && (t.dataset.needsOpen || t.dataset.chat), run: (t) => {
    // The Teams app on that message (its link as msteams:), else the chat.
    const u = t.dataset.needsOpen || `msteams:/l/chat/${encodeURIComponent(t.dataset.chat)}/0`
    return send({ cmd: 'open_url', url: u })
  } },
  // Won't do: a dialog asks why, or who it's for (needed); confirmed, Claude notes it in the background (Asks).
  { sel: '[data-needs-wont]', area: 'today', when: (t) => t.dataset.needsWont, run: (t) => { S.needsDlg = { kind: 'wont', id: t.dataset.needsWont, note: '' }; render(); const n = $('#nd-note'); if (n) n.focus(); return } },
  { sel: '[data-needs-wont-ok]', area: 'today', when: (t) => t.dataset.needsWontOk, run: (t) => {
    const id = t.dataset.needsWontOk, note = ((S.needsDlg && S.needsDlg.note) || '').trim()
    if (!note) { const n = $('#nd-note'); if (n) n.focus(); return }
    S.needsDlg = null
    if (S.today && S.today.needs) S.today.needs.items = S.today.needs.items.filter((x) => x.id !== id)
    todayWont(id, note)
    return render()
  } },
  { sel: '[data-wont-edit]', area: 'today', when: (t) => t.dataset.wontEdit, run: (t) => { const a = ((S.today && S.today.asks) || []).find((x) => x.id === t.dataset.wontEdit); S.wontEdit = { id: t.dataset.wontEdit, text: (a && a.draft && a.draft.text) || '' }; render(); const n = $('#wd-text'); if (n) n.focus(); return } },
  { sel: '[data-wont-send]', area: 'today', when: (t) => t.dataset.wontSend, run: (t) => {
    const id = t.dataset.wontSend, a = ((S.today && S.today.asks) || []).find((x) => x.id === id)
    const text = (S.wontEdit && S.wontEdit.id === id ? S.wontEdit.text : (a && a.draft && a.draft.text) || '').trim()
    if (!text) { const n = $('#wd-text'); if (n) n.focus(); return }
    // The box stays as typed until it is sent (a failed send keeps the user's words); a sent line has no box.
    send({ cmd: 'wont_send', ask: id, text })
    setTimeout(() => loadToday().then(render), 800)
    return render()
  } },
  { sel: '[data-wont-drop]', area: 'today', when: (t) => t.dataset.wontDrop, run: (t) => {
    const id = t.dataset.wontDrop
    send({ cmd: 'ask_drop', ask: id })
    if (S.wontEdit && S.wontEdit.id === id) S.wontEdit = null
    if (S.today && S.today.asks) S.today.asks = S.today.asks.filter((x) => x.id !== id)
    return render()
  } },
  { sel: '[data-needs-todo]', area: 'today', when: (t) => t.dataset.needsTodo, run: (t) => { S.needsDlg = { kind: 'todo', id: t.dataset.needsTodo }; return render() } },
  // ✦: My work's Claude dialog on the item itself (no task made) — Work on it or Yolo.
  { sel: '[data-needs-claude]', area: 'today', when: (t) => t.dataset.needsClaude, run: (t) => {
    const item = (((S.today && S.today.needs) || {}).items || []).find((x) => x.id === t.dataset.needsClaude)
    if (!item) return
    S.claudeSend = { needs: item.id, project: item.thread || '', title: `${item.where || ''} — ${item.line || ''}`, today: true, open: true }
    if (!TPL.data) loadTemplates().then(render)
    return render()
  } },
  { sel: '[data-needs-todo-ok]', area: 'today', when: (t) => t.dataset.needsTodoOk, run: (t) => {
    const id = t.dataset.needsTodoOk, thread = ($('#nd-thread') || {}).value, title = ($('#nd-title') || {}).value
    if (!thread) { const el = $('#nd-thread'); if (el) el.focus(); return }
    send({ cmd: 'needs_decide', id, how: 'todo', thread, title })
    S.needsDlg = null
    if (S.today && S.today.needs) S.today.needs.items = S.today.needs.items.filter((x) => x.id !== id)
    setTimeout(() => loadToday().then(render), 800)
    return render()
  } },
  { sel: '[data-needs-read]', area: 'today', when: (t) => t.dataset.needsRead, run: (t) => {
    const item = ((S.today.needs || {}).items || []).find((x) => x.id === t.dataset.needsRead)
    const c = S.needsDlg = { kind: 'mail', id: t.dataset.needsRead, message: item && item.message, mail: null }
    get('api/needs/mail?id=' + encodeURIComponent(c.message || '')).then((m) => { if (S.needsDlg === c) { c.mail = m; render() } })
    return render()
  } },
  { sel: '[data-needs-send]', area: 'today', when: (t) => t.dataset.needsSend, run: (t) => {
    const c = S.needsDlg, text = (($('#nd-reply') || {}).value || '').trim()
    if (!c || !text) return
    send({ cmd: 'needs_reply', id: c.id, message: c.message, text, all: t.dataset.all === '1' })
    S.needsDlg = null
    if (S.today && S.today.needs) S.today.needs.items = S.today.needs.items.filter((x) => x.id !== c.id)
    return render()
  } },
  { sel: '[data-needs-outlook]', area: 'today', when: (t) => t.dataset.needsOutlook, run: (t) => send({ cmd: 'open_url', url: t.dataset.needsOutlook }) },
  { sel: '[data-needs-off]', area: 'today', when: (t, e) => t.dataset.needsOff && (t.tagName === 'BUTTON' || e.target === t), run: (t) => { S.needsDlg = null; return render() } },
  { sel: '[data-today-askoff]', area: 'today', when: (t) => t.dataset.todayAskoff, run: (t) => { S.todayAskId = null; return render() } },
  { sel: '[data-today-close]', area: 'today', when: (t) => t.dataset.todayClose, run: (t) => {
    S.todayClose = { project: t.dataset.project, node: t.dataset.todayClose, title: t.dataset.title }
    render()
    const n = $('#td-close-note'); if (n) n.focus()
    return
  } },
  // Cancel, or a click on the dimmed page around the dialog (not inside it).
  { sel: '[data-today-keep]', area: 'today', when: (t, e) => t.dataset.todayKeep && (t.tagName === 'BUTTON' || e.target === t), run: (t) => { S.todayClose = null; return render() } },
  { sel: '[data-today-done]', area: 'today', when: (t) => t.dataset.todayDone, run: (t) => {
    const note = ($('#td-close-note') || {}).value || ''
    S.todayClose = null
    send({ cmd: 'project', op: 'ledger', id: t.dataset.project, body: { op: 'edit', node: t.dataset.todayDone, set: { state: 'done' }, note: note.trim() || undefined } })
    // Gone from My work at once; Today is read again once the ledger has it.
    if (S.today && S.today.work) for (const k of ['mine', 'owed', 'waiting']) if (S.today.work[k]) S.today.work[k].items = S.today.work[k].items.filter((x) => !(x.id === t.dataset.todayDone && x.project === t.dataset.project))
    setTimeout(() => loadToday().then(render), 800)
    return render()
  } },
  { sel: '[data-today-seen]', area: 'today', when: (t) => t.dataset.todaySeen, run: (t) => { send({ cmd: 'today_seen', key: t.dataset.todaySeen }); if (S.today) S.today.cards = (S.today.cards || []).filter((c) => c.key !== t.dataset.todaySeen); return render() } },
  { sel: '[data-prep-write]', area: 'prep', when: (t) => t.dataset.prepWrite, run: (t) => {
    send({ cmd: 'project', op: 'prep_write', prep: t.dataset.prepWrite })
    // From the calendar: open it in Calls, where it shows as it is written.
    if (t.dataset.prepOpen) { S.cal.selected = null; return go(`upcoming/${t.dataset.prepWrite}`) }
    setTimeout(() => loadUpcoming().then(render), 800); return
  } },
  { sel: '[data-ev-prep]', area: 'prep', when: (t) => t.dataset.evPrep, run: (t) => {
    // A recorded meeting's prep is its call's Prep tab; one still to come is in Coming up today.
    if (t.dataset.evCall) return go(`call/${t.dataset.evCall}/prep`)
    const ev = S.cal.data && (S.cal.data.events || []).find((x) => x.prep && x.prep.id === t.dataset.evPrep)
    S.cal.selected = null
    return ev && ev.recorded ? go(`call/${ev.recorded.name}/prep`) : go(`upcoming/${t.dataset.evPrep}`)
  } },
  { sel: '[data-minutes-of]', area: 'calls', when: (t) => t.dataset.minutesOf, run: (t) => go(`call/${t.dataset.minutesOf}/notes`) },
  { sel: '[data-tab]', area: 'call', when: (t) => t.dataset.tab, run: (t) => go(`call/${S.selected}/${t.dataset.tab}`) },
  { sel: '[data-at]', area: 'call', when: (t) => t.dataset.at, run: (t) => { S.jump = Number(t.dataset.at); S.find = ''; return go(`call/${S.selected}/transcript`) } },
  { sel: '[data-frame]', area: 'call', when: (t) => t.dataset.frame, run: (t) => { S.lightbox = t.dataset.frame; return render() } },
  { sel: '[data-close]', area: 'call', when: (t) => t.dataset.close, run: (t) => { S.lightbox = null; return render() } },
  { sel: '[data-copy]', area: 'page', when: (t) => t.dataset.copy, run: (t) => { send({ cmd: 'copy', text: t.dataset.copy }); t.textContent = 'Copied'; return } },
  { sel: '[data-arm]', area: 'calls', when: (t) => t.dataset.arm !== undefined, run: (t) => send({ cmd: 'arm_meeting', id: t.dataset.arm, languages: recordLang() }) },
  { sel: '[data-join]', area: 'calendar', when: (t) => t.dataset.join !== undefined, run: (t) => send({ cmd: 'join_meeting', id: t.dataset.join, languages: recordLang() }) },
  // The `data-cmd` buttons: recording, the window's own, the open call's. No case: nothing.
  { sel: '[data-cmd]', area: 'commands', when: (t) => t.dataset.cmd !== undefined, run: (t) => {
    switch (t.dataset.cmd) {
      case 'record': return send({ cmd: 'record', languages: recordLang() })
      case 'record-memo': return send({ cmd: 'record_memo', languages: recordLang() })
      case 'stop': S.stopAsk = false; render(); return send({ cmd: 'stop' })
      // A memo stops at once (nothing else is recorded); a call asks first.
      case 'stop-ask': if ((S.state || {}).recording_memo) { S.stopAsk = false; render(); return send({ cmd: 'stop' }) } S.stopAsk = true; return render()
      case 'stop-keep': S.stopAsk = false; return render()
      case 'list-toggle': S.listOpen = !S.listOpen; return render()
      case 'call-menu': S.menuOpen = !S.menuOpen; return render()
      case 'later-menu': S.laterOpen = !S.laterOpen; return render()
      case 'later-15': S.laterOpen = false; render(); return send({ cmd: 'minutes_later', call: S.selected, minutes: 15 })
      case 'later-manual': S.laterOpen = false; render(); return send({ cmd: 'minutes_later', call: S.selected, minutes: null })
      case 'later-clear': return send({ cmd: 'minutes_later', call: S.selected, minutes: 0 })
      case 'claude-panel': return claudePanel()
      // The account chip: open the Claude session, never fold it (the status bar's Claude toggles).
      case 'claude-open': return claudePanel('claude-folder')
      case 'bar-flip': return flipBar()
      case 'win-min': return send({ cmd: 'window_min' })
      case 'win-max': return send({ cmd: 'window_zoom' })
      case 'win-close': return send({ cmd: 'close_request' })
      case 'ask-claude': return askClaudeAboutCall()
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
        // A choice after a policy file Echo couldn't read (#404): another profile, or without that file.
        const pid = t.dataset.profile || (S.notesProfile && S.notesProfile.call === call && S.notesProfile.id ? S.notesProfile.id : null)
        const without = t.dataset.withoutPolicy || undefined
        // A note typed or edited and not added yet is part of them.
        if (commitPendingNotes()) render()
        NotesSync.flush(call).then(() => {
          send({ cmd: 'write_notes', call, profile: pid || undefined, without_policy: without })
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
        if (sameCall(S.noteDraftCall, name)) { S.noteDraft = ''; S.noteAt = null; S.noteAtOk = null; S.noteDraftCall = null }
        if (S.noteEdit && sameCall(S.noteEdit.call, name)) S.noteEdit = null
        S.calls = (S.calls || []).filter((c) => c.name !== name)
        send({ cmd: 'delete', call: name })
        return go('calls')
      }
    }
  } },
)

document.addEventListener('click', (e) => {
  // Any click closes the call's ⋯ menu (its own button toggles it).
  const menuWas = S.menuOpen
  if (menuWas && !(e.target.closest && e.target.closest('[data-cmd="call-menu"]'))) S.menuOpen = false
  if (S.laterOpen && !(e.target.closest && e.target.closest('[data-cmd^="later-"]'))) { S.laterOpen = false; render() }
  // The sidebar open over a narrow page closes on a click beside it.
  if (S.barOver && !(e.target.closest && e.target.closest('.actbar'))) { S.barOver = false; render() }
  const hit = clickFor(e.target, e)
  if (!hit) { if (menuWas) render(); return }
  if (hit.c) hit.c.run(hit.t, e)
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
async function stampNotes() {
  let ta = $('#my-notes')
  if (!ta || !S.call || !S.call.started_at || ta.readOnly || ta.dataset.notesCall !== S.call.name) return
  const call = ta.dataset.notesCall
  // The call's time at the click, inserted only once the app confirms it records the call: after hang-up the window
  // may still show it recording for a few seconds (#377, as a note's time, #358).
  const t = (Date.now() - new Date(S.call.started_at)) / 1000
  const ok = await within(askRecording(get, call).then((r) => r.ok), 500)
  ta = $('#my-notes')
  if (!ta || ta.readOnly || ta.dataset.notesCall !== call) return
  if (noteTime(t, ok) == null) return window.echo.notice(ok === false ? 'The call has ended: no call time to insert.' : "Couldn't confirm the call is recording: no call time inserted.")
  const stamp = `[${clock(t)}] `
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

/** One plain setting, as Settings' controls and the top bar's account chip set it: shown at once (the next snapshot
 *  confirms it), then saved by the app. */
function setSetting(key, val) {
  if (S.settings && key.startsWith('learned_')) {
    S.settings.learned = (S.settings.learned || []).filter((x) => x.term !== val)
    if (key === 'learned_always') S.settings.vocabulary = [...(S.settings.vocabulary || []), val]
  } else if (S.settings) S.settings[key] = val
  if (key === 'live_notes' && S.state && S.state.ai) S.state.ai.live_notes = val
  if (key === 'default_profile' && S.state && S.state.ai) S.state.ai.default = val
  if (key === 'own_title_bar' && S.state) S.state.own_title_bar = !!val
  if (key === 'brain_enabled') {
    if (S.state) S.state.brain = !!val
    if (S.settings && S.settings.brain) S.settings.brain.enabled = !!val
    setTimeout(() => loadSettings().then(render), 400)
  }
  send({ cmd: 'settings_set', key, value: val })
  return render()
}

// macOS, and Windows with Echo's own title bar: the top strip is the window's title bar (`window/mod.rs`): dragging
// it moves the window, a double click zooms (maximises) it — anywhere but on its controls.
const onTitleBar = (e) => (MAC || ownBar()) && e.button === 0 && e.target.closest && e.target.closest('#r-top') && !e.target.closest('button,select,label,a,input,.pill-note,[data-resize-top]')
document.addEventListener('mousedown', (e) => {
  // Windows, Echo's own title bar: the strip's top pixels resize the window (WebView2 covers the window's own border).
  const edge = e.button === 0 && e.target.closest && e.target.closest('[data-resize-top]')
  if (edge) { e.preventDefault(); return send({ cmd: 'window_resize_top', edge: edge.dataset.resizeTop }) }
  if (onTitleBar(e) && e.detail === 1) { e.preventDefault(); send({ cmd: 'window_drag' }) }
})
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

