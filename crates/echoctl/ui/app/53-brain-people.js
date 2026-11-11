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
  // Someone seen only in closed threads is the archive (#55): off the map unless asked for (the toggle above it) or the
  // scope is that thread.
  const own = (x) => S.br.pClosed || (k === 'thread' && (x.archived || []).some((t) => t.id === v))
  const all = d.people.filter((x) => brShown(x) && (x.threads || []).some(inScope))
  const people = all.filter((x) => !x.archived || own(x))
  const archived = all.filter((x) => x.archived && !own(x))
  const threads = (d.threads || []).filter((t) => inScope(t.id) && people.some((x) => (x.threads || []).includes(t.id)))
  return { people, threads, archived }
}
/** "Hooli workshop · closed 28 Oct" — a closed thread as the archive names it. */
function closedLabel(t) {
  const how = t.stage === 'lost' ? 'lost' : t.stage === 'cancelled' ? 'cancelled' : 'closed'
  const d = t.closed_at ? new Date(t.closed_at) : null
  const opts = d && d.getFullYear() !== new Date().getFullYear() ? { day: 'numeric', month: 'short', year: 'numeric' } : { day: 'numeric', month: 'short' }
  return `${t.name} · ${how}${d ? ' ' + d.toLocaleDateString([], opts) : ''}`
}
/** The folded "Only in closed threads" group under a list or map. */
function archiveGroup(title, rows, note) {
  if (!rows.length) return ''
  return `<details class="br-arch"${S.br.archOpen === title ? ' open' : ''} data-arch="${esc(title)}"><summary><span class="br-arch-h">${esc(title)}</span><span class="n">${rows.length}</span><span class="set-sub">${esc(note)}</span></summary>${rows.join('')}</details>`
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
  // People seen only in closed threads: on the map when asked for, greyed, with their closed threads.
  const nOld = d.people.filter((x) => x.archived && brShown(x)).length
  const tog = nOld ? `<span class="br-tog"><button class="sw${S.br.pClosed ? ' on' : ''}" role="switch" aria-checked="${!!S.br.pClosed}" aria-label="Show people from closed threads" data-br-pclosed></button>Show people from closed threads <span class="set-sub">${nOld}</span></span>` : ''
  if (!v.people.length) return `<div class="br-graph-bar">${box}${tog}<span class="set-sub">${sub}</span></div><p class="set-sub">${nOld ? 'Nobody current here — switch on the people from closed threads to see them.' : 'Nobody yet: people appear as their threads are synced.'}</p>`
  return `<div class="br-graph-bar">${box}${res}${tog}<span class="set-sub">${sub}</span></div><div class="br-graph" id="br-people"></div><p class="set-foot">Threads labelled, people near theirs: a line is a role, calls (faint), reports to (dark) or calls together. Scroll to zoom, hover for someone's links, click for who they are.</p>`
}
function drawPeople() {
  const box = $('#br-people')
  const v = box && brPeopleIn()
  if (!box || !v) return
  const key = `people|${S.br.scope || ''}|${S.br.pClosed ? 'closed' : ''}|${v.people.length}|${v.threads.length}|${(S.br.people || {}).at || ''}`
  const st = GraphView.draw(box, key, () => {
    const G = new graphology.Graph({ type: 'undirected' })
    const clients = [...new Set(v.threads.map((t) => (projById(t.id) || {}).client || '-'))]
    const shut = (id) => ProjectArt.stage((projById(id) || {}).stage).gid === 'closed'
    const tColor = (id) => { const c = (projById(id) || {}).client; return shut(id) ? '#8a8a9588' : c ? CLIENT_COLORS[clients.indexOf(c) % CLIENT_COLORS.length] : '#8a8a95' }
    const orgs = [...new Set(v.people.map((x) => (x.org || {}).id || x.side))]
    const pColor = (x) => x.archived ? '#8a8a9588' : x.side === 'yours' ? '#8a8a95' : CLIENT_COLORS[(orgs.indexOf((x.org || {}).id || x.side) + 3) % CLIENT_COLORS.length]
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
// The calls list: near its end, the next page.
document.addEventListener('scroll', (e) => {
  const el = e.target
  if (el && el.classList && el.classList.contains('rows') && S.callsNext && el.scrollTop + el.clientHeight > el.scrollHeight - 400) loadMoreCalls()
}, true)
document.addEventListener('toggle', (e) => {
  const d = e.target
  if (d.classList && d.classList.contains('br-arch')) S.br.archOpen = d.open ? d.dataset.arch : (S.br.archOpen === d.dataset.arch ? null : S.br.archOpen)
}, true)
document.addEventListener('input', (e) => {
  if (e.target.id === 'pp-about') { S.pj.aboutEdit = e.target.value; return }
  if (e.target.id === 'br-into-q') {
    const q = e.target.value
    S.br.mergeQ = q
    clearTimeout(S.br.mergeT)
    S.br.mergeT = setTimeout(async () => {
      if (!q.trim()) { S.br.mergeHits = []; return render() }
      try { const r = await get(`api/brain/browse/terms?q=${encodeURIComponent(q)}&limit=12${brainQuery().replace('?', '&')}`); if (S.br.mergeQ === q) { S.br.mergeHits = r.items || []; render() } } catch { /* app restarting */ }
    }, 200)
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
    : `<button class="btn" data-br-edit="rename">Rename</button>${same.length || isTerm ? '<button class="btn" data-br-edit="merge">Merge into…</button>' : ''}<button class="btn" data-br-edit="define">Define…</button>${isTerm ? ([...(x.synonyms || []), ...(x.variants || [])].length ? '<button class="btn" data-br-edit="split">Split off…</button>' : '') : `<button class="btn" data-br-edit="move">Move under…</button><button class="btn" data-br-split="${esc(JSON.stringify([layer, name]))}" title="Make 2–6 subtopics of it from its own sections (Claude), then tag only those again">Split into subtopics</button>`}<button class="btn" data-br-edit="scope">${layer === 'shared' ? 'Only one client…' : 'Up to the practice…'}</button><button class="btn ghost danger" data-br-edit="remove">Remove</button>`
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
  if (x.missing || x.error) return `<div class="wp-empty">${esc(x.error || 'This source is gone from Recall.')}</div>`
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
    ${x.closed ? `<div class="br-closed">${x.closed.map((t) => `<span class="chip closed">🗄 closed thread · ${esc(closedLabel(t))}</span>`).join('')}<span class="set-sub">its archive: still found, ranked under current material</span></div>` : ''}
    <div class="br-acts">${open}</div>
    ${topics ? `<div class="wp-h">Topics</div><div class="br-topics">${topics}</div>` : ''}
    ${terms ? `<div class="wp-h">Terms</div><div class="br-topics">${terms}</div>` : ''}
    <div class="wp-h">Sections</div>${secs || '<p class="set-sub">No sections.</p>'}${x.sections_total > (x.sections || []).length ? `<p class="set-sub">${x.sections_total - x.sections.length} more — open it to read them all.</p>` : ''}
    ${rel ? `<div class="wp-h">Related sources</div><div class="br-rels">${rel}</div>` : ''}
  </div>`
}
PANEL_VIEWS.push({ id: 'brain-doc', label: 'Source', icon: ICON('<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h4"/>'), area: 'right', when: () => S.view === 'brain' && !!S.br.doc, render: brainDocPanel })
PANEL_VIEWS.push({ id: 'brain-node', label: 'Topic', icon: ICON('<circle cx="12" cy="12" r="3"/><circle cx="5" cy="6" r="2"/><circle cx="19" cy="7" r="2"/><circle cx="7" cy="19" r="2"/><path d="M9.5 10.5 6.6 7.4M14.6 10.6l2.8-2.4M10.2 14.2l-2 3.3"/>'), area: 'right', when: () => S.view === 'brain' && !!S.br.node, render: brainNodePanel })

/** The edit errors this page has seen (numbered by the app): an edit shows only one newer than these. */
function noteEditErrors(d) { const n = ((d || {}).edit_error || {}).n || 0; if (n > (S.br.errSeen || 0)) S.br.errSeen = n }
/** The user's edit: the app makes the new version in the background (a large Brain takes seconds); the page
 *  reads the Brain again until it is done, and says why when it couldn't be done. */
function brainEdit(body, reload = loadBrain, data = () => S.br.data) {
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
  const t = e.target.closest && e.target.closest('[data-br-tab],[data-br-node],[data-br-accept],[data-br-fold],[data-br-rebuild],[data-br-split],[data-br-retag],[data-br-move],[data-br-snooze],[data-br-edit],[data-br-do],[data-br-op],[data-br-merge-into],[data-br-open],[data-graph-doc],[data-br-doc-open]')
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
  // Folded = it was open: one click opens a closed one (it took two — a closed topic had no `fold` entry yet).
  if (d.brFold) { S.br.fold = { ...(S.br.fold || {}), [d.brFold]: t.getAttribute('aria-expanded') === 'true' }; return render() }
  if (d.brRebuild) { send({ cmd: 'brain_rebuild_topics', layer: d.brRebuild }); S.br.rebuilding = { ...(S.br.rebuilding || {}), [`taxonomy:${d.brRebuild}`]: true }; setTimeout(() => { S.br.rebuilding = {}; loadBrain().then(render) }, 1500); return render() }
  if (d.brSplit) { const [layer, label, asker] = JSON.parse(d.brSplit); send({ cmd: 'brain_split_topic', layer, label, for: asker || layer }); S.br.rebuilding = { ...(S.br.rebuilding || {}), [`split:${asker || layer}`]: true }; setTimeout(() => { S.br.rebuilding = {}; loadBrain().then(render) }, 1500); return render() }
  if (d.brMove) { send({ cmd: 'brain_topic_move', key: d.brMove, do: d.do }); return setTimeout(() => loadBrain().then(render), 400) }
  if (d.brRetag) { send({ cmd: 'brain_retag_topics', layer: d.brRetag }); S.br.rebuilding = { ...(S.br.rebuilding || {}), [`retag:${d.brRetag}`]: true }; setTimeout(() => { S.br.rebuilding = {}; loadBrain().then(render) }, 1500); return render() }
  if (d.brSnooze) { send({ cmd: 'brain_topics_snooze', layer: d.brSnooze }); return setTimeout(() => loadBrain().then(render), 300) }
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
    const tx = (S.br.data || {}).taxonomy || {}
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
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (Theme.chosen()) return // the user's choice holds
  GraphView.retheme()
  for (const e of Terms.values()) if (e.term) e.term.options.theme = termTheme()
  render()
})

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
  if (!brainOn() || !RECALL_EXTRAS) return ''
  const k = `${kind}:${id}`
  if (insights.key !== k) { loadInsights(kind, id); return '' }
  const d = insights.data
  if (!d || d.error) return ''
  const list = d.insights || []
  const when = d.at ? `checked ${esc(relDay(d.at))}` : 'not run yet'
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
    case 'edit-field': S.pj.editField = d.field; render(); { const i = document.querySelector('.wp [autofocus]'); if (i) i.focus() } return
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

