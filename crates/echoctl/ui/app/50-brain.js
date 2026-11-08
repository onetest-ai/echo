// ── the Brain (`brain/`): what Echo knows across calls, by client and thread ──────────────
// Its topics and terms to review (two layers: your practice, and each client's), the sources it
// holds, its coverage, and a graph of documents around their topics. Asking is Claude's job (the
// window's Claude sessions have the Brain); this shows and edits what it is made of.
/** Recall's tabs. Topics, numbers, coverage and the graph were the Brain's and didn't survive our tests (0.8.0, `brain::EXTRAS`):
 *  their views stay in the code until removed, never shown. */
const RECALL_EXTRAS = false
/** The plugins' tabs of Knowledge (its notes, its graph), before Recall's. */
const knowledgeTabs = () => (Plugins.slots['knowledge.tab'] || []).map((x) => ({ ...x, tab: `${x.plugin}:${x.id}` }))
const BRAIN_TABS = RECALL_EXTRAS
  ? [['topics', 'Topics'], ['terms', 'Terms'], ['people', 'People'], ['sources', 'Sources'], ['numbers', 'Numbers'], ['coverage', 'Coverage'], ['graph', 'Graph']]
  : [['sources', 'Sources'], ['terms', 'Dictionary'], ['people', 'People']]
const brainQuery = () => { const [k, v] = (S.br.scope || '').split(':'); return k && v ? `?${k}=${encodeURIComponent(v)}` : '' }
async function loadBrain() {
  const q = brainQuery()
  const before = S.br.data ? `${S.br.data.q}|${(S.br.data.taxonomy || {}).version}|${(S.br.data.status || {}).chunks}` : ''
  try { const d = await get('api/brain' + q); d.q = q; S.br.data = d; noteEditErrors(d) } catch { /* app restarting */ }
  const now = S.br.data ? `${S.br.data.q}|${(S.br.data.taxonomy || {}).version}|${(S.br.data.status || {}).chunks}` : ''
  // The topics graph again when the scope, the taxonomy or the store changed (a sync, a rename); the sets opened stay.
  const gv = `${((S.br.data || {}).taxonomy || {}).version}|${((S.br.data || {}).status || {}).chunks}`
  if (S.br.tab === 'graph' && (!S.br.graph || S.br.graph.q !== q || S.br.graph.v !== gv)) {
    try { const g = await get('api/brain/browse/graph' + q); g.q = q; g.v = gv; if (!S.br.graph || S.br.graph.q !== q) S.br.gsets = []; S.br.graph = g } catch { /* app restarting */ }
  }
  // Sources and the dictionary read their pages again only when a sync or an edit changed them (else they keep
  // the pages shown and the letter picked).
  if (before !== now && S.br.src && S.br.src.key === q) { S.br.src.groups = null; S.br.src.lists = {} }
  if (before !== now && S.br.dict) { S.br.dict.head = null; S.br.dict.lists = {} }
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
  const ktabs = knowledgeTabs()
  const kt = ktabs.find((x) => x.tab === S.br.tab)
  const ktabsHtml = ktabs.map((x) => `<button class="tab" role="tab" data-br-tab="${esc(x.tab)}" aria-selected="${S.br.tab === x.tab}">${esc(x.label)}</button>`).join('')
  // Recall off: only the plugins' tabs.
  if (kt || !brainOn()) {
    const t = kt || ktabs[0]
    if (!t) return ''
    S.br.tab = t.tab
    let html = ''
    try { html = t.render() } catch (e) { html = `<p class="error">${esc(t.label)}: ${esc(String((e && e.message) || e))}</p>` }
    const recallTabs = brainOn() ? BRAIN_TABS.map(([id, l]) => `<button class="tab" role="tab" data-br-tab="${id}" aria-selected="false">${l}</button>`).join('') : ''
    return `<div class="br">${`<header class="pj-head br-head"><div class="br-title"><h2>Knowledge</h2></div><div class="tabs" role="tablist">${ktabsHtml}${recallTabs}</div></header>`}<div class="pane pj-pane wide br-pane plug-act" data-keep-scroll data-ktab="${esc(t.tab)}">${html}</div></div>`
  }
  const d = S.br.data
  if (!d) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<div class="br"><p class="error">${esc(d.error)}</p></div>`
  const st = d.status || {}, tx = d.taxonomy || {}, sc = d.scopes || { clients: [], threads: [] }
  const scopeSel = `<label class="fchip${S.br.scope ? ' on' : ''}">${esc(S.br.scope ? (S.br.scope.startsWith('client:') ? clientName(S.br.scope.slice(7)) : ((projById(S.br.scope.slice(7)) || {}).name || 'In no thread')) : 'Everything')} ▾<select data-br-scope aria-label="Scope"><option value="">Everything</option><optgroup label="Clients">${sc.clients.map((c) => `<option value="client:${esc(c.id)}"${S.br.scope === `client:${c.id}` ? ' selected' : ''}>${esc(c.name)} · ${c.documents}</option>`).join('')}</optgroup><optgroup label="Threads">${sc.threads.map((t) => `<option value="thread:${esc(t.id)}"${S.br.scope === `thread:${t.id}` ? ' selected' : ''}>${esc(t.name)} · ${t.documents}</option>`).join('')}</optgroup></select></label>`
  const [sk, sv] = (S.br.scope || '').split(':')
  const inScope = sk ? ((sc[sk === 'client' ? 'clients' : 'threads'] || []).find((x) => x.id === sv) || {}).documents : null
  const n = { topics: (tx.topics || []).length, terms: tx.terms_n || 0, sources: inScope != null ? inScope : st.documents || 0 }
  const tabs = BRAIN_TABS.map(([id, l]) => `<button class="tab" role="tab" data-br-tab="${id}" aria-selected="${S.br.tab === id}">${l}${n[id] ? `<span class="n">${nf(n[id])}</span>` : ''}</button>`).join('')
  const sub = st.exists ? `${nf(st.documents)} sources · ${nf(st.chunks)} sections${st.embed_model ? ` · meaning by ${esc(st.embed_model)}` : ' · words only (no embedding server)'}` : 'Empty — it fills as your calls are synced'
  if (!BRAIN_TABS.some(([t]) => t === S.br.tab)) S.br.tab = 'sources'
  const head = `<header class="pj-head br-head"><div class="br-title"><h2>Knowledge</h2><span class="tk-sub">${sub}</span><span class="grow"></span>${scopeSel}<button class="btn ghost" data-go="settings" title="Settings → Recall">Settings</button></div><div class="tabs" role="tablist">${ktabsHtml}${tabs}</div></header>`
  const body = S.br.tab === 'terms' ? brainTerms(tx) : S.br.tab === 'sources' ? brainSources() : S.br.tab === 'numbers' ? brainNumbers() : S.br.tab === 'coverage' ? brainCoverage(tx) : S.br.tab === 'graph' ? brainGraph() : S.br.tab === 'people' ? brainPeople() : brainTopics(tx)
  return `<div class="br">${head}<div class="pane pj-pane wide br-pane" data-keep-scroll>${body}</div></div>`
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
  // A topic only closed threads' sections have is the archive (#55): its own folded group at the end.
  const kidsOf = (x) => topics.filter((k) => k.level === 2 && k.parent_id === x.id && !k.archived).sort((a, b) => (a.layer !== 'shared') - (b.layer !== 'shared') || (b.sections || 0) - (a.sections || 0))
  const roots = topics.filter((x) => (x.level === 1 || !x.parent_id) && !x.archived)
  const oldTopics = topics.filter((x) => x.archived)
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
    const fold = sub.length ? `<button class="br-fold" data-br-fold="${esc(x.id)}" aria-expanded="${isOpen}" title="${isOpen ? 'Fold' : 'Show'} ${sub.length} subtopic${sub.length === 1 ? '' : 's'}">${CHEV(isOpen)}</button>` : '<span class="br-fold"></span>'
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
  const arch = archiveGroup('Closed threads', oldTopics.map((x) => `<div class="br-card">${row(x, true)}</div>`), 'topics only closed threads talked about · still searchable')
  return `${top}${builds}${mv ? `<div class="br-mvs">${mv}</div>` : ''}${practice.length ? `<section class="br-layer"><div class="br-lh"><span class="cap">Your practice</span><span class="set-sub">one tree for every client: their topics sit under the practice's where they fit</span></div>${tree}</section>` : ''}${only}${arch}` || '<p class="set-sub">Nothing in this scope.</p>'
}
/** The dictionary (the Terms tab): every term — named things: organisations, products, initiatives… — by
 *  group, each group closed until opened, then A–Z 50 at a time (a letter jumps); a search over its name,
 *  other names and spellings lists the matches across groups. A click opens it to edit. Tens of thousands of
 *  terms: the app sends a page at a time (`browse::terms`). */
function termsState() {
  const key = brainQuery() + '|' + ((S.br.data || {}).taxonomy || {}).version
  if (!S.br.dict || S.br.dict.key !== key) { termById.clear(); S.br.dict = { key, head: null, open: S.br.dict ? S.br.dict.open : new Set(), lists: {}, letters: S.br.dict ? S.br.dict.letters : {}, q: S.br.dict ? S.br.dict.q : '' } }
  return S.br.dict
}
const termById = new Map()
async function loadTermsHead() {
  const s = termsState(), key = s.key
  if (s.loadingHead || s.head) return
  s.loadingHead = true
  try { const h = await get('api/brain/browse/terms?limit=1' + brainQuery().replace('?', '&')); if (termsState().key === key) s.head = h } catch { /* app restarting */ }
  s.loadingHead = false
  render()
}
/** A group's page (`g|<group>`, from `letter`) or the search (`q`); `more` adds the next 50. */
async function loadDictTerms(k, more, letter) {
  const s = termsState(), cur = s.lists[k]
  if (cur && cur.loading) return
  const p = new URLSearchParams({ limit: '50' })
  if (k === 'q') p.set('q', s.q)
  else p.set('group', k.slice(2))
  // The letter picked stays when the group is read again (after an edit or a sync).
  const from = letter !== undefined ? letter : cur ? cur.letter : (s.letters || {})[k] || ''
  s.letters = { ...(s.letters || {}), [k]: from }
  if (from) p.set('letter', from)
  const offset = more && cur ? cur.items.length : 0
  if (offset) p.set('offset', String(offset))
  s.lists[k] = { ...(cur || { items: [] }), letter: from, loading: true }
  if (!more) s.lists[k].items = []
  render()
  const asked = s.q
  try {
    const r = await get(`api/brain/browse/terms?${qs(p)}${brainQuery().replace('?', '&')}`)
    if (termsState() !== s || (k === 'q' && s.q !== asked)) return
    for (const x of r.items || []) termById.set(x.id, x)
    s.lists[k] = { items: (more && cur ? cur.items : []).concat(r.items || []), letters: r.letters || {}, more: r.more || 0, matches: r.matches || 0, letter: from, loading: false }
  } catch (e) { s.lists[k] = { items: [], loading: false, error: String(e) } }
  render()
}
function brainTerms(tx) {
  if (!tx.exists) return '<p class="set-sub">No terms yet: they come with the topics.</p>'
  const s = termsState()
  if (!s.head) { setTimeout(loadTermsHead, 0); return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(5)}</div>` }
  const layers = tx.layers || []
  const lname = (id) => id === 'shared' ? 'Practice' : ((layers.find((l) => l.id === id) || {}).name || id)
  const lcolor = (id) => { const i = layers.filter((l) => l.id !== 'shared').findIndex((l) => l.id === id); return i < 0 ? '#8a8a95' : CLIENT_COLORS[i % CLIENT_COLORS.length] }
  const entry = (x) => {
    const names = (x.synonyms || []).length ? `<span class="dict-aka">also ${esc(x.synonyms.join(', '))}</span>` : ''
    const flags = [x.always ? '<span class="br-pin" title="Hints every call’s transcription">∞</span>' : '', (x.pinned || []).length ? '<span class="br-pin" title="Pinned to a thread: hints its calls’ transcription">📌</span>' : '', x.learned ? '<span class="dict-flag" title="Learned from the minutes">learned</span>' : ''].join('')
    const tps = (x.topics || []).map((t) => `<span class="dict-tp">${esc(t.label)}</span>`).join('')
    const was = (x.archived || []).map((t) => `<span class="chip closed">${esc(closedLabel(t))}</span>`).join('')
    return `<button class="dict-e${S.br.node === x.id ? ' on' : ''}${x.archived ? ' closed' : ''}" data-br-node="${esc(x.id)}"><span class="dict-h"><b>${esc(x.name)}</b> <span class="br-cl" style="--c:${lcolor(x.layer)}">${esc(lname(x.layer))}</span> <span class="set-sub">${esc(x.kind || '')}</span>${flags}${was}<span class="grow"></span><span class="br-n" title="Sections that mention it">${x.sections || 0}</span></span>${names}${x.description ? `<span class="dict-d">${esc(x.description)}</span>` : ''}${tps ? `<span class="dict-tps">${tps}</span>` : ''}</button>`
  }
  const list = (k) => {
    const l = s.lists[k]
    if (!l) setTimeout(() => loadDictTerms(k), 0)
    if (!l || (l.loading && !l.items.length)) return '<div class="bs-wait">Loading…</div>'
    if (l.error) return `<div class="bs-wait error">${esc(l.error)}</div>`
    const az = k === 'q' ? '' : `<div class="dict-az">${Object.entries(l.letters || {}).map(([ch, n]) => `<button class="${l.letter === ch ? 'on' : ''}" data-dict-letter="${esc(ch)}" data-dict-in="${esc(k)}" title="${n} term${n === 1 ? '' : 's'}">${esc(ch)}</button>`).join('')}${l.letter ? `<button data-dict-letter="" data-dict-in="${esc(k)}">All</button>` : ''}</div>`
    const rows = l.items.length ? `<div class="dict-list">${l.items.map(entry).join('')}</div>` : '<div class="bs-wait">Nothing here.</div>'
    const more = l.more > 0 ? `<button class="bs-more" data-dict-more="${esc(k)}"${l.loading ? ' disabled' : ''}>${l.loading ? 'Loading…' : `Show ${Math.min(50, l.more)} more${l.letter ? ` in ${esc(l.letter)}–Z` : ''} · ${nf(l.more)} left`}</button>` : ''
    return az + rows + more
  }
  const box = `<input class="wp-in br-pq" id="dict-q" type="search" placeholder="Find a term — its name, another name, a spelling" value="${esc(s.q)}" aria-label="Find a term">`
  const groups = s.head.groups || {}
  const sub = `<span class="set-sub">${nf(s.head.current)} terms · ${Object.keys(groups).length} groups</span>`
  const foot = '<p class="set-foot">A term is a named thing: its name, its other names (what search and Claude take for it) and its spellings (how it is written or misheard — they hint transcription). Merge two that are one thing; split off a name that is a thing of its own.</p>'
  if (s.q.trim()) {
    if (!s.lists.q) setTimeout(() => loadDictTerms('q'), 0)
    const l = s.lists.q
    return `<div class="br-graph-bar">${box}${sub}</div><div class="bs-acc"><div class="bs-h static"><b>${l && !l.loading ? `${nf(l.matches)} match${l.matches === 1 ? '' : 'es'}` : 'Searching…'}</b><span class="n">every group, A–Z</span></div><div class="bs-body pad">${list('q')}</div></div>${foot}`
  }
  const label = Object.fromEntries((tx.groups || []).map((g) => [g.id, g.label]))
  const order = (tx.groups || []).map((g) => g.id).filter((id) => groups[id]).concat(Object.keys(groups).filter((id) => !label[id]))
  const acc = (id, title, n, note) => {
    const k = `g|${id}`, open = s.open.has(k)
    return `<div class="bs-acc"><button class="bs-h" data-dict-toggle="${esc(k)}" aria-expanded="${open}"><span class="ch">${CHEV(open)}</span><b>${esc(title)}</b>${note ? `<span class="n">${esc(note)}</span>` : ''}<span class="grow"></span><span class="n">${nf(n)}</span></button>${open ? `<div class="bs-body pad">${list(k)}</div>` : ''}</div>`
  }
  const accs = order.map((id) => acc(id, label[id] || id, groups[id])).join('')
  const closed = s.head.closed ? acc('closed', 'Closed threads', s.head.closed, "out of transcription hints and Claude's current list · search and spellings still resolve them") : ''
  return `<div class="br-graph-bar">${box}${sub}</div>${accs}${closed}${foot}`
}
document.addEventListener('click', (e) => {
  const a = e.target.closest && e.target.closest('a[href^="echo://"]')
  if (!a) return
  e.preventDefault()
  window.echo.openLink(a.getAttribute('href'))
}, true)
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-dict-toggle],[data-dict-more],[data-dict-letter]')
  if (!t) return
  const s = termsState(), d = t.dataset
  if (d.dictMore) return loadDictTerms(d.dictMore, true)
  if (d.dictLetter !== undefined) return loadDictTerms(d.dictIn, false, d.dictLetter)
  const k = d.dictToggle
  if (s.open.has(k)) s.open.delete(k)
  else { s.open.add(k); if (!s.lists[k]) return loadDictTerms(k) }
  render()
})
let dictTimer = null
document.addEventListener('input', (e) => {
  if (e.target.id !== 'dict-q') return
  const s = termsState()
  s.q = e.target.value
  delete s.lists.q
  clearTimeout(dictTimer)
  dictTimer = setTimeout(render, 300)
})
/** A topic or a term by id: as the panel last read it (`/api/brain/node`, current after every edit), else the
 *  overview's topics or the dictionary's pages until that comes. */
function brItem(id) {
  const nd = S.br.nodeData
  if (nd && nd.id === id && nd.item) return nd.item
  const tx = (S.br.data || {}).taxonomy || {}
  return (tx.topics || []).find((t) => t.id === id) || termById.get(id) || (tx.terms || []).find((t) => t.id === id) || null
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
