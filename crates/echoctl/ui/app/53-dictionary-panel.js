// ── the dictionary's term panel (Settings → Vocabulary): a term, its edits and its latest sections ──────────────
/** "Hooli workshop · closed 28 Oct" — a closed thread as the archive names it. */
function closedLabel(t) {
  const how = t.stage === 'lost' ? 'lost' : t.stage === 'cancelled' ? 'cancelled' : 'closed'
  const d = t.closed_at ? new Date(t.closed_at) : null
  const opts = d && d.getFullYear() !== new Date().getFullYear() ? { day: 'numeric', month: 'short', year: 'numeric' } : { day: 'numeric', month: 'short' }
  return `${t.name} · ${how}${d ? ' ' + d.toLocaleDateString([], opts) : ''}`
}
// The calls list: near its end, the next page.
document.addEventListener('scroll', (e) => {
  const el = e.target
  if (el && el.classList && el.classList.contains('rows') && S.callsNext && el.scrollTop + el.clientHeight > el.scrollHeight - 400) loadMoreCalls()
}, true)
document.addEventListener('input', (e) => {
  if (e.target.id === 'pp-about') { S.pj.aboutEdit = e.target.value; return }
  if (e.target.id === 'br-into-q') {
    const q = e.target.value
    S.br.mergeQ = q
    clearTimeout(S.br.mergeT)
    S.br.mergeT = setTimeout(async () => {
      if (!q.trim()) { S.br.mergeHits = []; return render() }
      try { const r = await get(`api/brain/browse/terms?q=${encodeURIComponent(q)}&limit=12`); if (S.br.mergeQ === q) { S.br.mergeHits = r.items || []; render() } } catch { /* app restarting */ }
    }, 200)
  }
})

/** A topic or term in the right panel: its description, where it comes up, the user's review
 *  (rename, merge, move, remove; a term's kind, variants, pins) and its latest sections. */
function brainNodePanel() {
  const d = dictData(), id = S.br.node, tx = (d && d.taxonomy) || {}
  const x = brItem(id)
  if (!x) return id && !(S.br.nodeData && S.br.nodeData.id === id) ? '<div class="skeleton"><i></i><i></i><i></i></div>' : '<div class="wp-empty">Pick a topic or a term.</div>'
  const isTerm = !!x.name, name = x.name || x.label, layer = x.layer
  // A client's topic may merge into, or sit under, one of the practice's too (one tree).
  const same = (isTerm ? [] : (tx.topics || [])).filter((t) => (t.layer === layer || (layer !== 'shared' && t.layer === 'shared')) && t.id !== id)
  const l1s = (tx.topics || []).filter((t) => (t.layer === layer || (layer !== 'shared' && t.layer === 'shared')) && t.level === 1 && t.id !== id)
  const lnm = (l) => l === 'shared' ? 'practice' : (((tx.layers || []).find((y) => y.id === l) || {}).name || l)
  const ed = S.br.edit
  const body = (o) => esc(JSON.stringify({ ...o, layer, [isTerm ? 'name' : 'label']: name, term: isTerm }))
  const actions = S.br.saving ? '<span class="set-sub">Saving your change…</span>' : ed === 'rename' ? `<span class="tok-in"><input id="br-rename" value="${esc(name)}" aria-label="New name" autofocus></span><button class="btn primary" data-br-do="rename">Rename</button>`
    : ed === 'merge' && isTerm ? `<div class="br-def"><input class="wp-in" id="br-into-q" value="${esc(S.br.mergeQ || '')}" placeholder="Find the term it is the same thing as" aria-label="Merge into" autofocus>${(S.br.mergeHits || []).filter((t) => t.id !== id).map((t) => `<button class="tok link" data-br-merge-into="${esc(JSON.stringify([t.name, t.layer]))}">${esc(t.name)}${t.layer !== layer ? ` <small>${esc(lnm(t.layer))}</small>` : ''}</button>`).join('') || (S.br.mergeQ ? '<span class="set-sub">No term by that name.</span>' : '')}</div>`
    : ed === 'merge' ? `<select class="wp-in" id="br-into" aria-label="Merge into">${same.map((t) => `<option value="${esc(JSON.stringify([t.name || t.label, t.layer]))}">${esc(t.name || t.label)}${t.layer !== layer ? ` (${esc(lnm(t.layer))})` : ''}</option>`).join('')}</select><button class="btn primary" data-br-do="merge">Merge</button>`
    : ed === 'scope' ? `<select class="wp-in" id="br-scope-to" aria-label="Whose topic">${layer === 'shared' ? (tx.layers || []).filter((l) => l.id !== 'shared').map((l) => `<option value="${esc(l.id)}">Only ${esc(l.name || l.id)}</option>`).join('') : '<option value="shared">Your practice (every client)</option>'}</select><button class="btn primary" data-br-do="scope">Move</button>`
    : ed === 'define' ? `<div class="br-def"><textarea class="wp-in" id="br-desc" rows="3" aria-label="What it is" placeholder="${isTerm ? 'What it is' : "What belongs in it — and what doesn't"}">${esc(x.description || '')}</textarea><input class="wp-in" id="br-syn" value="${esc((x.synonyms || []).join(', '))}" placeholder="Other names, comma-separated" aria-label="Synonyms">${isTerm ? `<input class="wp-in" id="br-spell" value="${esc((x.variants || []).join(', '))}" placeholder="Spellings — how it is written or misheard" aria-label="Spellings"><select class="wp-in" id="br-group" aria-label="Group">${(tx.groups || []).map((g) => `<option value="${esc(g.id)}"${x.group === g.id ? ' selected' : ''}>${esc(g.label)}</option>`).join('')}</select>` : ''}<button class="btn primary" data-br-do="define">Save</button></div>`
    : ed === 'split' ? `<select class="wp-in" id="br-split" aria-label="Which name">${[...(x.synonyms || []), ...(x.variants || [])].map((n) => `<option>${esc(n)}</option>`).join('')}</select><button class="btn primary" data-br-do="split">Make it a term</button>`
    : ed === 'move' ? `<select class="wp-in" id="br-parent" aria-label="Move under"><option value="">(top level)</option>${l1s.map((t) => `<option${x.parent === t.label ? ' selected' : ''}>${esc(t.label)}</option>`).join('')}</select><button class="btn primary" data-br-do="move">Move</button>`
    : ed === 'remove' ? `<span class="set-sub">Remove it? Its tags go too.</span><button class="btn danger" data-br-op='${body({ op: 'remove' })}'>Remove</button>`
    : `<button class="btn" data-br-edit="rename">Rename</button>${same.length || isTerm ? '<button class="btn" data-br-edit="merge">Merge into…</button>' : ''}<button class="btn" data-br-edit="define">Define…</button>${isTerm ? ([...(x.synonyms || []), ...(x.variants || [])].length ? '<button class="btn" data-br-edit="split">Split off…</button>' : '') : `<button class="btn" data-br-edit="move">Move under…</button>`}<button class="btn" data-br-edit="scope">${layer === 'shared' ? 'Only one client…' : 'Up to the practice…'}</button><button class="btn ghost danger" data-br-edit="remove">Remove</button>`
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
PANEL_VIEWS.push({ id: 'brain-node', label: 'Topic', icon: ICON('<circle cx="12" cy="12" r="3"/><circle cx="5" cy="6" r="2"/><circle cx="19" cy="7" r="2"/><circle cx="7" cy="19" r="2"/><path d="M9.5 10.5 6.6 7.4M14.6 10.6l2.8-2.4M10.2 14.2l-2 3.3"/>'), area: 'right', when: () => S.view === 'settings' && S.setSection === 'vocabulary' && !!S.br.node, render: brainNodePanel })

/** The edit errors this page has seen (numbered by the app): an edit shows only one newer than these. */
function noteEditErrors(d) { const n = ((d || {}).edit_error || {}).n || 0; if (n > (S.br.errSeen || 0)) S.br.errSeen = n }
/** The user's edit: the app makes the new version in the background (a large Brain takes seconds); the page
 *  reads the Brain again until it is done, and says why when it couldn't be done. */
function brainEdit(body, reload = loadDict, data = dictData) {
  const seen = S.br.errSeen || 0
  send({ cmd: 'brain_taxonomy', body })
  S.br.edit = null
  // Until it is saved, the panel offers no edit of what it shows (a form would start from the values before it).
  S.br.saving = true
  render()
  let tries = 0
  const again = () => reload().then(() => {
    const d = data() || {}
    if (d.edit_error && d.edit_error.n > seen) { S.br.saving = false; noteEditErrors(d); window.echo.notice(d.edit_error.text); return render() }
    const running = (d.topic_jobs || []).includes('edit:taxonomy') && ++tries < 150
    if (!running) S.br.saving = false
    render()
    if (running) setTimeout(again, 400)
  })
  setTimeout(again, 250)
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-br-tab],[data-br-node],[data-br-edit],[data-br-do],[data-br-op],[data-br-merge-into],[data-br-open]')
  if (!t) return
  const d = t.dataset
  if (d.brTab) { S.br.tab = d.brTab; return go(`brain/${d.brTab}`) }
  if (d.brNode) {
    S.br.node = d.brNode; S.br.edit = null; S.br.nodeData = null
    const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
    if (eff.narrow) S.overlay = true
    Layout.show('brain-node', known)
    layoutChanged()
    return loadBrainNode().then(render)
  }
  if (d.brEdit) { S.br.edit = d.brEdit; S.br.mergeHits = []; S.br.mergeQ = ''; render(); { const i = $('#br-rename') || $('#br-into-q'); if (i) { i.focus(); if (i.select) i.select() } } return }
  if (d.brOp) return brainEdit(JSON.parse(d.brOp))
  if (d.brMergeInto) {
    const x = brItem(S.br.node)
    if (!x) return
    const [into, into_layer] = JSON.parse(d.brMergeInto)
    S.br.node = null; S.br.mergeHits = []; S.br.mergeQ = ''
    return brainEdit({ layer: x.layer, term: true, name: x.name, op: 'merge', into, into_layer })
  }
  if (d.brDo) {
    const tx = (dictData() || {}).taxonomy || {}
    const x = brItem(S.br.node)
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
})
document.addEventListener('keydown', (e) => {
  if (e.target.id === 'br-rename' && e.key === 'Enter') { e.preventDefault(); const b = $('[data-br-do="rename"]'); if (b) b.click() }
  if (e.target.id === 'br-rename' && e.key === 'Escape') { S.br.edit = null; render() }
})
window.addEventListener('resize', () => { if (S.view === 'brain') GraphView.resize() })
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (Theme.chosen()) return // the user's choice holds
  GraphView.retheme()
  for (const e of Terms.values()) if (e.term) e.term.options.theme = termTheme()
  render()
})
