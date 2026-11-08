// ── the Brain's sources at scale (`brain/browse.rs`): tens of thousands, a piece at a time ─────────────────
// Clients, each opening to its kinds, channels and folders; each of those to its newest 50 and "Show 50 more".
// Everything starts closed; a search lists what matches across all of it. Nothing loads until it is opened.
const SRC_KINDS = [['call', '▮', 'Calls'], ['mail', '✉', 'Mail'], ['channel', '#', 'Channels'], ['file', '▣', 'Folders']]
const nf = (n) => Number(n || 0).toLocaleString()
/** Query parameters as Echo reads them (`%20`, never `+`, for a space). */
const qs = (p) => p.toString().replace(/\+/g, '%20')
function srcState() {
  const key = brainQuery()
  if (!S.br.src || S.br.src.key !== key) S.br.src = { key, groups: null, open: new Set(), lists: {}, kind: S.br.src ? S.br.src.kind : '', q: '', search: null }
  return S.br.src
}
async function loadSourceGroups(force) {
  const s = srcState(), key = s.key
  if (s.loadingGroups || (s.groups && !force)) return
  s.loadingGroups = true
  try { const g = await get('api/brain/browse/groups' + key); if (srcState().key === key) s.groups = g } catch { /* app restarting */ }
  s.loadingGroups = false
  render()
}
/** What a list key asks for: `k|client|kind`, `ch|client|channel`, `f|client|root`, `q` (the search). */
function srcUrl(k, cursor) {
  const s = srcState(), sc = s.key.replace('?', '&')
  const [t, client, x] = k.split('|')
  if (t === 'f') return `api/brain/browse/folder?root=${encodeURIComponent(x)}&offset=${cursor || 0}&limit=50`
  const p = new URLSearchParams({ limit: '50' })
  if (t === 'q') { p.set('q', s.q); if (s.kind) p.set('kind', s.kind) } else p.set('of', client)
  if (t === 'k') p.set('kind', x)
  if (t === 'ch') p.set('channel', x)
  if (cursor) p.set('before', cursor)
  return `api/brain/browse/sources?${qs(p)}${sc}`
}
async function loadSrcList(k, more) {
  const s = srcState()
  const cur = s.lists[k]
  if (cur && cur.loading) return
  const cursor = more && cur ? (k.startsWith('f|') ? cur.items.length : cur.next) : null
  s.lists[k] = { ...(cur || { items: [] }), loading: true }
  render()
  const asked = s.q
  try {
    const r = await get(srcUrl(k, cursor))
    if (srcState() !== s || (k === 'q' && s.q !== asked)) return
    const items = (more && cur ? cur.items : []).concat(r.items || [])
    s.lists[k] = { items, next: r.next || null, total: r.total || 0, loading: false, error: r.error }
  } catch (e) { s.lists[k] = { ...(s.lists[k] || { items: [] }), loading: false, error: String(e) } }
  render()
}
const SRC_STATUS = { indexed: 'read', pending: 'read · pages waiting for the page reader', twin: 'not read', skipped: 'not read', unsupported: 'not read', error: "couldn't be read", reading: 'reading…' }
function srcRow(x, opts) {
  const o = opts || {}
  const call = x.link && x.link.startsWith('echo://call/') ? x.link.slice(12) : ''
  const fileId = (x.source || '').startsWith('file/') ? x.source.slice(5) : ''
  const title = o.channel && x.channel && (x.title || '').startsWith(x.channel + ': ') ? x.title.slice(x.channel.length + 2) : (x.title || x.source)
  const hist = x.history ? ' <span class="br-hist" title="Dated before its thread\'s history date: kept, searched only when history is asked for">history</span>' : ''
  const where = o.client ? `<span class="chip">${esc(clientName(x.client || '-'))}</span>` : ''
  const open = call ? `data-call="${esc(call)}"` : fileId ? `data-br-file="${esc(fileId)}"` : extAttr(x.link)
  return `<div class="bs-row" ${open}><span class="k">${kindMark(x)}</span><span class="t">${esc(title)}${hist}</span><span class="d">${x.date ? esc(relDay(x.date)) : ''}</span><span class="th">${where}${threadChips((x.threads || []).map((t) => t.id))}</span><span class="num">${x.sections || ''}</span></div>`
}
function fileRow(f) {
  const read = f.status === 'indexed' || f.status === 'pending'
  const why = [SRC_STATUS[f.status] || f.status, f.pages ? `${f.pages} page${f.pages === 1 ? '' : 's'}${f.flagged ? `, ${f.read || 0}/${f.flagged} read from images` : ''}` : '', f.fidelity && f.fidelity !== 'full' ? f.fidelity : '', f.reason || ''].filter(Boolean).join(' · ')
  return `<div class="bs-row${read ? '' : ' off'}"${read ? ` data-br-file="${esc(f.id)}"` : ''}><span class="k">${read ? '▣' : '·'}</span><span class="t">${esc(f.rel)}<span class="set-sub br-why">${esc(why)}</span></span><span class="d">${f.date ? esc(relDay(f.date)) : ''}</span><span class="th"></span><span class="num"></span></div>`
}
/** A list's rows, its "Show 50 more · N left", or what it is waiting for. */
function srcListBody(k, opts) {
  const l = srcState().lists[k]
  // Open but not read (the Brain was read again meanwhile): read it.
  if (!l) setTimeout(() => loadSrcList(k), 0)
  if (!l || (l.loading && !l.items.length)) return '<div class="bs-wait">Loading…</div>'
  if (l.error) return `<div class="bs-wait error">${esc(l.error)}</div>`
  if (!l.items.length) return '<div class="bs-wait">Nothing here.</div>'
  const rows = l.items.map((x) => (k.startsWith('f|') ? fileRow(x) : srcRow(x, opts))).join('')
  const left = l.total - l.items.length
  const more = left > 0 ? `<button class="bs-more" data-bs-more="${esc(k)}"${l.loading ? ' disabled' : ''}>${l.loading ? 'Loading…' : `Show ${Math.min(50, left)} more · ${nf(left)} left`}</button>` : ''
  return rows + more
}
function srcAcc(k, head, body, depth) {
  const open = srcState().open.has(k)
  return `<div class="bs-acc${depth ? ' sub' : ''}"><button class="bs-h" data-bs-toggle="${esc(k)}" aria-expanded="${open}"><span class="ch">${CHEV(open)}</span>${head}</button>${open ? `<div class="bs-body">${body()}</div>` : ''}</div>`
}
function brainSources() {
  const s = srcState()
  if (!s.groups) { setTimeout(loadSourceGroups, 0); return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(5)}</div>` }
  loadSourceState()
  const g = s.groups
  if (g.error) return `<p class="error">${esc(g.error)}</p>`
  const byKind = g.by_kind || {}
  const chips = `<button class="fchip${!s.kind ? ' on' : ''}" data-bs-kind="">All <b>${nf(g.total)}</b></button>` + SRC_KINDS.filter(([k]) => byKind[k]).map(([k, m, l]) => `<button class="fchip${s.kind === k ? ' on' : ''}" data-bs-kind="${k}">${m} ${l} <b>${nf(byKind[k])}</b></button>`).join('')
  const box = `<input class="wp-in br-pq" id="bs-q" type="search" placeholder="Find a source — title, sender, file" value="${esc(s.q)}" aria-label="Find a source">`
  // Who may speak: one line, open for the senders and the decisions waiting.
  const sd = ((m365.state || {}).senders || {}).clients || {}
  const waiting = Object.values(sd).reduce((n, l) => n + (l.waiting || []).length + (l.unverified || []).length, 0)
  const anySenders = Object.values(sd).some((l) => (l.waiting || []).length || (l.unverified || []).length || (l.trusted || []).length || (l.ignored || []).length)
  const who = anySenders ? srcAcc('senders', `<b>Who may speak</b><span class="n">mail and posts are read only from you, colleagues and senders you trust</span><span class="grow"></span>${waiting ? `<span class="warn">● ${waiting} waiting</span>` : ''}`, () => brainSenders()) : ''
  if (s.q.trim()) {
    const k = 'q'
    if (!s.lists[k]) setTimeout(() => loadSrcList(k), 0)
    const l = s.lists[k]
    return `<div class="br-graph-bar">${box}${chips}</div><div class="bs-acc"><div class="bs-h static"><b>${l && !l.loading ? `${nf(l.total)} match${l.total === 1 ? '' : 'es'}` : 'Searching…'}</b><span class="n">newest first</span></div><div class="bs-body">${srcListBody(k, { client: true })}</div></div>`
  }
  const KIND = Object.fromEntries(SRC_KINDS.map(([k, m, l]) => [k, [m, l]]))
  const clients = (g.clients || []).map((c) => {
    const kinds = (c.kinds || []).filter((k) => !s.kind || k.kind === s.kind)
    if (!kinds.length) return ''
    const docs = kinds.reduce((n, k) => n + k.docs, 0), secs = kinds.reduce((n, k) => n + k.sections, 0)
    const body = () => {
      const rows = []
      for (const k of kinds.filter((k) => k.kind !== 'channel' && k.kind !== 'file')) {
        const [m, l] = KIND[k.kind] || ['·', k.kind]
        rows.push(srcAcc(`k|${c.client}|${k.kind}`, `${m} <b>${esc(l)}</b><span class="grow"></span><span class="n">${nf(k.docs)} · newest first</span>`, () => srcListBody(`k|${c.client}|${k.kind}`), 1))
      }
      if (!s.kind || s.kind === 'channel') {
        for (const ch of c.channels || []) rows.push(srcAcc(`ch|${c.client}|${ch.name}`, `# <b>${esc(ch.name)}</b><span class="grow"></span><span class="n">${nf(ch.posts)} post${ch.posts === 1 ? '' : 's'}</span>`, () => srcListBody(`ch|${c.client}|${ch.name}`, { channel: true }), 1))
      }
      if (!s.kind || s.kind === 'file') {
        for (const f of c.folders || []) rows.push(srcAcc(`f|${c.client}|${f.root}`, `▣ <b title="${esc(f.root)}">${esc(f.name)}</b><span class="grow"></span><span class="n">${esc(folderSummary(f.root).n ? folderSummary(f.root).line : `${nf(f.docs)} read`)}</span>`, () => srcListBody(`f|${c.client}|${f.root}`), 1))
      }
      return rows.join('')
    }
    const what = c.client === '-' ? 'your practice, internal calls' : 'client'
    return srcAcc(`c|${c.client}`, `<b>${esc(c.name)}</b><span class="n">${what}</span><span class="grow"></span><span class="n">${nf(docs)} source${docs === 1 ? '' : 's'} · ${nf(secs)} sections</span>`, body)
  }).join('')
  return `<div class="br-graph-bar">${box}${chips}</div>${who}${clients || '<p class="set-sub">No sources in this scope yet.</p>'}<p class="set-foot">Everything starts closed: a client opens to its kinds, channels and folders, each to its newest 50. A search lists what matches across all of them.</p>`
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-bs-toggle],[data-bs-more],[data-bs-kind]')
  if (!t) return
  const s = srcState(), d = t.dataset
  if (d.bsKind !== undefined) { s.kind = d.bsKind; if (s.q.trim()) delete s.lists.q; return render() }
  if (d.bsMore) return loadSrcList(d.bsMore, true)
  const k = d.bsToggle
  if (s.open.has(k)) s.open.delete(k)
  else { s.open.add(k); if (/^(k|ch|f)\|/.test(k) && !s.lists[k]) return loadSrcList(k) }
  render()
})
let srcSearchTimer = null
document.addEventListener('input', (e) => {
  if (e.target.id !== 'bs-q') return
  const s = srcState()
  s.q = e.target.value
  delete s.lists.q
  clearTimeout(srcSearchTimer)
  srcSearchTimer = setTimeout(render, 300)
})
/** The numbers lane, per client: its reporting families (Claude proposes them from the workbooks;
 *  nothing is built until accepted), what each loaded, the catalog of governed metrics, and what the
 *  last build warns about (a file that yielded nothing, figures restated or in conflict, holes). */
function brainNumbers() {
  const d = S.br.numbers
  if (!d) return '<div class="skeleton"><i></i><i></i></div>'
  const busy = d.busy || {}
  const scope = S.br.scope && S.br.scope.startsWith('client:') ? S.br.scope.slice(7) : null
  const cs = (d.clients || []).filter((c) => !scope || c.client === scope)
  if (!cs.length) return '<p class="set-sub">No reporting workbooks yet. Add a folder to a client\'s thread (Profile → Sources → Folder); its spreadsheets show up here.</p>'
  return cs.map((c) => {
    const cfg = c.config || {}, fams = cfg.families || [], st = c.status
    const job = busy[c.client], err = busy[`${c.client}:error`]
    const units = c.units || []
    const famStat = (name) => { const u = units.filter((x) => x.family === name); const k = (s) => u.filter((x) => x.status === s).length; return u.length ? `${k('ok')} ok${k('partial') ? ` · ${k('partial')} partial` : ''}${k('zero') + k('error') ? ` · <span class="error">${k('zero') + k('error')} empty</span>` : ''}` : '' }
    const rounds = ((cfg._echo || {}).rounds || []), lastRound = (cfg._echo || {}).kept || rounds[rounds.length - 1]
    const state = job ? `<span class="set-sub">${job.op === 'propose' ? 'Claude is reading the workbooks and proposing families — a few minutes…' : job.op === 'accept' ? 'Accepting and building…' : 'Building…'}</span>`
      : st === 'accepted' ? `<span class="tk-pj">accepted</span><span class="set-sub">${c.built_at ? `built ${esc(relDay(c.built_at))} · ${c.audit ? c.audit.facts : 0} facts` : 'not built yet'}</span>`
      : st === 'proposed' ? `<span class="tk-pj br-prop">proposed</span><span class="set-sub">${lastRound ? `loads ${lastRound.ok} of ${lastRound.units} sheets fully · ${lastRound.facts} facts — review, then accept` : 'review, then accept'}</span>` : `<span class="set-sub">${c.workbooks} workbook${c.workbooks === 1 ? '' : 's'} · no families yet</span>`
    const acts = job ? '' : [
      `<button class="btn${st ? '' : ' primary'}" data-num="propose" data-client="${esc(c.client)}">${st ? 'Propose again' : 'Propose families'}</button>`,
      st === 'proposed' ? `<button class="btn primary" data-num="accept" data-client="${esc(c.client)}">Accept and build</button>` : '',
      st === 'accepted' ? `<button class="btn" data-num="build" data-client="${esc(c.client)}">Build now</button>` : '',
    ].join('')
    const famRows = fams.map((f) => `<tr><td><b>${esc(f.name)}</b><div class="set-sub">${esc([].concat(f.glob).join(' · '))}</div></td><td>${esc(f.layout)}</td><td>${esc(Object.keys(f.grains || {}).concat(Object.keys(f.dim_candidates || {})).join(', ') || '—')}</td><td title="${esc(Object.keys(f.measures || {}).join(', '))}">${Object.keys(f.measures || {}).length}</td><td>${famStat(f.name)}</td></tr>`).join('')
    const cat = Object.entries(c.catalog || {})
    const catRows = cat.map(([n, m]) => `<tr><td><b>${esc(n)}</b></td><td>${esc(m.unit || '')}</td><td>${esc(m.desc || '')}${m.definition ? `<div class="set-sub">${esc(m.definition)}</div>` : ''}</td></tr>`).join('')
    const warn = units.filter((x) => x.status === 'zero' || x.status === 'error' || x.status === 'partial').slice(0, 8)
    const a = c.audit || {}
    const notes = [a.restated ? `${a.restated} figures restated by a later report` : '', a.conflicts ? `${a.conflicts} where reports of one month disagree` : '', a.holes ? `${a.holes} grains with an entity missing some months` : '', a.collisions ? `${a.collisions} rows listed twice in one file (${a.collisions_merged} merged by policy)` : ''].filter(Boolean)
    const skipped = ((cfg._echo || {}).skipped || [])
    return `<section class="set-sec num-sec">
      <div class="ins-h"><h3>${esc(clientName(c.client))}</h3>${state}<span class="grow"></span>${acts}</div>
      ${err ? `<p class="error">${esc(err)}</p>` : ''}
      ${fams.length ? `<div class="pp-card"><table class="pp-tbl"><thead><tr><th>Family</th><th>Layout</th><th>Grains</th><th class="num">Measures</th><th>Last build</th></tr></thead><tbody>${famRows}</tbody></table></div>` : ''}
      ${notes.length || warn.length ? `<div class="num-warn">${notes.map((n) => `<div>⚠ ${esc(n)}</div>`).join('')}${warn.map((x) => `<div class="set-sub">${esc(x.status)} · ${esc(x.family)} / ${esc(x.unit)} · ${esc(String(x.file || '').split('/').pop())} — ${esc(x.reason || '')}</div>`).join('')}</div>` : ''}
      ${cat.length ? `<details class="num-cat"><summary>${cat.length} governed metrics — what Claude's sessions can ask for (list_metrics, get_metric)</summary><div class="pp-card"><table class="pp-tbl"><tbody>${catRows}</tbody></table></div></details>` : ''}
      ${skipped.length ? `<details class="num-cat"><summary>${skipped.length} workbooks left out</summary>${skipped.map((s) => `<div class="set-sub">${esc(String(s.series || '').split('/').pop())} — ${esc(s.why || '')}</div>`).join('')}</details>` : ''}
    </section>`
  }).join('') + '<p class="set-foot">Numbers are computed from the workbooks\' real cells and cited to their file — never read or estimated by a model. Claude proposes which tables to load; nothing is built until you accept. Rebuilt when the workbooks change.</p>'
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-num]')
  if (!b) return
  send({ cmd: 'brain_numbers', op: b.dataset.num, client: b.dataset.client })
  b.disabled = true
  const poll = () => get('api/brain/numbers').then((d) => { S.br.numbers = d; render(); if (Object.keys(d.busy || {}).some((k) => !k.endsWith(':error'))) setTimeout(poll, 3000) }).catch(() => {})
  setTimeout(poll, 600)
})

function brainCoverage(tx) {
  const c = tx.coverage
  if (!c || !c.total) return '<p class="set-sub">Nothing to cover yet.</p>'
  const pct = (n) => Math.round(100 * n / c.total)
  const bar = (label, n, cls) => `<div class="br-cov"><span>${label}</span><span class="br-bar"><i class="${cls}" style="width:${pct(n)}%"></i></span><span class="br-n">${n} · ${pct(n)}%</span></div>`
  return `<section class="set-sec"><div class="set-card br-covs">${bar('Tagged with a topic', c.tagged, 'ok')}${bar('Small talk', c.no_topic, 'mute')}${bar('Nothing fits', c.nothing_fits, 'warn')}${bar('Not tagged yet', c.not_judged, 'mute')}</div>
    <p class="set-foot">Of ${c.total} sections. New sections are tagged after each sync; "nothing fits" growing means the topics need another one — rebuild a layer with <code>echoctl brain taxonomy build --layer …</code>.</p></section>`
}

