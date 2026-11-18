// ── the thread editor: a call's threads, and the edits of a thread, a client and their work ──────────────────────
// Claude about a thread or a client: the Echo-folder session (it has the Brain); the window tells it
// which client or thread "this" is (`<echo_view>`), so its searches are scoped.
document.addEventListener('click', (e) => {
  if (!(e.target.closest && e.target.closest('[data-claude-about]'))) return
  S.pj.menu = false
  claudePanel('claude-folder')
})

/** A call's projects in its header: chips, the suggestion bar, and the picker. */
/** A memo's span for a thread, as the chip shows it (mm:ss–mm:ss); several joined. */
const memoSpans = (c, id) => (c.memo_parts || []).filter((x) => x.thread === id).map((x) => `${clock(x.from)}–${clock(x.to)}`).join(', ')
function callProjects(c) {
  const chips = (c.projects || []).map((p) => {
    const span = c.memo ? memoSpans(c, p.id) : ''
    // A memo's threads were found from what was said: × takes it out for good (the next split leaves it out too).
    const x = c.memo ? `<button class="pj-x" data-pj="member" data-id="${esc(p.id)}" data-call-name="${esc(c.name)}" data-how="exclude" title="Not this — never put here again" aria-label="Not ${esc(p.name)}">×</button>` : ''
    return `<span class="pj-chip memo-pj"><button class="link-btn" data-pj="goto" data-id="${esc(p.id)}" title="${esc(c.memo && span ? 'Found from what you said' : (WHY[p.why] || p.why))}">${projIcon(p, 14)}${esc(p.name)}</button>${span ? `<span class="set-sub">${esc(span)}</span>` : ''}${x}</span>`
  }).join('')
  const all = (S.pj.data && S.pj.data.projects) || []
  const mine = new Set((c.projects || []).map((p) => p.id))
  const picker = S.pj.picker ? `<div class="menu pj-menu" role="menu">${all.filter((p) => p.group !== 'closed').map((p) => `<button class="menu-item" role="menuitemcheckbox" aria-checked="${mine.has(p.id)}" data-pj="member" data-id="${esc(p.id)}" data-call-name="${esc(c.name)}" data-how="${mine.has(p.id) ? 'exclude' : 'include'}">${mine.has(p.id) ? '✓ ' : ''}${projIcon(p, 14)} ${esc(p.name)}</button>`).join('') || '<div class="menu-item set-sub">No threads yet</div>'}<div class="menu-sep" role="separator"></div><button class="menu-item" role="menuitem" data-pj="from-call" data-call-name="${esc(c.name)}">New thread from this call…</button></div>` : ''
  if (c.projects_error) return `<div class="pj-chips"><span class="set-sub">Projects: ${esc(c.projects_error)}</span></div>`
  return `<div class="pj-chips">${chips}<span class="pj-add-wrap"><button class="pj-chip add" data-pj="picker" aria-haspopup="menu" aria-expanded="${S.pj.picker}">+ Thread</button>${picker}</span></div>`
}
/** A thread the call may belong to, asked under the call's header (the header's full width, buttons included). */
function callSuggestion(c) {
  // Threads that can't be read offer nothing to join (`callProjects` says why).
  const sug = !c.projects_error && (c.suggested || [])[0]
  return sug ? `<div class="pj-sug">${projIcon(sug, 16)}<span class="grow">Part of <b>${esc(sug.name)}</b>? <span class="set-sub">${esc(sug.why)}</span></span><button class="btn primary" data-pj="member" data-id="${esc(sug.id)}" data-call-name="${esc(c.name)}" data-how="include">Add</button><button class="btn ghost" data-pj="member" data-id="${esc(sug.id)}" data-call-name="${esc(c.name)}" data-how="exclude">Not this</button></div>` : ''
}


// ── editor focus: an editor opened takes the focus; a list opens, a box's text is selected so typing replaces it
function focusEditor(i, open) {
  i.focus()
  if (i.tagName === 'SELECT') open(i)
  else if (i.select) try { i.select() } catch {}
}
// end of the editor focus block
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
    // A date box opens without its calendar: an open calendar takes Enter and picks today (#303); its icon opens it.
    case 'edit-field': S.pj.editField = d.field; render(); { const i = document.querySelector('.wp [autofocus]'); if (i) focusEditor(i, smOpen) } return // a list opens at once, as the other fields' menus do; a box's text is selected, so typing replaces it
    case 'edit-title': S.pj.editField = 'title'; render(); { const i = document.querySelector('.wp-title'); if (i) { i.focus(); i.select() } } return
    case 'sync': send({ cmd: 'project', op: 'sync', id: S.pj.sel }); if (S.pj.work) S.pj.work.sync = { state: 'running', message: 'reading calls' }; return render()
    case 'ledger': return ledgerEdit(JSON.parse(d.body), d.project || S.pj.sel)
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
    case 'col-add': { const bd = boardOf(); const id = 'c' + Date.now().toString(36); bd.columns = [...bd.columns, { id, name: 'New column', color: COL_COLORS[bd.columns.length % COL_COLORS.length], state: 'open' }]; S.pj.colMenu = id; saveBoard(bd); { const b = $('.bd'); if (b) { b.scrollLeft = b.scrollWidth; placeColMenu() } } { const i = $('#bd-col-name'); if (i) { i.focus(); i.select() } } return } // the new column scrolled into view, so its menu shows (#109)
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
    // Work's tab first: the item's Details panel exists only on Work, and the route changes on a later hashchange (#314).
    case 'goto-node': { S.pj.node = d.node; S.pj.wantNode = d.node; S.pj.inline = null; S.pj.tab = 'work'; go(`project/${d.id}/work`); Layout.show('work-item', panelViews()); return layoutChanged() }
    case 'tab': return go(`project/${S.pj.sel}/${d.tabId}`)
    case 'toggle-closed': S.pj.showClosed = !S.pj.showClosed; return render()
    // Drawn again: the keyboard stays on the fold's head.
    case 'toggle-idle-clients': { S.pj.showIdle = !S.pj.showIdle; render(); const h = $('[data-pj="toggle-idle-clients"]'); if (h) h.focus(); return }
    case 'review-fold': { S.pj.reviewFolded = !S.pj.reviewFolded; render(); const h = $('[data-pj="review-fold"]'); if (h) h.focus(); return }
    case 'fold': {
      pjFold(d.id); render()
      // The sidebar was drawn again: the keyboard stays on this chevron.
      const again = [...document.querySelectorAll('.pj-chev[data-pj="fold"]')].find((x) => x.dataset.id === d.id)
      if (again) again.focus()
      return
    }
    case 'stage': return projectSend('stage', { stage: d.stage })
    case 'stage-back': return projectSend('stage_back', { plugin: d.plugin })
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
    case 'brief-edit': { const k = S.pj.client || `thread:${S.pj.sel}`; if (!aboutIsOpen(k)) aboutFold(k) } // what was edited stays open
      S.pj.briefEdit = (S.pj.client ? S.pj.cdata && S.pj.cdata.brief : S.pj.detail && S.pj.detail.brief) || ''; render(); { const t = $('#pj-brief'); if (t) t.focus() } return
    case 'brief-cancel': S.pj.briefEdit = null; return render()
    case 'about-fold': aboutFold(d.client); render(); { const b = document.querySelector('[data-pj="about-fold"]'); if (b) b.focus() } return
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
    // A date box fires `change` for each part typed (a year half typed is a date too): typed, it is saved once, on
    // Enter or when left (#303); a day picked in its calendar, at once.
    if (el.type === 'date') return el.dataset.typed ? undefined : saveDateField(el)
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
    if (what === 'mail') {
      // The address field and the subject field: Enter in either adds the source with both.
      const who = (($('#pj-add-mail') || {}).value || '').trim(), subject = (($('#pj-add-mail-subject') || {}).value || '').trim()
      // No address yet (Enter in the subject field first): the form stays, the words kept.
      if (!who) { S.pj.adding = 'mail'; return }
      projectSend('source_add', { kind: 'mail', source: who, subject }); setTimeout(() => loadSourceState(true), 1500)
    }
    if (what === 'exclude') { projectSend('source_exclude', { source: el.dataset.root, exclude: v.split(',').map((g) => g.trim()).filter(Boolean) }); setTimeout(() => loadSourceState(true), 1500) }
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
  if (el.dataset && el.dataset.ledgerSet && el.type === 'date') {
    if (e.key === 'Enter') { e.preventDefault(); return saveDateField(el) }
    if (e.key !== 'Escape' && e.key !== 'Tab') el.dataset.typed = '1'
  }
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
/** A work item's date box, done with (Enter, or left): its date saved once, when it changed and is one a work item can
 *  be due on; an emptied box clears a date it had, a half-typed one (a part cleared) changes nothing. */
function saveDateField(el) {
  const k = el.dataset.ledgerSet, n = S.pj.work && S.pj.work.nodes[S.pj.node]
  if (S.pj.editField !== k || !n) return
  S.pj.editField = null
  const v = el.value, year = Number(v.slice(0, 4))
  if (el.validity.badInput || v === (n[k] || '') || (v && !(year >= 2000 && year <= 2100))) return render()
  ledgerEdit({ op: 'edit', node: S.pj.node, set: { [k]: v || null } })
}
// A work item's field opened for a change and left as it was (or Esc): back to reading it.
document.addEventListener('focusout', (e) => {
  if (!(e.target.closest && e.target.closest('.wp') && e.target.dataset.ledgerSet)) return
  if (e.target.type === 'date') return saveDateField(e.target)
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
  const nodes = S.pj.work && S.pj.work.nodes
  const text = de.text.trim(), x = nodes && Object.hasOwn(nodes, de.node) ? nodes[de.node] : null
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

