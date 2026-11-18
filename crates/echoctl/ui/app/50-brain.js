// ── Recall (`#brain`): the plugins' Knowledge tabs; the dictionary (Settings → Vocabulary); who may speak ──────────
// The activity is there only while a plugin adds a tab (`knowledge.tab`). Asking what was said is Claude's job (the
// window's Claude sessions have Echo's search); the dictionary's terms are edited here.
/** The plugins' tabs of Knowledge (its notes, its graph). */
const knowledgeTabs = () => (Plugins.slots['knowledge.tab'] || []).map((x) => ({ ...x, tab: `${x.plugin}:${x.id}` }))
const nf = (n) => Number(n || 0).toLocaleString()
/** Query parameters as Echo reads them (`%20`, never `+`, for a space). */
const qs = (p) => p.toString().replace(/\+/g, '%20')
/** The dictionary's own data for Settings → Vocabulary (`/api/brain/dictionary`: unscoped, with Intake on or off). */
const dictData = () => S.br.dictData
async function loadDict() {
  const before = S.br.dictData ? (S.br.dictData.taxonomy || {}).version : null
  try { const d = await get('api/brain/dictionary'); S.br.dictData = d; noteEditErrors(d) } catch { /* app restarting */ }
  if (S.br.dictData && before !== (S.br.dictData.taxonomy || {}).version && S.br.dict) { S.br.dict.head = null; S.br.dict.lists = {} }
  if (S.br.node) await loadBrainNode()
}
async function loadBrainNode() {
  const id = S.br.node
  try { const d = await get(`api/brain/node?id=${encodeURIComponent(id)}`); if (S.br.node === id) S.br.nodeData = d } catch { /* app restarting */ }
}
const threadChips = (ids) => (ids || []).filter((t) => t !== '-').map((t) => projById(t)).filter(Boolean).map((p) => `<span class="tk-pj">${projIcon(p, 12)}${esc(p.name)}</span>`).join('') + ((ids || []).includes('-') ? '<span class="tk-pj br-none">no thread</span>' : '')
/** Draws the Knowledge view in the primary area (`renderNow`). */
// ── recall leave: Recall with no Knowledge tab goes to Calls, after the grace a plugin's update needs
/** Recall without a Knowledge tab (its plugin switched off or removed): Calls, once `RECALL_GRACE` ms passed and it is
 *  still on Recall with none — a plugin updating is unloaded and applied again in between (as `unloadPlugin` gives
 *  its activities). One wait at a time. */
const RECALL_GRACE = 1500
let recallLeave = null
function leaveRecallLater(later = setTimeout) {
  if (recallLeave) return
  recallLeave = later(() => { recallLeave = null; if (S.view === 'brain' && !knowledgeTabs().length) go('calls', { replace: true }) }, RECALL_GRACE)
}
// end of the recall leave block
function drawBrain(primary) {
  if (!knowledgeTabs().length) return leaveRecallLater()
  patch(primary, `<main class="main">${brainView()}</main>`)
  // A plugin's Knowledge tab draws after the page is in (its graph).
  const kt = knowledgeTabs().find((x) => x.tab === S.br.tab)
  const box = kt && primary.querySelector(`[data-ktab="${CSS.escape(kt.tab)}"]`)
  try { if (box && kt.after) kt.after(box) } catch (e) { console.error(e) }
}
/** The plugins' Knowledge tabs; a tab Echo no longer has (`#brain/sources`, `#brain/people`) opens the first of them. */
function brainView() {
  const ktabs = knowledgeTabs()
  const t = ktabs.find((x) => x.tab === S.br.tab) || ktabs[0]
  if (!t) return ''
  S.br.tab = t.tab
  const ktabsHtml = ktabs.map((x) => `<button class="tab" role="tab" data-br-tab="${esc(x.tab)}" aria-selected="${S.br.tab === x.tab}">${esc(x.label)}</button>`).join('')
  let html = ''
  try { html = t.render() } catch (e) { html = `<p class="error">${esc(t.label)}: ${esc(String((e && e.message) || e))}</p>` }
  return `<div class="br"><header class="pj-head br-head"><div class="br-title"><h2>Recall</h2></div><div class="tabs" role="tablist">${ktabsHtml}</div></header><div class="pane pj-pane wide br-pane plug-act" data-keep-scroll data-ktab="${esc(t.tab)}">${html}</div></div>`
}
/** The dictionary (the Terms tab): every term — named things: organisations, products, initiatives… — by
 *  group, each group closed until opened, then A–Z 50 at a time (a letter jumps); a search over its name,
 *  other names and spellings lists the matches across groups. A click opens it to edit. Tens of thousands of
 *  terms: the app sends a page at a time (`browse::terms`). */
function termsState() {
  const key = '|' + ((dictData() || {}).taxonomy || {}).version
  if (!S.br.dict || S.br.dict.key !== key) { termById.clear(); S.br.dict = { key, head: null, open: S.br.dict ? S.br.dict.open : new Set(), lists: {}, letters: S.br.dict ? S.br.dict.letters : {}, q: S.br.dict ? S.br.dict.q : '' } }
  return S.br.dict
}
const termById = new Map()
async function loadTermsHead() {
  const s = termsState(), key = s.key
  if (s.loadingHead || s.head) return
  s.loadingHead = true
  try { const h = await get('api/brain/browse/terms?limit=1'); if (termsState().key === key) s.head = h } catch { /* app restarting */ }
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
    const r = await get(`api/brain/browse/terms?${qs(p)}`)
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
  window.echo.openLink(a.getAttribute('href'), true)
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
 *  dictionary's pages until that comes. */
function brItem(id) {
  const nd = S.br.nodeData
  if (nd && nd.id === id && nd.item) return nd.item
  const tx = (dictData() || {}).taxonomy || {}
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
/** Who may speak through mail and channels (Settings → Intake), per client: senders waiting for a decision
 *  (their messages aren't read until trusted), then the trusted and ignored ones. Colleagues and the
 *  user are trusted without asking; a trusted outside sender's mail is read only when authenticated. */
function brainSenders() {
  loadSourceState()
  const s = (m365.state || {}).senders
  if (!s) return ''
  const name = (c) => c === '-' ? 'No client' : ((((S.pj.data || {}).clients) || []).find((x) => x.id === c) || {}).name || c
  const cards = Object.entries(s.clients || {}).map(([c, l]) => {
    const btn = (k, d, label, primary) => `<button class="btn${primary ? ' primary' : ''}" data-sender="${esc(d)}" data-client="${esc(c)}" data-key="${esc(k)}">${label}</button>`
    const who = (x) => `<b>${esc(x.name || x.key)}</b>${x.key.startsWith('teams:') ? '' : ` <span class="set-sub">${esc(x.key)}</span>`}`
    const wait = (l.waiting || []).sort((a, b) => (b.last_at || '').localeCompare(a.last_at || '')).map((w) => `<tr><td><span class="pp-who"><span class="br-kind">${w.kind === 'teams' ? '#' : '✉'}</span>${who(w)}<span class="set-sub br-why">${w.count} message${w.count === 1 ? '' : 's'} · last ${esc(relDay(w.last_at))} · “${esc(w.last_about)}”${w.kind === 'mail' && !w.verified ? ' · not authenticated' : ''}${w.hint ? ' · ' + esc(w.hint) : ''}</span></span></td><td class="br-acts">${btn(w.key, 'trust', 'Trust', true)}${btn(w.key, 'ignore', 'Ignore')}</td></tr>`)
    // A trusted sender's mail with no authentication verdict (a mailing list, a domain without DMARC).
    const unver = (l.unverified || []).map((w) => `<tr><td><span class="pp-who"><span class="br-kind">✉</span>${who(w)}<span class="set-sub br-why">${w.count} message${w.count === 1 ? '' : 's'} not authenticated · last ${esc(relDay(w.last_at))} · “${esc(w.last_about)}” · could be forged</span></span></td><td class="br-acts">${btn(w.key, 'read_unverified', 'Read anyway')}${btn(w.key, 'keep_unverified_out', 'Keep out')}</td></tr>`)
    const unverWord = (x) => x.unverified === true ? ' · unauthenticated mail read too' : x.unverified === false ? ' · unauthenticated mail kept out' : ''
    const decided = [...(l.trusted || []).map((x) => [x, 'trusted' + unverWord(x)]), ...(l.ignored || []).map((x) => [x, 'ignored'])].map(([x, st]) => `<tr class="br-sub"><td><span class="pp-who">${who(x)}<span class="set-sub br-why">${st}</span></span></td><td class="br-acts">${btn(x.key, 'forget', 'Forget')}</td></tr>`)
    if (!wait.length && !unver.length && !decided.length) return ''
    return `<div class="pp-card"><table class="pp-tbl"><thead><tr><th>${esc(name(c))} · senders${wait.length ? ` · <span class="error">${wait.length} waiting</span>` : ''}${unver.length ? ` · ${unver.length} not authenticated` : ''}</th><th></th></tr></thead><tbody>${wait.join('')}${unver.join('')}${decided.join('')}</tbody></table></div>`
  }).filter(Boolean)
  return cards.length ? `<p class="set-sub">Mail and posts are read only from you, colleagues and the senders you trust; Intake keeps facts read from them, never their text.</p>${cards.join('')}` : ''
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-sender]')
  if (!b) return
  b.disabled = true
  send({ cmd: 'brain_sender', client: b.dataset.client, key: b.dataset.key, decision: b.dataset.sender })
  setTimeout(() => loadSourceState(true), 400)
})
