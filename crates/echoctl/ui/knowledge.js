// Knowledge in the Echo window (spec tasks/knowledge/spec.md): your vaults — folders of Markdown you own — read,
// edited, graphed and asked, and the minutes of your calls saved into them. Loaded before app.js; it uses app.js's
// helpers (S, esc, get, send, render, go, ICON …) only when called, never while it loads. Everything shown that isn't
// Echo's own words is escaped; a page's HTML comes from Echo's safe writer (`minutes::page_html`).
'use strict'

// ── kn-route: `#kn/<vault>/<tab>/<path>` (a page's path encoded as one part) ──
const KN_TABS = ['pages', 'graph', 'ask', 'health', 'settings']
/** `decodeURIComponent`, or `null` for a broken escape (a `%` that is the name's own). */
function knDecode(s) {
  try { return decodeURIComponent(s) } catch { return null }
}
function knParseRoute(hash) {
  const parts = String(hash || '').split('/')
  if (parts[0] !== 'kn') return null
  const vault = parts[1] ? knDecode(parts[1]) : ''
  if (vault === null) return null
  if (vault && !/^[\p{L}\p{N}_-]+$/u.test(vault)) return null
  const tab = parts[2] || 'pages'
  if (!KN_TABS.includes(tab)) return null
  const path = parts.length > 3 ? knDecode(parts.slice(3).join('/')) : ''
  if (path === null) return null
  return { vault, tab, path }
}
function knRouteOf({ vault, tab = 'pages', path = '' }) {
  return `kn/${encodeURIComponent(vault)}/${tab}${path ? '/' + encodeURIComponent(path) : ''}`
}
// end of the kn-route block

// ── kn-tabs: which tabs show a number (only where something waits) ──
/** `{ ingest, health }`: what waits to be ingested and what needs a look; `null` at 0 or when missing, so no badge. */
function knTabCounts({ waiting, needs } = {}) {
  const n = (v) => (Number.isFinite(v) && v > 0 ? v : null)
  return { ingest: n(waiting), health: n(needs) }
}
// end of the kn-tabs block

// ── kn-switcher: what the header's switcher lists ──
function knSwitcherEntries(vaults, recallOn, pluginTabs) {
  const out = [{ kind: 'head', label: 'Your vaults' }]
  for (const v of vaults) out.push({ kind: 'vault', id: v.id, name: v.name, pages: v.pages, star: !!(v.minutes && v.minutes.mode === 'auto'), available: v.available !== false })
  out.push({ kind: 'connect', label: 'Connect a folder…' }, { kind: 'new', label: 'Start a new vault…' })
  if (recallOn) out.push({ kind: 'sep' }, { kind: 'head', label: "Echo's index" }, { kind: 'recall', label: 'Recall' })
  if (pluginTabs.length) {
    out.push({ kind: 'sep' }, { kind: 'head', label: 'From plugins' })
    for (const t of pluginTabs) out.push({ kind: 'plugin', tab: t.tab, label: t.label, plugin: t.plugin })
  }
  return out
}
// end of the kn-switcher block

// ── kn-chip: a call's Knowledge chip (`call.knowledge` from /api/call) ──
function knChipState(k) {
  if (!k) return { state: 'none' }
  const rec = k.record || {}
  const saved = rec.saved || []
  const byId = (id) => (k.vaults || []).find((v) => v.id === id)
  if (k.failure) return { state: 'failed', label: `Couldn't save to ${k.failure.name}`, vault: k.failure.vault, error: k.failure.error }
  const stale = (k.stale || [])[0]
  if (stale && byId(stale)) return { state: 'stale', label: `Update in ${byId(stale).name}`, vault: stale }
  const kept = saved.find((e) => e.state === 'kept')
  if (kept) return { state: 'kept', label: `${(byId(kept.vault) || { name: kept.vault }).name} · not updated`, vault: kept.vault, path: kept.path }
  if (saved.length) {
    const e = saved[saved.length - 1]
    const v = byId(e.vault)
    if (!v) return { state: 'removed', label: 'Saved to a vault since removed', vault: e.vault }
    return { state: 'saved', label: `In ${v.name}`, vault: e.vault, path: e.path, more: saved.length - 1 }
  }
  if (rec.choice === 'none') return { state: 'off', label: 'Not saved to Knowledge' }
  const t = k.targets || []
  if (t.length) return { state: 'will', label: `Minutes → ${t.map((v) => v.name).join(', ')}`, vault: t[0].id }
  return { state: 'none' }
}
// end of the kn-chip block

// ── kn-tree: the Pages tree from the vault's pages (paths vault-relative; `root` the pages root) ──
function knBuildTree(pages, { root = '', filter = 'all', open = new Set(), index = '' } = {}) {
  const rel = (p) => (root && p.startsWith(root + '/') ? p.slice(root.length + 1) : p)
  const name = (p) => p.split('/').pop()
  if (filter === 'recent' || filter === 'unindexed') {
    const list = filter === 'recent' ? [...pages].sort((a, b) => (b.mtime || 0) - (a.mtime || 0)) : pages.filter((p) => p.in_index === false)
    return list.map((p) => ({ kind: 'page', path: p.path, name: name(p.path), title: p.title, depth: 0, sub: rel(p.path).split('/').slice(0, -1).join('/') }))
  }
  const rows = []
  const walk = (prefix, depth) => {
    const here = pages.filter((p) => { const r = rel(p.path); return prefix ? r.startsWith(prefix + '/') : true })
    const folders = new Map(), files = []
    for (const p of here) {
      const rest = rel(p.path).slice(prefix ? prefix.length + 1 : 0)
      const slash = rest.indexOf('/')
      if (slash >= 0) { const f = rest.slice(0, slash); folders.set(f, (folders.get(f) || 0) + 1) } else files.push(p)
    }
    for (const [f, n] of [...folders].sort((a, b) => a[0].localeCompare(b[0]))) {
      const full = prefix ? `${prefix}/${f}` : f
      const vpath = root ? `${root}/${full}` : full
      const isOpen = open.has(vpath)
      rows.push({ kind: 'folder', path: vpath, name: f, depth, count: n, open: isOpen })
      if (isOpen) walk(full, depth + 1)
    }
    // The vault's index page (`index`, vault-relative) is marked.
    for (const p of files.sort((a, b) => name(a.path).localeCompare(name(b.path)))) rows.push({ kind: 'page', path: p.path, name: name(p.path), title: p.title, depth, index: !!index && p.path === index })
  }
  walk('', 0)
  return rows
}
// end of the kn-tree block

// ── kn-graph: which nodes and edges the graph shows ──
function knFilterGraph(g, { hideIndex = true, isolated = true, tags = false, focus = null, depth = 0, hidden = [] } = {}) {
  const hubs = new Set(hideIndex ? g.hubs || [] : [])
  const off = new Set(hidden)
  let nodes = g.nodes.filter((n) => !hubs.has(n.id) && (tags || !n.tag) && (n.tag || !off.has(n.folder || '(top)')))
  const keep = new Set(nodes.map((n) => n.id))
  let edges = g.edges.filter(([a, b]) => keep.has(a) && keep.has(b))
  if (!isolated) {
    const linked = new Set(edges.flat())
    nodes = nodes.filter((n) => linked.has(n.id) || n.tag)
  }
  if (focus && depth > 0 && keep.has(focus)) {
    const near = new Set([focus])
    for (let d = 0; d < depth; d++) {
      const was = new Set(near)
      for (const [a, b] of edges) { if (was.has(a)) near.add(b); if (was.has(b)) near.add(a) }
    }
    nodes = nodes.filter((n) => near.has(n.id))
    edges = edges.filter(([a, b]) => near.has(a) && near.has(b))
  }
  const shown = new Set(nodes.map((n) => n.id))
  return { nodes, edges: edges.filter(([a, b]) => shown.has(a) && shown.has(b)) }
}
/** A graph of more than 5,000 pages starts as the open page's local graph (else the index's); `null`: the whole. */
function knGraphStart(pages, path, index) {
  return pages > 5000 ? { depth: 1, sel: path || index || null } : null
}
/** How many pages "Show isolated pages" hides: no page links in or out (when the index is hidden, its own don't count;
 *  with tags as nodes, a tag's do), of the folders shown. */
function knIsolatedCount(g, hideIndex, hidden = [], tags = false) {
  const pages = (isolated) => knFilterGraph(g, { hideIndex, isolated, hidden, tags }).nodes.filter((n) => !n.tag).length
  return pages(true) - pages(false)
}
// end of the kn-graph block

// ── kn-force: the graph's live layout, forces as d3-force has them (many-body, links, a pull to the centre) ──
/** Push every node away from the others into its velocity (`strength` > 0). Past 64 nodes, a cell farther away than
 *  its size / `theta` counts as one body at its centre of mass (Barnes–Hut); `theta` 0 is the exact sum. */
function knRepel(nodes, strength, theta) {
  const n = nodes.length
  if (n < 2) return
  const near = (p, q, i, j) => {
    let dx = q.x - p.x, dy = q.y - p.y, d2 = dx * dx + dy * dy
    if (d2 < 1e-6) { dx = 0.1 * ((j - i) || 1); dy = 0.1 * (((i * 7 + j * 3) % 5) - 2 || 1); d2 = dx * dx + dy * dy } // on one spot: apart, the same way each time
    const f = strength / Math.max(d2, 16)
    return [dx * f, dy * f]
  }
  if (!theta || n < 64) {
    for (let i = 0; i < n; i++) {
      const p = nodes[i]
      for (let j = i + 1; j < n; j++) {
        const q = nodes[j], [fx, fy] = near(p, q, i, j)
        p.vx -= fx; p.vy -= fy; q.vx += fx; q.vy += fy
      }
    }
    return
  }
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
  for (const p of nodes) { x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y); x1 = Math.max(x1, p.x); y1 = Math.max(y1, p.y) }
  const cell = (x, y, s) => ({ x, y, s, n: 0, sx: 0, sy: 0, k: null, b: null })
  const sub = (c, p) => {
    const h = c.s / 2, i = (p.x >= c.x + h ? 1 : 0) + (p.y >= c.y + h ? 2 : 0)
    return c.k[i] || (c.k[i] = cell(c.x + (i & 1 ? h : 0), c.y + (i & 2 ? h : 0), h))
  }
  const add = (c, p, d) => {
    for (;;) {
      c.n++; c.sx += p.x; c.sy += p.y
      if (c.k) { c = sub(c, p); d++; continue }
      if (!c.b) { c.b = [p]; return }
      if (d > 40) { c.b.push(p); return } // the same spot: one leaf holds them all
      const q = c.b[0]
      c.b = null; c.k = [null, null, null, null]
      add(sub(c, q), q, d + 1)
      c = sub(c, p); d++
    }
  }
  const root = cell(x0, y0, Math.max(x1 - x0, y1 - y0, 1) * 1.0001)
  const at = new Map()
  nodes.forEach((p, i) => { at.set(p, i); add(root, p, 0) })
  const t2 = theta * theta, stack = []
  nodes.forEach((p, i) => {
    stack.push(root)
    while (stack.length) {
      const c = stack.pop()
      if (c.b) {
        for (const q of c.b) if (q !== p) { const [fx, fy] = near(p, q, i, at.get(q)); p.vx -= fx; p.vy -= fy }
        continue
      }
      const dx = c.sx / c.n - p.x, dy = c.sy / c.n - p.y, d2 = dx * dx + dy * dy
      if (c.s * c.s < t2 * d2) { const f = (strength * c.n) / Math.max(d2, 16); p.vx -= dx * f; p.vy -= dy * f; continue }
      for (const k of c.k) if (k) stack.push(k)
    }
  })
}
/** A layout that moves: `set(nodes, links, heat)` keeps where known pages were (a page filtered out and back returns to
 *  its place; a new one starts beside the pages it links to) and warms it by `heat` (a first layout: fully), `tick()`
 *  steps it, and it cools until `settled()`. A dragged page is `fix`ed under the pointer while `hold(true)` keeps the
 *  rest moving round it. */
function knForceSim(opts) {
  const o = { distance: 30, charge: 34, gravity: 0.05, decay: 0.6, theta: 0.9, ...opts }
  let nodes = [], links = [], byId = new Map(), alpha = 1, target = 0
  const seen = new Map()
  function set(list, edges, heat = 0.5) {
    for (const p of nodes) seen.set(p.id, { x: p.x, y: p.y })
    const fresh = []
    byId = new Map()
    nodes = list.map((n) => {
      const was = seen.get(n.id)
      const p = { ...n, x: was ? was.x : 0, y: was ? was.y : 0, vx: 0, vy: 0, fx: null, fy: null, nb: [], deg: 0, r: 0 }
      if (!was) fresh.push(p)
      byId.set(p.id, p)
      return p
    })
    links = []
    const twice = new Set()
    for (const [a, b] of edges) {
      const s = byId.get(a), t = byId.get(b), key = a < b ? `${a}\n${b}` : `${b}\n${a}`
      if (!s || !t || s === t || twice.has(key)) continue
      twice.add(key); links.push({ s, t }); s.nb.push(t); t.nb.push(s)
    }
    for (const p of nodes) { p.deg = p.nb.length; p.r = p.size || 3 + Math.sqrt(p.deg) * 1.4 }
    for (const l of links) { l.k = 1 / Math.min(l.s.deg, l.t.deg); l.bias = l.s.deg / (l.s.deg + l.t.deg) }
    // New pages: beside the placed pages they link to, else on a sunflower spiral (no two on one spot).
    const placed = new Set(nodes.filter((p) => !fresh.includes(p)))
    fresh.forEach((p, i) => {
      const by = p.nb.filter((m) => placed.has(m))
      if (by.length) {
        p.x = by.reduce((s, m) => s + m.x, 0) / by.length + 6 * Math.cos(i * 2.39996)
        p.y = by.reduce((s, m) => s + m.y, 0) / by.length + 6 * Math.sin(i * 2.39996)
      } else {
        const r = 10 * Math.sqrt(0.5 + i + placed.size), a = (i + placed.size) * 2.39996
        p.x = r * Math.cos(a); p.y = r * Math.sin(a)
      }
      placed.add(p)
    })
    alpha = Math.max(alpha, fresh.length === nodes.length ? 1 : heat)
  }
  function stepLayout() {
    knRepel(nodes, o.charge * alpha, nodes.length > 600 ? o.theta : 0)
    for (const l of links) {
      const a = l.s, b = l.t
      let dx = b.x + b.vx - a.x - a.vx, dy = b.y + b.vy - a.y - a.vy
      const d = Math.sqrt(dx * dx + dy * dy) || 0.01
      const f = ((d - o.distance - a.r - b.r) / d) * alpha * l.k
      dx *= f; dy *= f
      b.vx -= dx * l.bias; b.vy -= dy * l.bias; a.vx += dx * (1 - l.bias); a.vy += dy * (1 - l.bias)
    }
    const g = o.gravity * alpha
    for (const p of nodes) {
      if (p.fx != null) { p.x = p.fx; p.y = p.fy; p.vx = p.vy = 0; continue }
      const gp = p.deg ? g : g * 1.6 // a page with no links rings the rest, not far out
      p.vx -= p.x * gp; p.vy -= p.y * gp
      p.x += (p.vx *= o.decay); p.y += (p.vy *= o.decay)
    }
    alpha += (target - alpha) * 0.0228
  }
  return {
    set, tick: stepLayout,
    forget() { seen.clear(); nodes = []; links = []; byId = new Map(); alpha = 1 },
    nodes: () => nodes, links: () => links, node: (id) => byId.get(id) || null,
    alpha: () => alpha,
    settled: () => !target && alpha < 0.003,
    reheat(a) { alpha = Math.max(alpha, a) },
    hold(on) { target = on ? 0.25 : 0; if (on) alpha = Math.max(alpha, 0.25) },
    fix(id, x, y) { const p = byId.get(id); if (p) { p.fx = x; p.fy = y; p.x = x; p.y = y } },
    free(id) { const p = byId.get(id); if (p) { p.fx = null; p.fy = null } },
  }
}
/** Which labels to draw: the most important first (hovered, dragged or selected: p 3, never skipped; neighbours and
 *  matches: 2; then the biggest pages), each skipped where it would overlap one placed or falls off the canvas. */
function knLabelPlan(cands, W, H, max = 300) {
  const out = []
  for (const c of cands.slice().sort((a, b) => b.p - a.p || b.r - a.r)) {
    const b = { x0: c.x - c.w / 2 - 2, x1: c.x + c.w / 2 + 2, y0: c.y - 1, y1: c.y + c.h + 2 }
    if (b.x1 < 0 || b.x0 > W || b.y0 > H || b.y1 < 0) continue
    if (c.p < 3 && (out.length >= max || out.some((q) => b.x0 < q.b.x1 && b.x1 > q.b.x0 && b.y0 < q.b.y1 && b.y1 > q.b.y0))) continue
    out.push({ ...c, b })
  }
  return out
}
/** The pages whose title or path has every word typed; `null` when nothing is typed. */
function knGraphHits(nodes, q) {
  const words = String(q || '').toLowerCase().split(/\s+/).filter(Boolean)
  if (!words.length) return null
  return new Set(nodes.filter((n) => { const t = `${n.title || ''} ${n.id}`.toLowerCase(); return words.every((w) => t.includes(w)) }).map((n) => n.id))
}
/** A label short enough to sit under its dot. */
function knShortLabel(text, max = 30) {
  const ch = [...String(text || '')]
  return ch.length <= max ? ch.join('') : `${ch.slice(0, max - 1).join('').trimEnd()}…`
}
// end of the kn-force block

// ── kn-prompt: text Echo types into a session (a page's path, a skill's name) — never a control character, which
// would press Enter or move the cursor ──
function knPlainPrompt(s) {
  return String(s).replace(/[\u0000-\u001f\u007f]+/g, ' ').trim()
}
// end of the kn-prompt block

// ── kn-draft: unsaved text kept per page across a reload (dropped when the file changed meanwhile) ──
function knDraftKey(vault, path) { return `echo.kn.draft:${vault}:${path}` }
function knKeepDraft(store, vault, path, base, text) {
  try { store.setItem(knDraftKey(vault, path), JSON.stringify({ base, text })) } catch { /* storage off */ }
}
function knRestoreDraft(store, vault, path, base) {
  try { const d = JSON.parse(store.getItem(knDraftKey(vault, path)) || 'null'); return d && d.base === base ? d.text : null } catch { return null }
}
function knDropDraft(store, vault, path) {
  try { store.removeItem(knDraftKey(vault, path)) } catch { /* storage off */ }
}
/** The text a page's editor starts from: its unsaved text kept for this base, else the file's `text`. */
function knEditText(store, vault, path, base, text) {
  const kept = store ? knRestoreDraft(store, vault, path, base) : null
  return kept != null ? kept : text
}
// end of the kn-draft block

// ── kn-diff: your text against the file on disk, line by line — ['=' | '-' (only yours) | '+' (only on disk), line], runs
// of the same lines folded to `context` around a change (['…', 'N lines the same']); null when too big to compare here ──
function knLineDiff(mine, theirs, context = 3) {
  const a = String(mine).split('\n'), b = String(theirs).split('\n')
  let pre = 0
  while (pre < a.length && pre < b.length && a[pre] === b[pre]) pre++
  let post = 0
  while (post < a.length - pre && post < b.length - pre && a[a.length - 1 - post] === b[b.length - 1 - post]) post++
  const x = a.slice(pre, a.length - post), y = b.slice(pre, b.length - post)
  if (x.length * y.length > 2.5e6) return null
  // The longest common run of lines, from the end.
  const L = Array.from({ length: x.length + 1 }, () => new Uint32Array(y.length + 1))
  for (let i = x.length - 1; i >= 0; i--) for (let j = y.length - 1; j >= 0; j--) L[i][j] = x[i] === y[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1])
  const ops = a.slice(0, pre).map((l) => ['=', l])
  let i = 0, j = 0
  while (i < x.length && j < y.length) {
    if (x[i] === y[j]) { ops.push(['=', x[i]]); i++; j++ } else if (L[i + 1][j] >= L[i][j + 1]) ops.push(['-', x[i++]]); else ops.push(['+', y[j++]])
  }
  while (i < x.length) ops.push(['-', x[i++]])
  while (j < y.length) ops.push(['+', y[j++]])
  for (const l of a.slice(a.length - post)) ops.push(['=', l])
  const changed = ops.map((o) => o[0] !== '=')
  const near = ops.map((_, k) => changed.slice(Math.max(0, k - context), k + context + 1).some(Boolean))
  const out = []
  for (let k = 0; k < ops.length;) {
    if (near[k] || !changed.some(Boolean)) { out.push(ops[k++]); continue }
    let n = 0
    while (k < ops.length && !near[k]) { n++; k++ }
    out.push(['…', `${n} line${n === 1 ? '' : 's'} the same`])
  }
  return out
}
// end of the kn-diff block

// ── kn-chat: an Ask conversation's events (spec §5.3) folded into turns — a message and what Claude did until its result ──
/** Turns `{ user, blocks, result, suggestion }`, in order; what came before any message (a history notice) is a turn
 *  without `user`. A block is `thinking` or `text` (one per streamed block: a later render replaces the one before until
 *  it is done; superseded renders and a sub-agent's own words are skipped), `tools` (a run of calls, each with its
 *  `result`), `permission` (with its `answer`), or a `notice` (its own kind in `what`), `error` or `todo`. */
function knChatTurns(events) {
  const turns = [], calls = new Map(), cards = new Map()
  let t = null
  const cur = () => t || (turns.push(t = { user: null, blocks: [], result: null, suggestion: null }), t)
  for (const e of events || []) {
    if (!e || e.superseded) continue
    switch (e.type) {
      case 'user': turns.push(t = { user: e, blocks: [], result: null, suggestion: null }); break
      case 'thinking': case 'text': {
        if (e.parent) break
        const bs = cur().blocks, b = { kind: e.type, block: e.block, n: e.n, html: e.html, md: e.md, text: e.text, done: !!e.done, seconds: e.seconds }
        // A process numbers its blocks from b1: only a block still streaming is this one's earlier render.
        const i = bs.findIndex((x) => x.kind === e.type && x.block === e.block && !x.done)
        if (i >= 0) bs[i] = b; else bs.push(b)
        break
      }
      case 'tool': {
        const bs = cur().blocks
        let g = bs[bs.length - 1]
        if (!g || g.kind !== 'tools') bs.push(g = { kind: 'tools', n: e.n, items: [] })
        const item = { ...e, result: null }
        g.items.push(item)
        calls.set(e.id, item)
        break
      }
      case 'tool_result': { const c = calls.get(e.id); if (c) c.result = e; break }
      case 'permission': { const b = { ...e, kind: 'permission', answer: null }; cur().blocks.push(b); cards.set(e.request, b); break }
      case 'answered': { const b = cards.get(e.request); if (b) b.answer = e; break }
      case 'result': cur().result = e; break
      case 'suggestion': cur().suggestion = e.text; break
      case 'notice': case 'error': case 'todo': cur().blocks.push({ ...e, kind: e.type, what: e.kind }); break
      // `denied`: the call's own row says why.
    }
  }
  return turns
}
/** Did the conversation read web pages or others' words (Research, an attached call)? Then the terminal panel says so
 *  first (D41). */
function knChatReadOutside(events) {
  return (events || []).some((e) => e && ((e.type === 'tool' && (e.name === 'WebSearch' || e.name === 'WebFetch')) || (e.type === 'user' && (e.context || []).some((c) => c && c.call))))
}
/** Follow the bottom only while there (K13-10): `top`, `height`, `view` are the thread's scrollTop, scrollHeight and
 *  clientHeight before `event` is drawn (`null`: the user scrolled), `jump` whether "Jump to the latest" shows now. */
function knFollow({ top = 0, height = 0, view = 0, jump = false } = {}, event) {
  const bottom = height - top - view <= 32
  if (!event) return { scroll: false, jump: bottom ? false : !!jump }
  return { scroll: bottom, jump: !bottom }
}
/** An answer from Ask in a page's Markdown (K13-06): with `source` in its properties (Save as a page, the default page
 *  format has an empty one), or under a dated `heading` at its end (Add to a page). */
function knAnswerInto(text, md, { heading = '', source = '' } = {}) {
  let t = String(text || '')
  if (source) {
    const fm = /^---\r?\n([\s\S]*?)\r?\n---[^\S\r\n]*(\r?\n|$)/.exec(t)
    if (fm) {
      const lines = fm[1].split(/\r?\n/)
      const i = lines.findIndex((l) => /^source\s*:/.test(l))
      if (i >= 0) lines[i] = `source: ${source}`; else lines.push(`source: ${source}`)
      t = `---\n${lines.join('\n')}\n---\n${t.slice(fm[0].length)}`
    } else t = `---\nsource: ${source}\n---\n\n${t}`
  }
  return `${t.replace(/\s+$/, '')}\n\n${heading ? `## ${heading}\n\n` : ''}${String(md || '').trim()}\n`
}
/** A turn's answer as Claude wrote it: its finished text blocks' Markdown (`md`, ruling R33), for Copy, Save as a
 *  page and Add to a page. */
function knAnswerMd(turn) {
  return ((turn && turn.blocks) || []).filter((b) => b.kind === 'text' && b.md).map((b) => b.md.trim()).join('\n\n')
}
/** The last turn's first card still waiting (⌘↵ allows it, Esc denies it): not answered by Echo yet, nor from this
 *  page (`local`: requests answered here before Echo's `answered` comes). */
function knWaitingCard(turns, live, local = {}) {
  const t = turns[turns.length - 1]
  return t && !t.result && live ? t.blocks.find((b) => b.kind === 'permission' && !b.answer && !local[b.request]) || null : null
}
/** What a key does in Ask: `allow` or `deny` the waiting card, `send`, `stop`, `blocked` (⌘↵ in a composer with text
 *  while a card waits: nothing is sent and the card isn't answered — that keystroke meant "send"), or null (not Ask's).
 *  Nothing while an input method composes. */
function knChatKey({ key, mod = false, composing = false, card = false, inComposer = false, typed = false, busy = false } = {}) {
  if (composing) return null
  if (key === 'Enter' && mod) return card ? (inComposer && typed ? 'blocked' : 'allow') : inComposer ? 'send' : null
  if (key === 'Escape') return card ? 'deny' : busy ? 'stop' : null
  return null
}
/** A message sent from the composer (`pending`): `landed` once a `user` event comes (the composer empties), `failed`
 *  when an error came instead (a sign-in needed: the text stays, to send again), else null. */
function knSentLanded(pending, events) {
  if (!pending) return null
  if (events.some((e) => e.type === 'user')) return 'landed'
  return events.some((e) => e.type === 'error') ? 'failed' : null
}
// end of the kn-chat block

// ── kn-poll: an Ask conversation's long poll — one loop per open conversation, each chunk merged once ──
/** Open conversation `id` ('' a new one): its events start afresh, and a loop still polling for the open before stops
 *  (a new `gen`); a new loop may start at once. The open's `gen`. */
function knChatOpen(c, id) {
  c.gen = (c.gen || 0) + 1
  Object.assign(c, { id: id || '', events: [], next: 0, renders: {}, loaded: false, live: false, busy: false, waiting: false, polling: null })
  return c.gen
}
/** May the loop started for open `gen` go on? */
function knPollKeep(c, gen) { return c.gen === gen }
/** The answer to a request made `from` event `from` by the loop of open `gen`, merged once: null when it is stale
 *  (another open since, or the view moved on from `from`), else what came — `{ events, sent, ended }`. A streaming
 *  block's older render drops its copy (as Echo's own list does); a new message starts the blocks' names afresh. */
function knChatTake(c, gen, from, r, now) {
  if (c.gen !== gen || from !== c.next) return null
  const evs = r.events || []
  // Echo started again: asked past its end, it answers from its first event.
  if (evs.length && evs[0].n < c.next) { c.events = []; c.renders = {} }
  for (const e of evs) {
    if (c.loaded) e.at = now
    if (e.type === 'user') c.renders = {}
    if ((e.type === 'text' || e.type === 'thinking') && e.block && !e.superseded) {
      const k = `${e.type}:${e.block}`, old = c.renders[k]
      if (old) { old.superseded = true; delete old.html; delete old.text }
      if (e.done) delete c.renders[k]; else c.renders[k] = e
    }
    c.events.push(e)
  }
  c.next = r.next
  const ended = c.busy && !r.busy
  Object.assign(c, { live: !!r.live, busy: !!r.busy, waiting: !!r.waiting, loaded: true })
  return { events: evs, sent: evs.some((e) => e.type === 'user'), ended }
}
// end of the kn-poll block

// ── kn-tools: a turn's tool calls as rows (K13-03) — one line each; three or more read-only calls in a row fold ──
const KN_FOLD = { Read: ['read', 'file', 'files'], Grep: ['searched', 'time', 'times'], Glob: ['searched', 'time', 'times'], WebSearch: ['searched the web', 'time', 'times'], WebFetch: ['opened', 'page', 'pages'], Skill: ['used', 'skill', 'skills'] }
/** One call: verb, target (`code` when it is a path or a name), `where` it searched, an edit's `pm`, the `note` after
 *  the dot (a read's lines, a search's count, a failure's reason), and `text`, the whole line in words. The events
 *  carry no result count for a web search and no file length for a read. */
function knToolRow(x, ended) {
  const d = x.detail || '', r = x.result
  const error = r && r.ok === false ? r.summary || 'It failed.' : ''
  const summary = r && !error ? r.summary || '' : ''
  let where = '', pm = '', note = d
  if (x.name === 'Read') note = d || summary
  else if (x.name === 'Grep' || x.name === 'Glob') { where = d; note = summary }
  else if (x.name === 'Edit') { pm = d; note = '' }
  if (error) note = error
  const text = [x.verb, x.target, where, pm].filter(Boolean).join(' ') + (note ? ` · ${note}` : '')
  return { kind: 'row', id: x.id, name: x.name, verb: x.verb, target: x.target, path: x.path, code: ['Read', 'Edit', 'Write', 'Glob', 'Skill', 'WebFetch'].includes(x.name), where, pm, note, error, running: !r && !ended, text, at: x.at, done: r && r.at, input: x.input, preview: r ? r.preview : '' }
}
function knFoldLabel(items) {
  const n = new Map()
  for (const x of items) { const k = (KN_FOLD[x.name] || [String(x.verb || x.name).toLowerCase(), 'time', 'times']).join('|'); n.set(k, (n.get(k) || 0) + 1) }
  const s = [...n].map(([k, c]) => { const [verb, one, many] = k.split('|'); return `${verb} ${c} ${c === 1 ? one : many}` }).join(', ')
  return s.charAt(0).toUpperCase() + s.slice(1)
}
/** The rows of a run of calls (each with its `result`): a row each, a `fold` (its `rows` inside) for three or more
 *  read-only calls in a row. `ended`: the turn is over. */
function knToolRows(items, { ended = false } = {}) {
  const out = []
  for (let i = 0; i < items.length;) {
    let j = i
    while (j < items.length && items[j].readonly) j++
    if (j - i >= 3) {
      const rows = items.slice(i, j).map((x) => knToolRow(x, ended))
      out.push({ kind: 'fold', id: rows[0].id, text: knFoldLabel(items.slice(i, j)), rows, running: rows.some((r) => r.running), errors: rows.filter((r) => r.error).length })
      i = j
    } else out.push(knToolRow(items[i++], ended))
  }
  return out
}
// end of the kn-tools block

// ── kn-mention: `@` in Ask's composer — a page picked from Echo's index goes in as a chip, its title in the words ──
/** The mention typed up to `caret`: `{ query, start }` (`start`: its `@`), or null. An `@` starts one only at the start
 *  or after a space (never in an address); a space ends it. */
function knMention(text, caret) {
  const before = String(text).slice(0, caret)
  const m = /(^|\s)@([^\s@]*)$/.exec(before)
  return m ? { query: m[2], start: before.length - m[2].length - 1 } : null
}
/** The text once mention `m` (typed up to `caret`) is picked: `words` in its place, and where the caret goes. */
function knMentionPick(text, m, caret, words) {
  const t = String(text), w = String(words)
  return { text: t.slice(0, m.start) + w + t.slice(caret), caret: m.start + w.length }
}
/** What `kn_chat_send` takes from the composer: the words, and its chips as pages and calls — a call by its name, never
 *  a path, with `anyway` only on the one the user said *Attach anyway* to. */
function knChatMessage(text, chips) {
  return { text: String(text).trim(), context: (chips || []).map((c) => c.page ? { page: c.page } : c.anyway ? { call: c.call, anyway: true } : { call: c.call }) }
}
// end of the kn-mention block

// ── kn-health: Health's findings by group and kind under a filter, the finding shown, the 30-day trend with its gaps,
// Fix all's items and what a fix did (spec K16). The page names a finding only by its key ──
const KN_HEALTH_GROUPS = [['needs', 'Needs you', 'only you can decide'], ['should', 'Should fix', 'most by code'], ['tidy', 'Tidy', 'when there is time']]
const knS = (n, one, many) => (n === 1 ? one : many)
/** A code check's kind: its name in the list, the heading of its findings (n of them) and, where *Fix all* applies, the
 *  sheet's title. */
const KN_HEALTH_KINDS = {
  not_in_index: ['Not in the index', (n) => `${n} ${knS(n, 'page isn\'t', 'pages aren\'t')} in the index`, (n) => `Add ${n} ${knS(n, 'row', 'rows')} to the index`],
  broken: ['Broken links', (n) => `${n} ${knS(n, 'link leads', 'links lead')} to no page`, (n) => `Fix ${n} broken ${knS(n, 'link', 'links')}`],
  isolated: ['Pages with no links', (n) => `${n} ${knS(n, 'page has', 'pages have')} no links`],
  no_props: ['Without properties', (n) => `${n} ${knS(n, 'page has', 'pages have')} no properties`, (n) => `Add properties to ${n} ${knS(n, 'page', 'pages')}`],
  unknown_tag: ['Tags not in the taxonomy', (n) => `${n} ${knS(n, 'tag isn\'t', 'tags aren\'t')} in the taxonomy`],
  meeting_tag_missing: ['The meeting tag', () => 'The taxonomy doesn\'t allow the meeting tag'],
  dead_row: ['Index rows to missing pages', (n) => `${n} index ${knS(n, 'row leads', 'rows lead')} to no page`, (n) => `Fix ${n} index ${knS(n, 'row', 'rows')}`],
  usual_prop: ['Missing a usual property', (n) => `${n} usual ${knS(n, 'property is', 'properties are')} missing`],
  duplicate_title: ['Pages that share a title', (n) => `${n} ${knS(n, 'title is', 'titles are')} on more than one page`],
  oversized_index: ['An oversized index', () => 'The index is too big to read first'],
}
/** The findings under `filter` — `all`, `needs` (Needs you), `code` or `ai` — by group, then by kind in Echo's order:
 *  each kind with its count `n`, its keys, how many have a fix (`fixable`) and how many *Fix all* takes (`fixAll`, never
 *  a guess). Empty groups are left out; the ignored findings are a group of their own with `showIgnored`. `kinds` and
 *  `needs` (findings in Needs you: the tab's number) count the findings not ignored, whatever the filter. */
function knHealthList(findings, { filter = 'all', ignored = [], showIgnored = false } = {}) {
  const keep = (f) => (filter === 'needs' ? f.group === 'needs' : filter === 'code' ? f.by === 'code' : filter === 'ai' ? f.by === 'ai' : true)
  const kindsOf = (list, ign) => {
    const by = new Map()
    for (const f of list) {
      let k = by.get(f.kind)
      if (!k) by.set(f.kind, (k = { kind: f.kind, by: f.by, group: f.group, ignored: ign, n: 0, keys: [], fixable: 0, fixAll: 0, name: (KN_HEALTH_KINDS[f.kind] || [f.title])[0], title: f.title, why: f.why }))
      k.n++
      k.keys.push(f.key)
      if (f.fixable) k.fixable++
      if (f.fix_all) k.fixAll++
    }
    return [...by.values()].map((k) => ({ ...k, meta: k.by === 'code' && k.fixable ? `${k.fixable} fixable by code` : k.n > 1 ? `${k.title} · ${k.n - 1} more` : k.title }))
  }
  const all = findings || [], shown = all.filter(keep)
  const groups = KN_HEALTH_GROUPS.map(([id, label, sub]) => ({ id, label, sub, kinds: kindsOf(shown.filter((f) => f.group === id), false) })).filter((g) => g.kinds.length)
  const off = showIgnored ? kindsOf((ignored || []).filter(keep), true) : []
  if (off.length) groups.push({ id: 'ignored', label: 'Ignored', sub: 'not counted', kinds: off })
  return { groups, kinds: new Set(all.map((f) => f.kind)).size, needs: all.filter((f) => f.group === 'needs').length }
}
/** The finding shown: `key` when it is listed, else the first of its kind (one just fixed or ignored), else the first
 *  listed — `{ kind, key }` (`kind`: its entry in `list`), or null when nothing is. */
function knHealthSelect(list, key) {
  const kinds = list.groups.flatMap((g) => g.kinds)
  const has = key ? kinds.find((k) => k.keys.includes(key)) : null
  if (has) return { kind: has, key }
  const k = (key && kinds.find((x) => x.kind === String(key).split(':')[0])) || kinds[0]
  return k ? { kind: k, key: k.keys[0] } : null
}
/** The trend: a point a day for the `days` days up to `today` (`YYYY-MM-DD`), its score `null` on a day none was kept;
 *  `first` and `last` the days with one, `delta` last − first (null with fewer than two). */
function knHealthTrend(history, today, days = 30) {
  const by = new Map((history || []).map((p) => [p.day, p.score]))
  const t = Date.parse(`${today}T00:00:00Z`)
  const points = []
  for (let i = days - 1; i >= 0; i--) {
    const day = new Date(t - i * 864e5).toISOString().slice(0, 10)
    points.push({ day, score: by.has(day) ? by.get(day) : null })
  }
  const kept = points.filter((p) => p.score != null)
  const first = kept[0] || null, last = kept[kept.length - 1] || null
  return { points, first, last, delta: kept.length > 1 ? last.score - first.score : null }
}
/** The trend drawn in a `w`×`h` box: a line and the area under it for each run of days with a score — a day without one
 *  is a gap, never bridged — and a dot for a day alone between gaps; `end` the last point. */
function knTrendPath(points, { w = 300, h = 44, lo = 60, hi = 100 } = {}) {
  const n = points.length
  const low = Math.min(lo, ...points.filter((p) => p.score != null).map((p) => Math.floor(p.score / 10) * 10))
  const x = (i) => +(n > 1 ? 3 + (i * (w - 6)) / (n - 1) : w / 2).toFixed(1)
  const y = (v) => +(h - 4 - ((v - low) / Math.max(1, hi - low)) * (h - 8)).toFixed(1)
  const runs = []
  let cur = null
  points.forEach((p, i) => {
    if (p.score == null) { cur = null; return }
    if (!cur) runs.push((cur = []))
    cur.push([x(i), y(p.score)])
  })
  const lines = runs.filter((r) => r.length > 1).map((r) => r.map(([a, b], j) => `${j ? 'L' : 'M'}${a},${b}`).join(''))
  const area = runs.filter((r) => r.length > 1).map((r, i) => `${lines[i]}L${r[r.length - 1][0]},${h}L${r[0][0]},${h}Z`).join('')
  return { line: lines.join(''), area, dots: runs.filter((r) => r.length === 1).map((r) => r[0]), end: runs.length ? runs[runs.length - 1][runs[runs.length - 1].length - 1] : null }
}
/** *Fix all*'s sheet from `/health?fixes=<kind>`: a row per fix, ticked unless its key is in `off` — never a guess (it is
 *  fixed on its own finding, ruling R36); one Echo couldn't compute says why, unticked. `items` is what `kn_fix_all`
 *  takes: each ticked fix by its key, with the `base` and `sha` of the diff shown (ruling R37); `files` how many files. */
function knFixAll(fixes, findings, off = new Set()) {
  const by = new Map((findings || []).map((f) => [f.key, f]))
  const rows = (fixes || []).filter((x) => x && !x.guess).map((x) => {
    const f = by.get(x.key) || {}, page = (f.pages || [])[0] || []
    const ok = !x.error && !!x.base && !!x.sha
    return { key: x.key, choice: x.choice || '', base: x.base, sha: x.sha, path: x.path || page[0] || '', line: page[1] ?? null, title: f.title || '', hunks: x.hunks || [], plus: x.plus || 0, minus: x.minus || 0, error: x.error || (ok ? '' : 'Echo can\'t show this fix'), on: ok && !off.has(x.key) }
  })
  const on = rows.filter((r) => r.on)
  return { rows, items: on.map((r) => ({ key: r.key, choice: r.choice, base: r.base, sha: r.sha })), files: new Set(on.map((r) => r.path)).size }
}
/** What came of `kn_fix` / `kn_fix_all` (`{written, changed_since, skipped, commit}`) in Echo's words — `text`, and
 *  `wait` when something wasn't done (said until OK; else it goes by itself). `findings` name the skipped ones. A
 *  commit takes only files without changes of the user's (X-G2), and the answer doesn't name them: said so for several. */
function knFixSummary(r, findings = []) {
  const title = (k) => ((findings || []).find((f) => f.key === k) || {}).title || 'a finding'
  const written = r.written || [], commit = String(r.commit || ''), skipped = /^skipped: /.test(commit)
  const committed = commit && commit !== 'off' && !skipped ? ` Committed ${written.length > 1 ? 'those without changes of yours ' : ''}(${commit}).` : ''
  const out = []
  if (written.length) out.push(`${written.length === 1 ? `Fixed: ${written[0]}` : `Fixed in ${written.length} files`}.${skipped ? ` Not committed: ${commit.slice(9)}.` : committed}`)
  for (const p of r.changed_since || []) out.push(`${p} changed since Echo showed its fix, so it was left as it is.`)
  for (const s of r.skipped || []) out.push(`Not fixed: ${title(s.key)} — ${String(s.why).replace(/\.$/, '')}.`)
  return { text: out.join(' ') || 'Nothing was changed.', wait: !written.length || skipped || out.length > 1 }
}
// end of the kn-health block

/** Knowledge's views and state (`S.kn`). */
const Kn = (() => {
  const st = () => {
    if (!S.kn) S.kn = { data: null, vault: '', tab: 'pages', path: '', tree: null, page: null, filter: 'all', q: '', hits: null, open: new Set(), rail: 'links', hist: null, ver: null, edit: null, graph: null, gopt: { hideIndex: true, isolated: true, tags: false, color: 'folder', depth: 0, sel: null, hidden: [] }, gq: '', settings: null, sheet: null, switcher: false, menu: null, busy: {}, error: null, chip: null, lastVault: null, ovr: {}, chat: null, askConv: '', hl: null, hlKey: '' }
    return S.kn
  }
  const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms))
  const I = (d, w = 14) => `<svg viewBox="0 0 24 24" width="${w}" height="${w}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`
  const IC = {
    gem: I('<path d="M6.5 3.5h11l3.5 5-9 12-9-12z"/><path d="M3 8.5h18M9.5 3.5l-1 5 3.5 12 3.5-12-1-5"/>'),
    folder: I('<path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/>'),
    file: I('<path d="M7 3.5h7l4 4V20a.5.5 0 0 1-.5.5h-10.5a.5.5 0 0 1-.5-.5V4a.5.5 0 0 1 .5-.5z"/><path d="M14 3.5V8h4"/>'),
    index: I('<path d="M5 5h14M5 10h14M5 15h9M5 20h6"/>'),
    plus: I('<path d="M12 5v14M5 12h14"/>'),
    minus: I('<path d="M5 12h14"/>'),
    fit: I('<path d="M4.5 9V4.5H9M15 4.5h4.5V9M19.5 15v4.5H15M9 19.5H4.5V15"/>'),
    more: I('<circle cx="5.5" cy="12" r="1.3"/><circle cx="12" cy="12" r="1.3"/><circle cx="18.5" cy="12" r="1.3"/>', 16),
    ask: I('<path d="M12 3.5l1.8 4.7 4.7 1.8-4.7 1.8L12 16.5l-1.8-4.7L5.5 10l4.7-1.8z"/>'),
    ext: I('<path d="M14 4.5h5.5V10M19.5 4.5L11 13"/><path d="M18 14v4.5a1 1 0 0 1-1 1H5.5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1H10"/>', 13),
    git: I('<circle cx="6" cy="6" r="2.2"/><circle cx="6" cy="18" r="2.2"/><circle cx="18" cy="8.5" r="2.2"/><path d="M6 8.2v7.6M18 10.7c0 4-6 3-10.5 6.2"/>', 12),
    check: I('<path d="M5 12.5l4.5 4.5L19 7.5"/>'),
    warn: I('<path d="M12 4l9 16H3z"/><path d="M12 10v4.5M12 17.5v.1"/>', 16),
    info: I('<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5M12 8v.1"/>', 16),
    link: I('<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>', 12),
    close: I('<path d="M6 6l12 12M18 6L6 18"/>', 16),
    recall: I('<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5M8.5 11h5M11 8.5v5"/>'),
    plug: I('<path d="M9 3.5v4M15 3.5v4M6.5 7.5h11V11a5.5 5.5 0 0 1-11 0z"/><path d="M12 16.5v4"/>'),
    graph: I('<circle cx="6" cy="7" r="2.3"/><circle cx="17.5" cy="6" r="2.3"/><circle cx="12" cy="17" r="2.3"/><path d="M8 8l2.8 7M15.8 7.6L13 15M8.3 7l7-.8"/>'),
    // Ask
    at: I('<circle cx="12" cy="12" r="3.6"/><path d="M15.6 12v1.6a2.6 2.6 0 0 0 5.2 0V12a8.8 8.8 0 1 0-3.4 6.9"/>'),
    clip: I('<path d="M20 11.5l-7.8 7.8a5 5 0 0 1-7.1-7.1l8.2-8.2a3.4 3.4 0 0 1 4.8 4.8l-8.2 8.2a1.7 1.7 0 0 1-2.4-2.4l7.5-7.5"/>'),
    stop: I('<rect x="7" y="7" width="10" height="10" rx="2" fill="currentColor"/>'),
    up: I('<path d="M12 19V5M6 11l6-6 6 6"/>'),
    globe: I('<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.5 2.6 3.6 5.3 3.6 8.5s-1.1 5.9-3.6 8.5c-2.5-2.6-3.6-5.3-3.6-8.5S9.5 6.1 12 3.5z"/>'),
    edit: I('<path d="M4.5 19.5h4l10-10a2.8 2.8 0 0 0-4-4l-10 10z"/><path d="M13.5 6.5l4 4"/>'),
    search: I('<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>'),
    bolt: I('<path d="M13 3.5L5.5 13.5H12l-1 7 7.5-10H12z"/>'),
    files: I('<path d="M8.5 3.5h6l4 4V17a.5.5 0 0 1-.5.5H8.5A.5.5 0 0 1 8 17V4a.5.5 0 0 1 .5-.5z"/><path d="M14.5 3.5V8h4M5 7v13a.5.5 0 0 0 .5.5H14"/>'),
    agent: I('<circle cx="12" cy="8" r="3.5"/><path d="M5 20a7 7 0 0 1 14 0"/>'),
    term: I('<rect x="3.5" y="5" width="17" height="14" rx="2"/><path d="M7.5 10l3 2.5-3 2.5M12.5 15.5h4"/>'),
    copy: I('<rect x="8.5" y="8.5" width="11" height="11" rx="2"/><path d="M15.5 8.5V6A1.5 1.5 0 0 0 14 4.5H6A1.5 1.5 0 0 0 4.5 6v8A1.5 1.5 0 0 0 6 15.5h2.5"/>'),
    clock: I('<circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/>', 12),
    call: I('<path d="M6.5 4.5h3l1.5 4-2 1.5a10 10 0 0 0 5 5l1.5-2 4 1.5v3a1.5 1.5 0 0 1-1.6 1.5A15.5 15.5 0 0 1 5 6.1a1.5 1.5 0 0 1 1.5-1.6z"/>'),
    asks: I('<circle cx="12" cy="12" r="8.5"/><path d="M9.6 9.5a2.5 2.5 0 0 1 4.8 1c0 1.7-2.4 2-2.4 3.5M12 17v.1"/>'),
    no: I('<circle cx="12" cy="12" r="8.5"/><path d="M6 6l12 12"/>'),
    side: I('<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M9.5 4.5v15"/>', 15),
    save: I('<path d="M5 4.5h11l3.5 3.5v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5.5a1 1 0 0 1 1-1z"/><path d="M8 4.5v5h7v-5M8 20v-6h8v6"/>'),
    lock: I('<rect x="5.5" y="10.5" width="13" height="9.5" rx="2"/><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5"/>'),
  }
  const GEM_BIG = I('<path d="M6.5 3.5h11l3.5 5-9 12-9-12z"/><path d="M3 8.5h18M9.5 3.5l-1 5 3.5 12 3.5-12-1-5"/>', 22)
  const vaults = () => ((st().data || {}).vaults || [])
  const vault = () => vaults().find((v) => v.id === st().vault) || null
  const enc = (p) => encodeURIComponent(p)
  const store = () => { try { return window.localStorage } catch { return null } }
  /** A message that waits for OK (something didn't happen, or did only in part). */
  const say = (text) => { S.notice = text; render() }
  /** A done thing, said for a moment. */
  const done = (text) => { S.notice = text; render(); setTimeout(() => { if (S.notice === text) { S.notice = null; render() } }, 4000) }
  const recallOn = () => typeof brainOn === 'function' && brainOn() && !!(S.state && S.state.brain)
  const pluginTabs = () => (typeof knowledgeTabs === 'function' ? knowledgeTabs() : [])
  const many = (n, one) => `${nf(n)} ${one}${n === 1 ? '' : 's'}`
  const kb = (bytes) => bytes < 1024 ? 'under 1 KB' : `${Math.round(bytes / 1024)} KB`
  const short = (p) => String(p || '').replace(/^\/Users\/[^/]+/, '~').replace(/^[A-Z]:\\Users\\[^\\]+/i, '~')

  // ── talking to the app: an action is a job the page polls (works in the window and in `--serve`) ──
  async function run(cmd, args = {}, busyKey) {
    const s = st()
    if (busyKey) { s.busy[busyKey] = true; render() }
    try {
      let last = 0
      try { last = (await get('api/kn/jobs?since=999999999')).last || 0 } catch { /* first */ }
      send({ cmd, ...args })
      for (let i = 0; i < 900; i++) {
        await sleep(i < 20 ? 50 : 200)
        let j
        try { j = await get(`api/kn/jobs?since=${last}`) } catch { continue }
        const mine = (j.jobs || []).find((x) => x.op === cmd)
        if (mine && mine.done) { if (mine.error) throw new Error(mine.error); return mine.value }
      }
      throw new Error("Echo didn't answer")
    } finally { if (busyKey) { delete s.busy[busyKey]; render() } }
  }
  /** The system's folder dialog; the folder picked (with what Echo found in it), or null when cancelled. */
  async function pick(what) {
    let before = 0
    try { before = (await get('api/kn/pick')).seq || 0 } catch { /* none */ }
    send({ cmd: 'kn_pick', for: what })
    let seen = false
    for (let i = 0; i < 1200; i++) {
      await sleep(250)
      let p
      try { p = await get('api/kn/pick') } catch { continue }
      if (p.busy) seen = true
      if (p.seq > before) return p
      if (seen && !p.busy) return null
      if (!seen && i > 20 && !p.busy) return null
    }
    return null
  }

  async function load() {
    const s = st()
    try { s.data = await get('api/kn') } catch { return }
    if (!s.vault && s.lastVault && vaults().some((v) => v.id === s.lastVault)) s.vault = s.lastVault
    if (s.vault && !vault()) s.vault = ''
    if (s.vault) await loadVault()
  }
  async function loadVault() {
    const s = st(), v = vault()
    if (!v) return
    s.lastVault = v.id
    try { localStorage.setItem('echo.kn.vault', v.id) } catch { /* off */ }
    if (v.available === false) return
    // Health's number on its tab (Needs you), wherever the vault opens; the Health tab loads it with its view.
    const h = hl()
    if (s.tab !== 'health' && !h.data && !h.loading) { h.loading = true; loadHealth({ detail: false }).then(() => { h.loading = false; render() }) }
    if (s.tab === 'pages') {
      if (!s.tree || s.tree.vault !== v.id) await loadTree()
      if (!s.path) s.path = (s.tree && s.tree.index) || ''
      if (s.path && (!s.page || s.page.path !== s.path || s.page.vault !== v.id)) await loadPage()
    } else if (s.tab === 'graph') await loadGraph()
    else if (s.tab === 'settings') await loadSettings()
    else if (s.tab === 'ask') { openChat(s.askConv); await loadChats() }
    else if (s.tab === 'health') { if (h.data) await loadFinding(); else await loadHealth() }
  }
  async function loadTree() {
    const s = st(), v = vault()
    try { const t = await get(`api/kn/${enc(v.id)}/tree`); t.vault = v.id; s.tree = t; if (!s.open.size && t.notes) s.open.add(t.notes) } catch (e) { s.error = String(e.message || e) }
  }
  async function loadPage() {
    const s = st(), v = vault()
    if (!v || !s.path) return
    try {
      const p = await get(`api/kn/${enc(v.id)}/page?path=${enc(s.path)}`)
      if (p.error) { s.page = { vault: v.id, path: s.path, error: p.error }; return }
      p.vault = v.id
      s.page = p
      if (s.rail === 'history') await loadHistory()
    } catch (e) { s.page = { vault: v.id, path: s.path, error: String(e.message || e) } }
  }
  async function loadHistory() {
    const s = st(), v = vault()
    if (!v || !s.path) return
    try { s.hist = await get(`api/kn/${enc(v.id)}/history?path=${enc(s.path)}`); s.hist.path = s.path } catch { s.hist = null }
  }
  async function loadGraph() {
    const s = st(), v = vault()
    const mode = s.gopt.tags ? 'both' : 'links'
    // Stamped with the vault's rev, so the poll sees a change on disk even when the Graph was opened without Pages.
    let rev = s.tree && s.tree.vault === v.id ? s.tree.rev : null
    if (!rev) { try { rev = (await get(`api/kn/${enc(v.id)}/rev`)).rev } catch { /* app restarting */ } }
    if (s.graph && s.graph.vault === v.id && s.graph.mode === mode && s.graph.rev === rev) return
    try { const g = await get(`api/kn/${enc(v.id)}/graph?mode=${mode}`); g.vault = v.id; g.mode = mode; g.rev = rev; s.graph = g } catch (e) { s.error = String(e.message || e); return }
    // A very large vault opens on a local graph (drawing every page would freeze the window); Off shows it all.
    if (s.bigGraph !== v.id) {
      const start = knGraphStart(s.graph.nodes.filter((n) => !n.tag).length, s.path, (s.tree || {}).index)
      if (start && start.sel) { s.gopt.depth = start.depth; s.gopt.sel = start.sel; s.bigGraph = v.id }
    }
  }
  async function loadSettings() {
    const s = st(), v = vault()
    try { s.settings = await get(`api/kn/${enc(v.id)}/settings`) } catch (e) { s.error = String(e.message || e) }
  }
  /** Health's findings, score and history (`/health`, checked by code when asked), stamped with the vault's rev so the
   *  poll sees a change on disk; then the finding shown, with its fix. */
  async function loadHealth({ detail = true } = {}) {
    const v = vault(), h = hl()
    let rev = null
    try { rev = (await get(`api/kn/${enc(v.id)}/rev`)).rev } catch { /* the app restarting */ }
    try {
      const d = await get(`api/kn/${enc(v.id)}/health`)
      if (hl() !== h) return
      if (d.error) { h.error = d.error; return }
      Object.assign(h, { data: d, rev, at: Date.now(), error: null })
    } catch (e) { h.error = String(e.message || e); return }
    if (detail) await loadFinding(true)
  }
  /** The finding shown (the route's, else the first of its kind, else the first): its choices and the fix of the one
   *  picked, with the diff (`health/<key>`, the key encoded once). `force`: asked again though it is the one shown. */
  async function loadFinding(force = false) {
    const v = vault(), h = hl()
    const sel = h.data && knHealthSelect(hlList(), st().hlKey)
    if (!sel) { h.detail = null; return }
    const choice = h.choice[sel.key] || '', want = `${sel.key}\n${choice}`
    if (!force && h.detail && h.detail.want === want) return
    h.want = want
    let d
    try { d = await get(`api/kn/${enc(v.id)}/health/${enc(sel.key)}${choice ? `?choice=${enc(choice)}` : ''}`) } catch (e) { d = { error: String(e.message || e) } }
    if (hl() === h && h.want === want) h.detail = { ...d, key: sel.key, want }
  }
  async function refreshAll() { st().tree = null; st().page = null; st().graph = null; await load(); render() }

  /** Polled with the window (every 2 s while Knowledge shows): a change on disk shows within a tick. */
  let ticks = 0
  async function tick() {
    const s = st(), v = vault()
    ticks++
    if (ticks % 15 === 0) { try { s.data = await get('api/kn') } catch { /* app restarting */ } }
    if (!v || v.available === false || S.view !== 'brain' || !s.vault) return
    let rev
    try { rev = (await get(`api/kn/${enc(v.id)}/rev`)).rev } catch { return }
    const h = s.hl && s.hl.vault === v.id && s.hl.data ? s.hl : null
    const known = s.tree ? s.tree.rev : s.graph ? s.graph.rev : h ? h.rev : null
    if (!known || rev === known) return
    await loadTree()
    // Health's findings, its number on the tab and the diff shown follow the change.
    if (h) await loadHealth({ detail: s.tab === 'health' })
    if (s.edit) {
      // Editing: the file changed under the editor? Say so; never replace what is typed.
      try { const p = await get(`api/kn/${enc(v.id)}/page?path=${enc(s.edit.path)}`); if (p.base && p.base !== s.edit.base) s.edit.conflict = { base: p.base, text: p.text, mtime: p.mtime } } catch { /* gone */ }
    } else if (s.path) await loadPage()
    if (s.tab === 'graph') { s.graph = null; await loadGraph() }
    render()
  }

  // ── routing ──
  function routeTo(r) {
    const s = st()
    if (r.vault !== s.vault) { s.page = null; s.tree = null; s.graph = null; s.settings = null; s.edit = null; s.hits = null; s.q = ''; s.gq = ''; s.gopt.hidden = [] }
    s.vault = r.vault || s.lastVault || (vaults()[0] || {}).id || ''
    s.tab = r.tab
    // Ask's part of the route is a conversation and Health's a finding's key, not the page Pages keeps open.
    if (r.tab === 'ask') { s.askConv = r.path || ''; openChat(s.askConv) } else if (r.tab === 'health') s.hlKey = r.path || ''
    else if (r.path !== undefined && r.path !== s.path) { s.path = r.path; s.ver = null }
    s.source = 'vault'
  }
  /** The route's last part for the tab open now. */
  const here = () => (st().tab === 'ask' ? st().askConv || '' : st().tab === 'health' ? st().hlKey || '' : st().path || '')
  const goVault = (id, tab = 'pages', path = '') => go(knRouteOf({ vault: id, tab, path }))
  /** Knowledge's own view (a vault, or none yet) rather than Recall's or a plugin's tab. */
  function active() {
    const s = st()
    if (s.source === 'vault') return true
    if (s.source === 'recall' && recallOn()) return false
    if (s.source === 'plugin' && pluginTabs().length) return false
    return true
  }
  /** Opening the Knowledge activity: where the user left it. */
  function home() {
    const s = st()
    if (s.source === 'recall' && recallOn()) return `brain/${(S.br && S.br.tab) || 'sources'}`
    const id = s.vault || s.lastVault || (() => { try { return localStorage.getItem('echo.kn.vault') } catch { return null } })() || ((s.data && s.data.vaults[0]) || {}).id
    return id ? knRouteOf({ vault: id, tab: s.tab || 'pages', path: here() }) : 'kn'
  }

  // ── the header: Knowledge, the switcher, the vault's place, actions ──
  function switcherMenu() {
    const e = knSwitcherEntries(vaults(), recallOn(), pluginTabs())
    const s = st()
    const item = (x) => {
      switch (x.kind) {
        case 'head': return `<div class="kn-menu-h">${esc(x.label)}</div>`
        case 'sep': return '<div class="menu-sep" role="separator"></div>'
        case 'vault': return `<button class="menu-item kn-mi${x.id === s.vault && s.source === 'vault' ? ' on' : ''}" role="menuitem" data-kn="vault" data-id="${esc(x.id)}"><span class="kn-gem sm">${IC.gem}</span><span class="grow">${esc(x.name)}</span><span class="kn-mi-r">${x.available ? `${x.pages == null ? '' : `${nf(x.pages)} pages`}${x.star ? ' · ★ minutes' : ''}` : 'folder not found'}</span></button>`
        case 'connect': return `<button class="menu-item kn-mi" role="menuitem" data-kn="connect">${IC.folder}<span class="grow">${esc(x.label)}</span></button>`
        case 'new': return `<button class="menu-item kn-mi" role="menuitem" data-kn="new">${IC.plus}<span class="grow">${esc(x.label)}</span></button>`
        case 'recall': return `<button class="menu-item kn-mi${s.source === 'recall' ? ' on' : ''}" role="menuitem" data-kn="recall">${IC.recall}<span class="grow">Recall</span><span class="kn-mi-r">what was said</span></button>`
        case 'plugin': return `<button class="menu-item kn-mi" role="menuitem" data-kn="plugin" data-kn-tab="${esc(x.tab)}">${IC.plug}<span class="grow">${esc(x.label)}</span><span class="kn-mi-r">${esc(x.plugin)}</span></button>`
      }
      return ''
    }
    return `<div class="menu kn-switch-menu" role="menu">${e.map(item).join('')}<div class="kn-menu-foot">Each vault is a folder of Markdown you own; Echo changes nothing in it until you save something.</div></div>`
  }
  /** The title and switcher — also drawn in Recall's and plugins' Knowledge headers. */
  function title() {
    const s = st(), v = vault()
    const label = s.source === 'recall' ? 'Recall' : s.source === 'plugin' ? ((pluginTabs().find((t) => t.tab === (S.br || {}).tab) || {}).label || 'Plugin') : v ? v.name : vaults().length ? 'Choose a vault' : 'No vault yet'
    const icon = s.source === 'recall' ? IC.recall : s.source === 'plugin' ? IC.plug : IC.gem
    return `<h2 class="kn-h2">Knowledge</h2><span class="kn-switch-wrap"><button class="kn-switch" data-kn="switcher" aria-haspopup="menu" aria-expanded="${!!s.switcher}"><span class="kn-gem">${icon}</span><span class="kn-switch-name">${esc(label)}</span><span class="kn-car" aria-hidden="true">▾</span></button>${s.switcher ? switcherMenu() : ''}</span>`
  }
  function head() {
    const s = st(), v = vault()
    if (!v) return `<header class="kn-head">${title()}<span class="grow"></span></header>`
    const g = v.git
    const where = v.available === false ? '<span class="kn-meta warn">The folder isn\'t there</span>'
      : `<span class="kn-meta"><code title="${esc(v.root)}">${esc(short(v.root))}</code>${g ? ` · ${IC.git} ${esc(g.branch || 'detached')}` : ''}${v.pages != null ? ` · ${nf(v.pages)} pages` : ''}</span>`
    const menu = s.menu === 'vault' ? `<div class="menu kn-head-menu" role="menu"><button class="menu-item" role="menuitem" data-kn="reveal" data-path="">${/Windows/.test(navigator.userAgent) ? 'Show in Explorer' : 'Show in Finder'}</button><button class="menu-item" role="menuitem" data-kn="obsidian" data-path="${esc((s.tree || {}).index || '')}">Open in Obsidian</button><button class="menu-item" role="menuitem" data-kn="reread">Read the folder again</button><button class="menu-item" role="menuitem" data-kn="ask-panel">Open Ask in the terminal panel</button><div class="menu-sep" role="separator"></div><button class="menu-item" role="menuitem" data-kn="tab" data-kn-tab="settings">Vault settings</button></div>` : ''
    const acts = v.available === false ? '' : `<button class="btn sm" data-kn="ask" title="A new conversation with Claude Code in this vault">${IC.ask} Ask</button><button class="btn sm" data-kn="newpage">${IC.plus} New page</button><span class="kn-rel"><button class="icon-btn" data-kn="menu" data-menu="vault" aria-haspopup="menu" aria-expanded="${s.menu === 'vault'}" aria-label="More">${IC.more}</button>${menu}</span>`
    return `<header class="kn-head">${title()}${where}<span class="grow"></span>${acts}</header>`
  }
  function tabs() {
    const s = st()
    const h = s.hl && s.hl.vault === s.vault && s.hl.data
    const n = knTabCounts({ needs: h ? knHealthList(h.findings).needs : 0 })
    const t = (id, label, count) => `<button class="tab" role="tab" data-kn="tab" data-kn-tab="${id}" aria-selected="${s.tab === id}">${label}${count ? `<span class="kn-cnt" title="${count} ${count === 1 ? 'needs' : 'need'} you">${nf(count)}</span>` : ''}</button>`
    return `<div class="kn-tabs"><div class="tabs" role="tablist">${t('pages', 'Pages')}${t('graph', 'Graph')}${t('ask', 'Ask')}${t('health', 'Health', n.health)}${t('settings', 'Settings')}</div></div>`
  }

  // ── nothing connected yet ──
  function emptyView() {
    const recall = recallOn() ? '<p class="kn-empty-foot">Recall — Echo\'s index of what was said in your calls, mail and folders — is in the switcher above.</p>' : ''
    return `<div class="kn-empty"><span class="kn-gem big">${GEM_BIG}</span><h2>Your knowledge, in files you own</h2>
      <p class="kn-empty-lead">A vault is a folder of Markdown — your second brain. Echo shows it, lets you edit and graph it, saves your meeting minutes into it, and Claude answers from it.</p>
      <div class="kn-choices">
        <div class="kn-choice"><h3>${IC.folder} Connect a folder you have</h3><p>An Obsidian vault, a git repository of notes, or any folder of Markdown. Echo maps its index, notes, skills and tags — and changes nothing.</p><button class="btn primary" data-kn="connect">Connect a folder…</button></div>
        <div class="kn-choice"><h3>${IC.plus} Start a new one</h3><p>Echo's twin starter: the folders, an index, tags and terms, a profile to fill, and the <code>/twin</code> and <code>/process-inbox</code> skills — in git.</p><button class="btn" data-kn="new">Start a new vault…</button></div>
      </div>${recall}${(st().data || {}).plugin_vaults ? `<p class="kn-empty-foot">The Obsidian vaults plugin has ${st().data.plugin_vaults} vault${st().data.plugin_vaults === 1 ? '' : 's'} Echo can connect as they are. <button class="btn ghost sm" data-kn="import">Connect them</button></p>` : ''}</div>`
  }
  function missingView(v) {
    return `<div class="kn-empty"><span class="kn-gem big warn">${GEM_BIG}</span><h2>${esc(v.name)}'s folder isn't there</h2><p class="kn-empty-lead"><code>${esc(v.root)}</code> can't be found — a disk that isn't connected, or a folder moved. Nothing is saved into it until it's back.</p><div class="kn-acts"><button class="btn primary" data-kn="reread">Look again</button><button class="btn ghost" data-kn="remove-ask">Remove from Echo…</button></div></div>`
  }

  // ── Pages ──
  function treeView() {
    const s = st(), t = s.tree
    if (!t) return '<div class="kn-tree"><div class="skeleton" style="padding:14px"><i></i><i></i><i></i></div></div>'
    const f = (id, label) => `<button class="${s.filter === id ? 'on' : ''}" data-kn="filter" data-filter="${id}">${label}</button>`
    let list
    if (s.q.trim() && s.hits) {
      list = s.hits.hits.length ? s.hits.hits.map((h) => `<button class="kn-tr${h.path === s.path ? ' on' : ''}" data-kn="open" data-path="${esc(h.path)}" title="${esc(h.path)}">${h.hit === 'tag' ? IC.link : IC.file}<span class="kn-tr-t"><span class="nm">${esc(h.title)}</span>${h.line ? `<span class="kn-tr-sub">${esc(h.line)}</span>` : `<span class="kn-tr-sub">${esc(h.path.replace(/^[^/]*\//, ''))}</span>`}</span></button>`).join('')
        : `<p class="kn-tree-none">Nothing matches “${esc(s.q)}”.</p>`
    } else {
      const rows = knBuildTree(t.pages, { root: t.root || '', filter: s.filter, open: s.open, index: t.index || '' })
      list = rows.map((r) => {
        const pad = `style="padding-left:${6 + r.depth * 14}px"`
        if (r.kind === 'folder') return `<button class="kn-tr" ${pad} data-kn="fold" data-path="${esc(r.path)}" aria-expanded="${r.open}"><span class="kn-car">${r.open ? '▾' : '▸'}</span>${IC.folder}<span class="nm">${esc(r.name)}</span><span class="kn-tr-r">${r.count}</span></button>`
        const sub = r.sub !== undefined ? `<span class="kn-tr-sub">${esc(r.sub || '')}</span>` : ''
        return `<button class="kn-tr${r.path === s.path ? ' on' : ''}" ${pad} data-kn="open" data-path="${esc(r.path)}" title="${esc(r.title)}"><span class="kn-car"></span>${r.index ? IC.index : IC.file}<span class="kn-tr-t"><span class="nm">${esc(r.name)}</span>${sub}</span>${r.index ? '<span class="kn-tr-r">index</span>' : ''}</button>`
      }).join('') || `<p class="kn-tree-none">${s.filter === 'unindexed' ? 'Every page is in the index.' : 'No pages yet.'}</p>`
    }
    return `<div class="kn-tree"><div class="kn-tree-top"><input class="search" id="kn-find" placeholder="Find a page — title, path, #tag, words" aria-label="Find a page" value="${esc(s.q)}" autocomplete="off" spellcheck="false">
      <div class="kn-filters">${f('all', 'All')}${f('recent', 'Recent')}${f('unindexed', 'Not in index')}</div></div>
      <div class="kn-tree-list" data-keep-scroll>${list}</div>
      <div class="kn-tree-foot">${nf(t.pages.length)} pages · sources and templates hidden</div></div>`
  }
  function propsView(p) {
    // Echo's own bookkeeping (which call a note came from) stays in the file, not in the reader.
    const echoKey = ([k, v]) => k === 'echo_call' || (k === 'origin' && v && v.v === 'echo')
    const rows = (p.props || []).filter((r) => r[0] && !['title', 'tags'].includes(r[0]) && !echoKey(r))
    // Tags: the page's own (its `tags` property and its inline #tags), as chips.
    if ((p.tags || []).length) rows.unshift(['tags', { t: 'list', v: p.tags }])
    if (!rows.length) return ''
    const val = ([k, v]) => {
      if (k === 'tags') return `<span class="kn-tags">${(v.v || []).concat(v.t === 'text' && v.v ? [v.v] : []).filter((x) => typeof x === 'string').map((t) => `<span class="tag kn-tag">${esc(String(t).replace(/^#/, ''))}</span>`).join('')}</span>`
      const text = v.t === 'list' ? (v.v || []).join(', ') : String(v.v == null ? '' : v.v)
      const link = /^\[\[(.+?)(\|(.+?))?\]\]$/.exec(text.trim())
      if (link) return `<a class="kn-wl" href="#kn-name:${esc(link[1])}" data-kn-name="${esc(link[1])}">${esc(link[3] || link[1])}</a>`
      if (/^echo:\/\//.test(text)) return `<a href="${esc(text)}" class="kn-src">${IC.link} ${esc(text)}</a>`
      return `<span>${esc(text)}</span>`
    }
    return `<div class="kn-props">${rows.slice(0, 14).map(([k, v]) => `<span class="kn-pk">${esc(k)}</span>${val([k, v])}`).join('')}</div>`
  }
  function welcomeCard() {
    const s = st(), v = vault()
    if (!v || v.template !== 'twin@1' || !s.page || !s.page.is_index) return ''
    try { if (localStorage.getItem(`echo.kn.welcome:${v.id}`)) return '' } catch { /* off */ }
    const profile = 'knowledge/profile/DIGITAL_PROFILE.md'
    const notes = `<code>${esc(String((v.paths || {}).notes || 'notes').split('/').pop())}/</code>`
    const mode = (v.minutes || {}).mode
    const meetings = mode === 'auto' ? `<b>Meetings come in by themselves.</b> <span class="set-sub">Minutes of your calls land in ${notes} and the index.</span>`
      : mode === 'ask' ? `<b>Keep your meetings.</b> <span class="set-sub">After a call, its page offers to save the minutes into ${notes} and the index.</span>`
        : `<b>Meetings stay out.</b> <span class="set-sub">Minutes aren't saved here by themselves; save one from its call's page when you want it.</span>`
    return `<div class="kn-welcome"><div class="kn-welcome-h"><b>Your vault is ready</b><button class="btn ghost sm" data-kn="welcome-done">Dismiss</button></div><div class="kn-steps">
      <div class="kn-step"><span class="n">1</span><span><b>Tell it who you are.</b> <span class="set-sub">Fill in your profile — everything Claude answers as you starts there.</span></span><button class="btn sm" data-kn="open" data-path="${profile}">Open profile</button></div>
      <div class="kn-step"><span class="n">2</span><span><b>Add what you know.</b> <span class="set-sub">Drop files into <code>raw_inbox/</code>, then ask Claude <code>/process-inbox</code>.</span></span><button class="btn sm" data-kn="reveal" data-path="raw_inbox">Show folder</button></div>
      <div class="kn-step"><span class="n">3</span><span><b>Ask it.</b> <span class="set-sub"><code>/twin what did we decide about…</code> — answers name the pages they used.</span></span><button class="btn sm" data-kn="ask">Ask</button></div>
      <div class="kn-step"><span class="n">4</span><span>${meetings}</span><button class="btn ghost sm" data-kn="tab" data-kn-tab="settings">Change</button></div></div></div>`
  }
  function readerView() {
    const s = st(), v = vault(), p = s.page
    if (s.edit) return editorView()
    if (s.ver) return versionView()
    if (!s.path) return `<div class="kn-reader"><div class="kn-none"><p>Pick a page on the left${(s.tree || {}).index ? '' : ' — this vault has no index page'}.</p></div></div>`
    if (!p || p.path !== s.path) return '<div class="kn-reader"><div class="skeleton" style="padding:24px"><i></i><i></i><i></i><i></i></div></div>'
    if (p.error) return `<div class="kn-reader"><div class="kn-none"><p class="error">${esc(p.error)}</p></div></div>`
    const parts = p.path.split('/'), file = parts.pop()
    const crumbs = `<span class="kn-crumbs" title="${esc(p.path)}">${parts.length ? `<b>${esc(parts.slice(-1)[0])}</b> / ` : ''}${esc(file)}${p.is_index ? ' · the index' : ''}</span>`
    const menu = s.menu === 'page' ? `<div class="menu kn-page-menu" role="menu"><button class="menu-item" role="menuitem" data-kn="reveal" data-path="${esc(p.path)}">${/Windows/.test(navigator.userAgent) ? 'Show in Explorer' : 'Show in Finder'}</button><button class="menu-item" role="menuitem" data-kn="copy" data-text="${esc(p.abs || p.path)}">Copy path</button><button class="menu-item" role="menuitem" data-kn="copy" data-text="${esc(`echo://page/${encodeURIComponent(p.vault || st().vault)}/${String(p.path).split('/').map(encodeURIComponent).join('/')}`)}">Copy echo:// link</button><button class="menu-item" role="menuitem" data-kn="ask-about" data-path="${esc(p.path)}">Ask about it</button><button class="menu-item" role="menuitem" data-kn="rail" data-rail="history">History</button></div>` : ''
    const editBtn = p.editable ? '<button data-kn="edit">Edit</button>' : '<button disabled title="Too large to edit in Echo">Edit</button>'
    const bar = `<div class="kn-bar">${crumbs}<div class="seg" role="group" aria-label="Read or edit"><button aria-pressed="true">Read</button>${editBtn}</div><button class="btn ghost sm" data-kn="obsidian" data-path="${esc(p.path)}" title="Open this page in Obsidian">${IC.ext} Obsidian</button><span class="kn-rel"><button class="icon-btn" data-kn="menu" data-menu="page" aria-haspopup="menu" aria-expanded="${s.menu === 'page'}" aria-label="Page actions">${IC.more}</button>${menu}</span></div>`
    return `<div class="kn-reader" data-keep-scroll>${bar}<article class="kn-doc selectable">${welcomeCard()}${propsView(p)}${p.html}</article></div>`
  }
  function editorView() {
    const s = st(), e = s.edit
    const conflict = e.conflict ? `<div class="kn-warn" role="alert">${IC.warn}<span class="grow"><b>This page changed on disk while you were editing</b>${e.conflict.mtime ? ` (${esc(hm(new Date(e.conflict.mtime)))})` : ''}. Your text isn't saved yet.</span><button class="btn sm" data-kn="compare">${e.compare ? 'Hide theirs' : 'Compare'}</button><button class="btn ghost sm" data-kn="take-theirs">Take theirs</button><button class="btn ghost sm" data-kn="keep-mine" title="Save your text over the change">Keep mine</button></div>` : ''
    const v = vault()
    const note = v && v.commit ? `Save writes the file and commits <b>only this file</b>${v.git && v.git.branch ? ` on <code>${esc(v.git.branch)}</code>` : ''}.` : 'Save writes the file. Commits are off for this vault — you commit.'
    const d = e.compare && e.conflict ? knLineDiff(e.text, e.conflict.text || '') : null
    const right = !(e.compare && e.conflict) ? `<div class="kn-preview kn-doc">${e.preview || ''}</div>`
      : d ? `<div class="kn-theirs kn-diff" role="region" aria-label="The file on disk compared with your text"><div class="hd">− only in your text · + only on disk</div>${d.map(([o, l]) => `<div class="${o === '+' ? 'add' : o === '-' ? 'del' : o === '…' ? 'hd' : ''}">${o === '=' || o === '…' ? ' ' : o} ${esc(l)}</div>`).join('')}</div>`
        : `<textarea class="kn-theirs" readonly aria-label="The file on disk">${esc(e.conflict.text || '')}</textarea>`
    return `<div class="kn-reader kn-editing"><div class="kn-bar"><span class="kn-crumbs">Editing <b>${esc(e.path)}</b>${e.dirty ? ' · not saved' : ''}</span><div class="seg" role="group" aria-label="Read or edit"><button data-kn="edit-cancel">Read</button><button aria-pressed="true">Edit</button></div><button class="btn ghost sm" data-kn="edit-cancel">Cancel</button><button class="btn primary sm" data-kn="save"${e.conflict || st().busy.save ? ' disabled' : ''}>${st().busy.save ? 'Saving…' : 'Save'}</button></div>
      ${conflict}<div class="kn-editor"><textarea id="kn-edit" spellcheck="true" aria-label="The page's Markdown">${esc(e.text)}</textarea>${right}</div>
      <div class="kn-edit-foot">${v && v.commit ? `<label class="kn-msg">Message <input class="search" id="kn-msg" value="${esc(e.message || `Edit ${e.path}`)}" aria-label="Commit message"></label>` : ''}<span class="set-sub">${note}</span></div></div>`
  }
  function versionView() {
    const s = st(), r = s.ver
    if (!r || r.loading) return '<div class="kn-reader"><div class="skeleton" style="padding:24px"><i></i><i></i><i></i></div></div>'
    const lines = String(r.diff || '').split('\n').filter((l) => !/^(diff --git|index |--- |\+\+\+ )/.test(l))
    const diff = lines.map((l) => `<div class="${l.startsWith('@@') ? 'hd' : l.startsWith('+') ? 'add' : l.startsWith('-') ? 'del' : ''}">${esc(l) || ' '}</div>`).join('')
    const v = (s.hist && s.hist.versions || []).find((x) => x.rev === r.rev) || {}
    return `<div class="kn-reader"><div class="kn-bar"><span class="kn-crumbs"><b>${esc(s.path.split('/').slice(-2, -1)[0] || '')}</b> / ${esc(s.path.split('/').pop())} · version of ${esc(v.at ? new Date(v.at).toLocaleString([], { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : r.rev)}</span><button class="btn sm" data-kn="restore" data-rev="${esc(r.rev)}"${st().busy.restore ? ' disabled' : ''}>Restore this version</button><button class="btn ghost sm" data-kn="ver-close">Back to the page</button></div>
      <div class="kn-doc"><p class="set-sub">${esc(v.message || '')} — ${esc(v.author || '')} · <code>${esc(r.rev)}</code></p><div class="kn-diff">${diff || '<div>No change in this file.</div>'}</div>
      <p class="set-sub">Restoring writes this version's text${vault() && vault().commit ? ' and commits it — nothing is lost' : '; commit it when you are ready'}.</p></div></div>`
  }
  function railView() {
    const s = st(), p = s.page
    const t = (id, label) => `<button class="tab" role="tab" data-kn="rail" data-rail="${id}" aria-selected="${s.rail === id}">${label}</button>`
    let body = ''
    if (!p || p.error || p.path !== s.path) body = ''
    else if (s.rail === 'links') {
      const back = (p.backlinks || []).map((b) => `<button class="kn-bl" data-kn="open" data-path="${esc(b.path)}"><b>${IC.file} ${esc(b.title || b.path.split('/').pop())}</b><span>${esc(b.line || b.path)}</span></button>`).join('') || '<p class="set-sub">Nothing links here yet.</p>'
      const out = (p.links || []).map((l) => l.path ? `<button class="kn-bl" data-kn="open" data-path="${esc(l.path)}"><b>${IC.file} ${esc(l.title || l.path.split('/').pop())}</b></button>` : `<button class="kn-bl missing" data-kn="newpage" data-name="${esc(l.target)}"><b>${IC.file} ${esc(l.target)}</b><span>no such page — make it</span></button>`).join('') || '<p class="set-sub">No links.</p>'
      const idx = p.in_index === false ? `<div class="kn-rail-note">${IC.info}<span>Not in the index — Claude finds pages through it. <button class="btn ghost sm" data-kn="rows" data-path="${esc(p.path)}">Add a row…</button></span></div>` : ''
      body = `${idx}<div><h4>Linked from <span>${(p.backlinks || []).length}</span></h4>${back}</div><div><h4>Links from this page <span>${(p.links || []).length}</span></h4>${out}</div>${(p.local && p.local.nodes.length > 1) ? '<div><h4>Local graph</h4><div class="kn-mini" id="kn-mini"></div></div>' : ''}`
    } else if (s.rail === 'outline') {
      body = (p.outline || []).map(([l, h]) => `<div class="kn-ol l${l}">${esc(h)}</div>`).join('') || '<p class="set-sub">No headings.</p>'
    } else {
      const h = s.hist
      if (!h || h.path !== s.path) body = '<div class="skeleton"><i></i><i></i></div>'
      else if (h.no_history) body = '<p class="set-sub">This vault isn\'t in git, so pages have no versions here.</p>'
      else body = `<div><h4>Versions <span>git</span></h4>${h.versions.map((x, i) => `<button class="kn-hist${s.ver && s.ver.rev === x.rev ? ' on' : ''}" data-kn="version" data-rev="${esc(x.rev)}"><i></i><span><b>${esc(x.message)}</b><span>${esc(x.author)} · ${esc(new Date(x.at).toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' }))} · +${x.added} −${x.removed}${i === 0 ? ' · current' : ''}</span></span></button>`).join('')}</div><p class="set-sub">Each version is a commit. Restoring one makes a new commit — nothing is lost.</p>`
    }
    return `<aside class="kn-rail"><div class="kn-rail-tabs tabs" role="tablist">${t('links', 'Links')}${t('outline', 'Outline')}${t('history', 'History')}</div><div class="kn-rail-body" data-keep-scroll>${body}</div></aside>`
  }
  function pagesView() {
    const narrow = window.innerWidth < 1100
    return `<div class="kn-pages${narrow ? ' no-rail' : ''}">${treeView()}${readerView()}${narrow ? '' : railView()}</div>`
  }

  // ── Graph ──
  const FOLDER_COLORS = ['#9b78ff', '#6cc4f0', '#f0a24a', '#3ccf91', '#f48fb8', '#c6d36a', '#d9b38c', '#b39cff', '#ff9f80', '#7fd1c7']
  const GREY = '#8a8a95'
  /** A live graph on a canvas, laid out by the kn-force block: it moves until it settles, then draws nothing until
   *  something changes. Drag a page and its links pull its neighbours along; drag the background to pan, scroll to zoom.
   *  A click calls `click(id | null)`, a double click `open(id)`. Kept across renders: `mount(box)` puts it back. */
  function graphCanvas(opts) {
    const o = { labels: 'auto', maxFit: 2.2, font: 11, long: 80, ...opts }
    const canvas = document.createElement('canvas'), ctx = canvas.getContext('2d'), root = document.documentElement
    canvas.className = 'kn-canvas'
    canvas.setAttribute('role', 'img')
    const sim = knForceSim(o.forces)
    const calm = matchMedia('(prefers-reduced-motion: reduce)')
    let W = 1, H = 1, dpr = 1, k = 1, tx = 0, ty = 0, fitting = true, raf = 0, dirty = true, pads = { l: 16, r: 16, t: 16, b: 16 }
    let hover = null, sel = null, drag = null, pan = null, down = null, hits = null, lastClick = null, fly = null, C = {}
    const widths = new Map()
    const css = (v) => getComputedStyle(root).getPropertyValue(v).trim()
    const SX = (n) => n.x * k + tx, SY = (n) => n.y * k + ty
    const zr = () => Math.pow(k, 0.7)
    const rad = (n) => Math.max(2, n.r * zr())
    function theme() {
      C = { text: css('--text'), text2: css('--text-2'), text3: css('--text-3'), accent: css('--accent'), halo: css('--bg'), font: getComputedStyle(document.body).fontFamily || 'system-ui, sans-serif' }
      widths.clear(); dirty = true; kick()
    }
    function kick() { if (!raf) raf = requestAnimationFrame(frame) }
    function frame() {
      raf = 0
      if (!canvas.isConnected) return // off the page: it carries on where it was once mounted again
      const live = !sim.settled()
      if (live) { sim.tick(); dirty = true; if (fitting) fit() }
      if (fly) flyStep()
      if (dirty) { draw(); dirty = false }
      if (live || fly) kick()
    }
    function draw() {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      ctx.clearRect(0, 0, W, H)
      const N = sim.nodes(), L = sim.links()
      const f = drag || hover, near = f ? new Set([f, ...f.nb]) : null
      const lit = (l) => f ? l.s === f || l.t === f : !!sel && (l.s === sel || l.t === sel)
      // Links: the plain ones, then the lit ones (the hovered page's, else the selected page's) over them. One stroke
      // each: a path of thousands of segments strokes several times slower than the segments one by one.
      ctx.lineWidth = Math.max(0.6, Math.min(1.4, k * 0.9))
      for (const on of [false, true]) {
        ctx.strokeStyle = on ? C.accent : C.text3
        ctx.globalAlpha = on ? 0.85 : f ? 0.06 : hits ? 0.1 : 0.3
        for (const l of L) {
          if (lit(l) !== on) continue
          const x1 = SX(l.s), y1 = SY(l.s), x2 = SX(l.t), y2 = SY(l.t)
          if ((x1 < 0 && x2 < 0) || (y1 < 0 && y2 < 0) || (x1 > W && x2 > W) || (y1 > H && y2 > H)) continue
          ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke()
        }
      }
      // Pages, grouped by colour; what isn't near the hovered page (or isn't found) is dimmed.
      const groups = new Map()
      for (const n of N) {
        const dim = near ? !near.has(n) : hits ? !hits.has(n.id) && n !== sel : false
        const key = `${dim ? 0 : 1}${n.color || C.text3}`
        if (!groups.has(key)) groups.set(key, [])
        groups.get(key).push(n)
      }
      for (const [key, list] of groups) {
        ctx.globalAlpha = key[0] === '0' ? 0.14 : 1
        ctx.fillStyle = key.slice(1)
        for (const n of list) {
          const x = SX(n), y = SY(n), r = rad(n)
          if (x < -r || y < -r || x > W + r || y > H + r) continue
          ctx.beginPath()
          if (n.shape === 'tag') { ctx.moveTo(x, y - r - 1); ctx.lineTo(x + r + 1, y); ctx.lineTo(x, y + r + 1); ctx.lineTo(x - r - 1, y); ctx.closePath() } else ctx.arc(x, y, r, 0, Math.PI * 2)
          ctx.fill()
        }
      }
      // Rings: the selected page, and what a search found.
      ctx.globalAlpha = 1
      ctx.strokeStyle = C.accent
      for (const n of N) {
        if (n !== sel && !(hits && hits.has(n.id))) continue
        ctx.lineWidth = n === sel ? 2.2 : 1.5
        ctx.beginPath(); ctx.arc(SX(n), SY(n), rad(n) + 3.5, 0, Math.PI * 2); ctx.stroke()
      }
      labels(N, near)
      ctx.globalAlpha = 1
    }
    // Labels: the hovered, dragged or selected page's always (its whole title); its neighbours and search matches next;
    // then the biggest pages, and every page once zoomed in (or in a small graph) — none drawn over another.
    function labels(N, near) {
      const z = zr(), fs = o.font, cands = [], all = N.length <= 80
      ctx.font = `${fs}px ${C.font}`
      for (const n of N) {
        const p = n === hover || n === drag || n === sel || n.pin ? 3 : (hits && hits.has(n.id)) || (near && near.has(n)) ? 2 : 1
        if (p === 1 && (o.labels !== 'auto' || near || hits || (!all && n.r * z < 6 && k < 1.9))) continue
        const x = SX(n), y = SY(n) + rad(n) + 3
        if (x < -300 || x > W + 300 || y < -20 || y > H + 20) continue
        const text = p === 3 ? knShortLabel(n.title || n.label, o.long) : n.label
        let w = widths.get(text)
        if (w == null) { w = ctx.measureText(text).width; widths.set(text, w) }
        cands.push({ text, x, y, w, h: fs, p, r: n.r })
      }
      ctx.textAlign = 'center'; ctx.textBaseline = 'top'; ctx.lineJoin = 'round'
      for (const c of knLabelPlan(cands, W, H)) {
        ctx.lineWidth = 3; ctx.strokeStyle = C.halo; ctx.strokeText(c.text, c.x, c.y)
        ctx.fillStyle = c.p > 1 ? C.text : C.text2; ctx.fillText(c.text, c.x, c.y)
      }
    }
    function fit() {
      const N = sim.nodes()
      if (!N.length || W < 2) return
      let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
      for (const n of N) { x0 = Math.min(x0, n.x - n.r); y0 = Math.min(y0, n.y - n.r); x1 = Math.max(x1, n.x + n.r); y1 = Math.max(y1, n.y + n.r + 14) }
      const p = pads, aw = Math.max(60, W - p.l - p.r), ah = Math.max(60, H - p.t - p.b)
      k = Math.max(0.05, Math.min(o.maxFit, (aw * 0.94) / Math.max(1, x1 - x0), (ah * 0.94) / Math.max(1, y1 - y0)))
      tx = p.l + aw / 2 - ((x0 + x1) / 2) * k; ty = p.t + ah / 2 - ((y0 + y1) / 2) * k
      dirty = true
    }
    function zoomAt(px, py, f) {
      const nk = Math.max(0.05, Math.min(8, k * f))
      tx = px - ((px - tx) * nk) / k; ty = py - ((py - ty) * nk) / k; k = nk
      fitting = false; fly = null; dirty = true; kick()
    }
    // Flying to a page: the camera eases to it over a moment (at once with reduced motion), following it as it moves.
    function flyStep() {
      const t = calm.matches ? 1 : Math.min(1, (performance.now() - fly.t0) / 380), e = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2
      const p = pads, ttx = p.l + (W - p.l - p.r) / 2 - fly.n.x * fly.k, tty = p.t + (H - p.t - p.b) / 2 - fly.n.y * fly.k
      k = fly.from.k + (fly.k - fly.from.k) * e; tx = fly.from.tx + (ttx - fly.from.tx) * e; ty = fly.from.ty + (tty - fly.from.ty) * e
      dirty = true
      if (t >= 1) fly = null
    }
    function resize() {
      const r = canvas.getBoundingClientRect()
      if (r.width < 2 || r.height < 2) return
      pads = { l: 16, r: 16, t: 16, b: 16, ...(o.pad ? o.pad() : null) }
      const d = window.devicePixelRatio || 1
      if (r.width !== W || r.height !== H || d !== dpr) {
        dpr = d; W = r.width; H = r.height
        canvas.width = Math.round(W * dpr); canvas.height = Math.round(H * dpr)
        if (fitting) fit()
        draw(); dirty = false // a new size clears the canvas: drawn again now, not a frame later
      }
      kick()
    }

    const pt = (e) => { const r = canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top } }
    function nodeAt(p) {
      let best = null, bd = Infinity
      for (const n of sim.nodes()) {
        const d = Math.hypot(SX(n) - p.x, SY(n) - p.y)
        if (d <= Math.max(rad(n), 4) + 3 && d < bd) { best = n; bd = d }
      }
      return best
    }
    function setHover(n) { if (n !== hover) { hover = n; canvas.style.cursor = n ? 'pointer' : ''; dirty = true; kick() } }
    function onDown(e) {
      if (e.button) return
      const p = pt(e), n = nodeAt(p)
      down = { x: p.x, y: p.y, moved: false, n, dx: n ? n.x - (p.x - tx) / k : 0, dy: n ? n.y - (p.y - ty) / k : 0 }
      canvas.setPointerCapture(e.pointerId)
      if (n) { drag = n; hover = n; dirty = true; kick() } else pan = { x: p.x, y: p.y, tx, ty }
    }
    function onMove(e) {
      const p = pt(e)
      if (!down) return setHover(nodeAt(p))
      if (!down.moved) {
        if (Math.hypot(p.x - down.x, p.y - down.y) <= 3) return
        down.moved = true; fitting = false; fly = null
        if (drag) sim.hold(true); else canvas.style.cursor = 'grabbing'
      }
      if (drag) sim.fix(drag.id, (p.x - tx) / k + down.dx, (p.y - ty) / k + down.dy)
      else { tx = pan.tx + p.x - pan.x; ty = pan.ty + p.y - pan.y }
      dirty = true; kick()
    }
    function onUp() {
      if (down && !down.moved) {
        const id = down.n ? down.n.id : null, now = performance.now()
        if (id && o.open && lastClick && lastClick.id === id && now - lastClick.at < 350) { lastClick = null; o.open(id) } else { lastClick = { id, at: now }; if (o.click) o.click(id) }
      }
      if (drag) { sim.free(drag.id); sim.hold(false) }
      drag = null; pan = null; down = null
      canvas.style.cursor = hover ? 'pointer' : ''
      dirty = true; kick()
    }
    function onLeave() { if (!down) setHover(null) }
    function onWheel(e) { e.preventDefault(); const p = pt(e); zoomAt(p.x, p.y, Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0016))) }
    canvas.addEventListener('pointerdown', onDown)
    canvas.addEventListener('pointermove', onMove)
    canvas.addEventListener('pointerup', onUp)
    canvas.addEventListener('pointercancel', onUp)
    canvas.addEventListener('pointerleave', onLeave)
    canvas.addEventListener('wheel', onWheel, { passive: false })
    // The theme: Echo's own choice, or the system's appearance.
    const mo = new MutationObserver(theme)
    mo.observe(root, { attributes: true, attributeFilter: ['data-theme'] })
    const dark = matchMedia('(prefers-color-scheme: dark)')
    dark.addEventListener('change', theme)
    const ro = new ResizeObserver(resize)
    ro.observe(canvas)
    theme()

    return {
      mount(box) { if (canvas.parentNode !== box) box.appendChild(canvas); resize() },
      /** New nodes and links. `how`: 'fresh' forgets where earlier pages were; 'keep' (a file changed on disk) only
       *  nudges the layout and leaves the camera where the user put it. A first layout warms up out of sight (no burst
       *  from the spiral), one that changed (a filter) moves there in view; with reduced motion both settle first. */
      set(nodes, links, how) {
        const first = how === 'fresh' || !sim.nodes().length
        if (how === 'fresh') sim.forget()
        sim.set(nodes, links, how === 'keep' ? 0.1 : 0.5)
        hover = null; drag = null; down = null
        if (sel) sel = sim.node(sel.id)
        if (hits) hits = knGraphHits(sim.nodes(), o.q || '')
        canvas.setAttribute('aria-label', o.aria ? o.aria(sim.nodes().length, sim.links().length) : '')
        const t0 = performance.now(), budget = calm.matches ? 400 : first ? 120 : 0
        while (!sim.settled() && performance.now() - t0 < budget) sim.tick()
        if (how !== 'keep' || fitting) { fitting = true; fly = null; fit() }
        dirty = true; kick()
      },
      paint(colorOf) { for (const n of sim.nodes()) n.color = colorOf(n); dirty = true; kick() },
      select(id) { const n = id ? sim.node(id) : null; if (n !== sel) { sel = n; dirty = true; kick() } },
      search(q) { o.q = q; hits = knGraphHits(sim.nodes(), q); dirty = true; kick(); return hits ? hits.size : 0 },
      /** The search match with the most links. */
      best() { let b = null; for (const n of sim.nodes()) if (hits && hits.has(n.id) && (!b || n.deg > b.deg)) b = n; return b ? b.id : null },
      center(id) { const n = sim.node(id); if (n) { fitting = false; fly = { t0: performance.now(), from: { k, tx, ty }, n, k: Math.max(k, 1.2) }; kick() } },
      zoom(f) { zoomAt(W / 2, H / 2, f) },
      fit() { fitting = true; fly = null; fit(); kick() },
      destroy() {
        cancelAnimationFrame(raf); raf = 0
        mo.disconnect(); ro.disconnect(); dark.removeEventListener('change', theme)
        canvas.remove()
      },
    }
  }
  let gview = null, mview = null
  /** The folders (or tags) the graph colours, the most pages first: each its colour, the rest grey. */
  function graphGroups(g, by) {
    const count = new Map()
    for (const n of g.nodes) if (!n.tag) for (const x of by === 'tag' ? (n.tags || []).slice(0, 1) : [n.folder || '(top)']) count.set(x, (count.get(x) || 0) + 1)
    const list = [...count].sort((a, b) => b[1] - a[1])
    const top = by === 'tag' ? list.slice(0, 9) : list
    const color = new Map(top.map(([x], i) => [x, FOLDER_COLORS[i % FOLDER_COLORS.length]]))
    const colorOf = (n) => n.tag ? GREY : color.get(by === 'tag' ? (n.tags || [])[0] : n.folder || '(top)') || GREY
    const pages = g.nodes.filter((n) => !n.tag).length
    return { list: top, rest: pages - top.reduce((s, [, c]) => s + c, 0), color, colorOf }
  }
  const folderName = (fo) => fo === '(top)' ? 'top level' : `${fo}/`
  function graphView() {
    const s = st(), g = s.graph, o = s.gopt
    if (!g) return '<div class="kn-graph-wrap"><div class="skeleton" style="padding:24px"><i></i><i></i></div></div>'
    if (g.error) return `<div class="kn-graph-wrap"><p class="error" style="padding:20px">${esc(g.error)}</p></div>`
    const f = knFilterGraph(g, { hideIndex: o.hideIndex, isolated: o.isolated, tags: o.tags, focus: o.depth ? o.sel : null, depth: o.depth, hidden: o.hidden })
    const isolated = knIsolatedCount(g, o.hideIndex, o.hidden, o.tags)
    const alone = o.hideIndex && (g.hubs || []).length ? 'Linked only from the index, or not at all.' : 'No links in or out. <a href="#" data-kn="tab" data-kn-tab="settings">See Checks</a>'
    const seg = (key, vals) => `<div class="seg" role="group">${vals.map(([v, l]) => `<button data-kn="gopt" data-key="${key}" data-val="${v}" aria-pressed="${String(o[key]) === String(v)}">${l}</button>`).join('')}</div>`
    const sw = (key, label, sub) => `<div class="kn-gc-row"><span><b>${label}</b>${sub ? `<span class="set-sub">${sub}</span>` : ''}</span><button class="sw${o[key] ? ' on' : ''}" role="switch" aria-checked="${!!o[key]}" data-kn="gopt" data-key="${key}" data-val="${!o[key]}" aria-label="${esc(label)}"></button></div>`
    const sel = o.sel && g.nodes.find((n) => n.id === o.sel)
    const tags = sel && !sel.tag ? (sel.tags || []).slice(0, 8).map((t) => `<span class="tag">#${esc(t)}</span>`).join('') : ''
    const card = sel ? `<div class="kn-gcard"><div class="kn-gcard-h"><b>${esc(sel.title || sel.id)}</b><button class="icon-btn" data-kn="gsel" data-id="" aria-label="Close">${IC.close}</button></div><div class="set-sub"><code>${esc(sel.id)}</code></div>
      <div class="kn-nums"><div><b>${g.edges.filter(([a]) => a === sel.id).length}</b><span>links out</span></div><div><b>${g.edges.filter(([, b]) => b === sel.id).length}</b><span>links in</span></div><div><b>${(sel.tags || []).length}</b><span>tags</span></div></div>
      ${tags ? `<div class="kn-gtags">${tags}</div>` : ''}
      ${sel.tag ? '' : `<div class="kn-acts"><button class="btn primary sm" data-kn="open-tab" data-path="${esc(sel.id)}">Open page</button><button class="btn sm" data-kn="ask-about" data-path="${esc(sel.id)}">${IC.ask} Ask about it</button></div><button class="btn ghost sm" data-kn="gopt" data-key="depth" data-val="${o.depth ? 0 : 1}">${IC.graph} ${o.depth ? 'The whole graph' : 'Its local graph'}</button>`}</div>` : ''
    const gr = graphGroups(g, o.color)
    const off = new Set(o.hidden || [])
    const legend = o.color === 'folder'
      ? `<div class="kn-legend">${gr.list.map(([fo, c]) => `<button class="kn-leg${off.has(fo) ? ' off' : ''}" data-kn="gfold" data-folder="${esc(fo)}" aria-pressed="${!off.has(fo)}" title="${off.has(fo) ? 'Show' : 'Hide'} the pages in ${esc(folderName(fo))}"><i style="background:${gr.color.get(fo)}"></i>${esc(folderName(fo))} <b>${nf(c)}</b></button>`).join('')}</div>`
      : `<div class="kn-legend">${gr.list.map(([t, c]) => `<span class="kn-leg"><i style="background:${gr.color.get(t)}"></i>#${esc(t)} <b>${nf(c)}</b></span>`).join('')}${gr.rest ? `<span class="kn-leg"><i style="background:${GREY}"></i>other or none <b>${nf(gr.rest)}</b></span>` : ''}</div>`
    const big = s.bigGraph === g.vault && o.depth ? '<span>Over 5,000 pages, so the graph starts around one page — Local graph <b>Off</b> draws them all</span>' : ''
    return `<div class="kn-graph-wrap"><div class="kn-graph" id="kn-graph"></div>
      <div class="kn-gctl"><div class="kn-gfind"><input class="search" id="kn-gfind" placeholder="Find pages and paths" aria-label="Find in the graph" value="${esc(s.gq)}" autocomplete="off" spellcheck="false"><span class="kn-gfind-n" id="kn-gfind-n" aria-live="polite"></span></div>
        <div class="kn-gc-row">Colour by ${seg('color', [['folder', 'Folder'], ['tag', 'Tag']])}</div>
        <div class="kn-gc-row">Local graph ${seg('depth', [[0, 'Off'], [1, '1'], [2, '2'], [3, '3']])}</div>
        <div class="menu-sep"></div>
        ${sw('hideIndex', 'Hide index pages', g.hubs && g.hubs.length ? `${esc(g.hubs.map((h) => h.split('/').pop()).join(', '))} links to most pages; hidden, the real structure shows.` : '')}
        ${sw('isolated', `Show isolated pages${isolated ? ` <span class="tag kn-warn-tag">${isolated}</span>` : ''}`, isolated ? alone : '')}
        ${sw('tags', 'Tags as nodes')}
      </div>
      <div class="kn-gstats">${big}<span><b>${nf(f.nodes.filter((n) => !n.tag).length)}</b> pages</span><span><b>${nf(f.edges.length)}</b> links</span>${o.hideIndex && (g.hubs || []).length ? '<span>index hidden</span>' : ''}${off.size ? `<span>${many(off.size, 'folder')} hidden</span>` : ''}</div>
      ${card}${legend}
      <div class="kn-gzoom"><button class="icon-btn" data-kn="gzoom" data-f="in" aria-label="Zoom in" title="Zoom in">${IC.plus}</button><button class="icon-btn" data-kn="gzoom" data-f="out" aria-label="Zoom out" title="Zoom out">${IC.minus}</button><button class="icon-btn" data-kn="gzoom" data-f="fit" aria-label="Fit the graph" title="Fit the graph">${IC.fit}</button></div></div>`
  }
  function drawGraph() {
    const s = st(), g = s.graph, box = document.getElementById('kn-graph')
    if (!g || !box) return
    const o = s.gopt
    if (!gview || gview.vault !== g.vault) {
      if (gview) gview.destroy()
      gview = graphCanvas({
        click: (id) => { const x = st().gopt; if (id || !x.depth) { x.sel = id; render() } },
        open: (id) => { if (!id.startsWith('#')) openPage(id) },
        // The controls sit over the graph's left side: fit it to what's left of them.
        pad: () => { const c = document.querySelector('.kn-gctl'); return { l: c ? c.offsetLeft + c.offsetWidth + 16 : 16, r: 16, t: 16, b: 48 } },
        aria: (n, m) => `Graph of ${many(n, 'page')} and ${many(m, 'link')}`,
      })
      gview.vault = g.vault
    }
    gview.mount(box)
    const gr = graphGroups(g, o.color)
    // What is shown; a new `rev` alone (a file changed on disk) keeps the camera and the layout as they are.
    const shown = `${g.mode}:${o.hideIndex}:${o.isolated}:${o.tags}:${o.depth}:${o.depth ? o.sel : ''}:${(o.hidden || []).join('\n')}`
    if (gview.shown !== shown || gview.rev !== g.rev) {
      const f = knFilterGraph(g, { hideIndex: o.hideIndex, isolated: o.isolated, tags: o.tags, focus: o.depth ? o.sel : null, depth: o.depth, hidden: o.hidden })
      gview.set(f.nodes.map((n) => ({ ...n, title: n.title || n.id, label: knShortLabel(n.title || n.id.split('/').pop()), color: gr.colorOf(n), shape: n.tag ? 'tag' : '' })), f.edges, gview.shown === shown ? 'keep' : '')
      gview.shown = shown; gview.rev = g.rev; gview.ckey = o.color
    } else if (gview.ckey !== o.color) { gview.paint(gr.colorOf); gview.ckey = o.color }
    gview.select(o.sel || null)
    const n = gview.search(s.gq), el = document.getElementById('kn-gfind-n')
    if (el) el.textContent = s.gq.trim() ? (n ? nf(n) : 'none') : ''
  }
  function drawMini() {
    const s = st(), p = s.page, box = document.getElementById('kn-mini')
    if (!p || !box || !p.local) return
    if (!mview) mview = graphCanvas({ labels: 'hover', maxFit: 1.6, long: 34, forces: { distance: 18, charge: 26 }, click: (id) => { if (id && id !== (st().page || {}).path) openPage(id) }, aria: (n, m) => `This page's local graph: ${many(n, 'page')} and ${many(m, 'link')}` })
    mview.mount(box)
    const key = `${p.vault}:${p.path}:${p.base}`
    if (mview.key === key) return
    mview.key = key
    mview.set(p.local.nodes.map((n) => ({ id: n.id, title: n.title || n.id, label: knShortLabel(n.title || n.id.split('/').pop(), 24), color: n.me ? '#9b78ff' : '#6cc4f0', pin: !!n.me })), p.local.edges, 'fresh')
  }

  // ── a vault's Settings ──
  function settingsView() {
    const s = st(), x = s.settings, v = vault()
    if (!x || !v || x.vault.id !== v.id) return '<div class="kn-vset"><div class="skeleton" style="padding:24px"><i></i><i></i><i></i></div></div>'
    const d = v.detected || {}, P = v.paths || {}
    const row = (t, sub, ctl, id) => `<div class="set-row"${id ? ` id="${id}"` : ''}><div><b>${t}</b><div class="set-sub">${sub}</div></div><div class="set-ctl">${ctl}</div></div>`
    const editing = s.ovr.key
    const change = (key, label, value, sub) => editing === key
      ? `<div class="set-row"><div><b>${label}</b><div class="kn-inline"><input class="search" id="kn-ovr" value="${esc(s.ovr.value)}" aria-label="${esc(label)}" spellcheck="false"><button class="btn primary sm" data-kn="ovr-save">Save</button><button class="btn ghost sm" data-kn="ovr-cancel">Cancel</button>${(v.overrides || {})[key] ? '<button class="btn ghost sm" data-kn="ovr-reset">As detected</button>' : ''}</div>${s.ovr.error ? `<div class="error">${esc(s.ovr.error)}</div>` : ''}</div><div></div></div>`
      : row(label, `${value ? `<code>${esc(value)}</code>` : '<span class="set-sub">none found</span>'}${sub ? ` — ${sub}` : ''}${(v.overrides || {})[key] ? ' <span class="tag">changed by you</span>' : ''}`, `<button class="btn ghost sm" data-kn="ovr" data-key="${key}" data-value="${esc(value || '')}">Change</button>`)
    const nav = [['vs-structure', 'Structure'], ['vs-minutes', 'Meeting minutes'], ['vs-schema', 'Schema & vocabulary'], ['vs-skills', 'Skills'], ['vs-git', 'Git & AI']]
    const m = v.minutes || {}
    const sel = (key, opts, val) => `<select class="set-in" data-kn-set="${key}" aria-label="${esc(key)}">${opts.map(([k, l]) => `<option value="${k}"${k === val ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>`
    const scopeSome = m.scope === 'some'
    const threads = ((S.pj && S.pj.data && S.pj.data.projects) || []).filter((p) => p.group !== 'closed')
    const clients = (S.pj && S.pj.data && S.pj.data.clients) || []
    const chosen = (list, ids) => list.filter((x) => ids.some((i) => i === x.id || i.toLowerCase() === String(x.name).toLowerCase()))
    const someRow = scopeSome ? row('Clients and threads', `${chosen(clients, m.clients || []).map((x) => `<span class="tag">${esc(x.name)}</span>`).join(' ') || ''} ${chosen(threads, m.threads || []).map((x) => `<span class="tag">${esc(x.name)}</span>`).join(' ') || ''}${!(m.clients || []).length && !(m.threads || []).length ? 'None chosen yet — no call is saved here by itself.' : ''}`,
      `<select class="set-in" data-kn-add="clients" aria-label="Add a client"><option value="">Add a client…</option>${clients.map((x) => `<option value="${esc(x.id)}">${esc(x.name)}</option>`).join('')}</select><select class="set-in" data-kn-add="threads" aria-label="Add a thread"><option value="">Add a thread…</option>${threads.map((x) => `<option value="${esc(x.id)}">${esc(x.name)}</option>`).join('')}</select>${(m.clients || []).length || (m.threads || []).length ? '<button class="btn ghost sm" data-kn="scope-clear">Clear</button>' : ''}`) : ''
    const past = x.past && x.past.count ? row(`Save past minutes <span class="tag">${x.past.count}</span>`, `Earlier calls that fit this rule and aren't saved here yet${x.past.first && x.past.first.length ? `: ${x.past.first.slice(0, 3).map((f) => esc(f.title)).join(', ')}${x.past.count > 3 ? '…' : ''}` : ''}. One note each${v.commit ? ', one commit' : ''}.`, `<button class="btn sm" data-kn="past"${s.busy.past ? ' disabled' : ''}>${s.busy.past ? 'Saving…' : `Save ${x.past.count}…`}</button>`) : ''
    const skills = (x.skills || []).filter((k) => k.kind !== 'agents')
    const agents = (x.skills || []).filter((k) => k.kind === 'agents')
    const skillRow = (k) => row(`${esc(k.name)} <span class="tag">${k.kind === 'command' ? '.claude/commands' : '.claude/skills'}</span>`, k.description ? `<span class="kn-clamp" title="${esc(k.description)}">${esc(k.description)}</span>` : 'no description', `<button class="btn sm" data-kn="run-skill" data-name="${esc(k.name)}">Run in Claude</button><button class="btn ghost sm" data-kn="open-tab" data-path="${esc(k.path)}">Open</button>`)
    const twinRow = x.has_twin ? row(`Use /twin from any Claude Code session${x.global_twin ? ` <span class="tag ok">installed as /${esc(x.global_twin)}</span>` : ''}`, x.global_twin ? `In <code>~/.claude/skills/${esc(x.global_twin)}/</code> — your own Claude Code. Sessions on other AI profiles don't see it.` : `A copy of this vault's twin skill with its paths, in <code>~/.claude/skills/${esc(x.global_twin_name)}/</code> — shown first, yours to remove.`, x.global_twin ? `<button class="btn sm" data-kn="twin-global">Update</button><button class="btn ghost sm" data-kn="twin-remove">Remove</button>` : `<button class="btn sm" data-kn="twin-global-ask">Install…</button>`)
      : row('twin <span class="tag acc">Echo starter</span>', 'Find, answer and remember — not in this vault. Adds <code>.claude/skills/twin/SKILL.md</code>; nothing else changes.', `<button class="btn sm" data-kn="twin-add"${s.busy.twin ? ' disabled' : ''}>Add…</button>`)
    const g = v.git
    const profiles = (typeof ai === 'function' ? ai().profiles : []) || []
    return `<div class="kn-vset"><nav class="kn-vset-nav">${nav.map(([id, l]) => `<button class="set-nav" data-kn="vs-go" data-to="${id}">${l}</button>`).join('')}<button class="set-nav" data-kn="tab" data-kn-tab="health">Health<span class="kn-nav-r" aria-hidden="true">→</span></button><span class="grow"></span><button class="set-nav danger" data-kn="remove-ask">Remove from Echo…</button></nav>
    <div class="kn-vset-main" data-keep-scroll>
      <section class="set-sec" id="vs-structure"><h3>Structure</h3><div class="set-card">
        ${row('Name', '', `<input class="set-in" data-kn-set="name" value="${esc(v.name)}" aria-label="The vault's name">`)}
        ${change('pages', 'Pages live in', P.pages === '' ? '(the folder itself)' : P.pages, d.obsidian ? 'Obsidian opens this folder.' : '')}
        ${change('index', 'Index', P.index, x.index_bytes ? `${kb(x.index_bytes)}${x.index_bytes > 60000 ? ' — large: Claude searches it rather than reading it whole' : ''}` : '')}
        ${change('notes', 'Meeting notes', P.notes, d.notes_count != null ? `${many(d.notes_count, 'note')}, named <code>${esc(P.naming || '{date}-{slug}.md')}</code>` : '')}
        ${change('sources', 'Sources', P.sources, `never edited by Echo${(d.ledgers || []).length ? ` · ledgers ${d.ledgers.map((l) => `<code>${esc(l)}</code>`).join(', ')}` : ''}`)}
        ${row('Read again', 'Detect the structure again — your changes above stay.', `<button class="btn sm" data-kn="reread"${s.busy.reread ? ' disabled' : ''}>Read again</button>`)}
      </div></section>
      <section class="set-sec" id="vs-minutes"><h3>Meeting minutes</h3><div class="set-card">
        ${row('Save each call\'s minutes here', 'After Echo writes them. Calls recorded with another AI profile are never saved here by themselves.', sel('minutes', [['auto', 'Automatically'], ['ask', 'Only when I ask'], ['off', 'Never']], m.mode))}
        ${row('Which calls', 'All calls and memos, calls only, or the calls of some clients and threads', sel('scope', [['all', 'All my calls'], ['calls', 'Calls, not memos'], ['some', 'Some clients and threads']], m.scope))}
        ${someRow}
        ${row('As', `<code>${esc(P.notes || 'notes')}/${esc(P.naming || '{date}-{slug}.md')}</code> · a row in <code>${esc((P.index || 'the index').split('/').pop())}</code> · client and thread in the properties · tag`, `<input class="set-in sm" data-kn-set="tag" value="${esc(x.meeting_tag)}" aria-label="The tag meeting notes get">`)}
        ${row('Also include', 'The transcript and the meeting chat stay in Echo unless you add them.', `<label class="kn-check"><input type="checkbox" data-kn-set="transcript"${m.transcript ? ' checked' : ''}> Transcript</label><label class="kn-check"><input type="checkbox" data-kn-set="chat"${m.chat ? ' checked' : ''}> Chat</label>`)}
        ${row('Preview', 'How a call lands here: the newest call\'s note and its index row. Nothing is written.', '<button class="btn ghost sm" data-kn="sample">Preview…</button>')}
        ${past}
      </div></section>
      <section class="set-sec" id="vs-schema"><h3>Schema & vocabulary</h3><div class="set-card">
        ${(d.schema || []).length ? row(d.schema.map((f) => `<code>${esc(f)}</code>`).join(' · '), 'How agents work in this vault — the rules every skill follows', `<button class="btn ghost sm" data-kn="open-tab" data-path="${esc(d.schema[0])}">Open</button>`) : row('No rules for agents', 'An <code>AGENTS.md</code> or <code>CLAUDE.md</code> at the vault\'s root tells Claude how to work here.', '')}
        ${(d.taxonomies || []).map((t) => row(`<code>${esc(t)}</code>`, /TAG/i.test(t) ? `${x.tags != null ? `${x.tags} tags · ` : ''}the type tag first` : `${x.terms != null ? `${x.terms} terms with their mishearings` : 'canonical names and terms'}`, `<button class="btn ghost sm" data-kn="open-tab" data-path="${esc(t)}">Open</button>`)).join('')}
      </div></section>
      <section class="set-sec" id="vs-skills"><h3>Skills</h3><div class="set-card">
        ${skills.map(skillRow).join('') || row('No skills', 'Skills in <code>.claude/skills</code> or <code>.claude/commands</code> appear here.', '')}
        ${agents.length ? row(`${agents.length} for other agents`, `${agents.map((k) => esc(k.name)).join(' · ')} — in <code>.agents/skills/</code>`, '') : ''}
        ${twinRow}
      </div></section>
      <section class="set-sec" id="vs-checks"><h3>Checks</h3><div class="kn-info">${IC.info}<span>Checks are in <button class="kn-lnk" data-kn="tab" data-kn-tab="health">Health</button>.</span></div></section>
      <section class="set-sec" id="vs-git"><h3>Git & AI</h3><div class="set-card">
        ${row('Git', g ? `${(g.remotes || []).length ? `<code>${esc(g.remotes.join(', '))}</code> · ` : ''}${IC.git} ${esc(g.branch || 'detached HEAD')}${g.changed ? ` · ${g.changed} file${g.changed === 1 ? '' : 's'} changed and not committed — yours; Echo never commits them` : ' · clean'}${g.busy ? ` · <span class="warn">${esc(g.busy)}</span>` : ''}` : (st().data || {}).git ? 'Not a git repository — pages have no history in Echo.' : 'git isn\'t installed — pages have no history in Echo.', `<button class="btn ghost sm" data-kn="reveal" data-path="">Reveal</button>`)}
        ${g ? row('Commit what Echo writes', v.commit ? 'One commit per change, only Echo\'s files; never a push, pull or branch switch.' : 'Off: Echo writes the files and leaves committing to you.', `<button class="sw${v.commit ? ' on' : ''}" role="switch" aria-checked="${!!v.commit}" data-kn="set-bool" data-key="commit" data-val="${!v.commit}" aria-label="Commit what Echo writes"></button>`) : ''}
        ${row('AI for this vault', 'Ask sessions here use this profile. Only calls recorded on it are saved here by themselves.', `<select class="set-in" data-kn-set="profile" aria-label="AI profile">${profiles.map((p) => `<option value="${esc(p.id)}"${p.id === v.profile ? ' selected' : ''}>${esc(p.name)}</option>`).join('') || `<option>${esc(v.profile)}</option>`}</select>`)}
        ${x.in_home && v.profile !== (st().data || {}).default_profile ? row('Inside Echo\'s data folder', 'Claude · Echo (the folder session) can read every file of the data folder, this vault too, whatever its profile.', '') : ''}
        ${row('Echo\'s index of this vault', `In Echo's data folder, not in the vault${x.cache_bytes ? ` · ${kb(x.cache_bytes)}` : ''} · rebuilt on its own`, `<button class="btn ghost sm" data-kn="reread">Rebuild</button>`)}
      </div></section>
    </div></div>`
  }

  // ── Health (spec K16): the score and its trend, how Health checks, the findings by group and kind, the finding shown
  // with its fix as a diff. Findings are named by key only, and a fix is sent back with the base and sha of the diff
  // shown (ruling R37). Titles, page names, lines, reasons and diff lines are others' words: esc(). AI checks (Task 14)
  // aren't here yet: the card says "code checks only" and has no *Run AI checks*. ──
  function hl() {
    const s = st()
    if (!s.hl || s.hl.vault !== s.vault) s.hl = { vault: s.vault, data: null, rev: null, at: 0, error: null, detail: null, want: '', choice: {}, filter: 'all', showIgnored: false, loading: false }
    return s.hl
  }
  const hlList = () => { const h = hl(), d = h.data || {}; return knHealthList(d.findings, { filter: h.filter, ignored: d.ignored, showIgnored: h.showIgnored }) }
  const hlFinding = (key) => { const d = hl().data || {}; return [...(d.findings || []), ...(d.ignored || [])].find((f) => f.key === key) || null }
  const isoDay = (d = new Date()) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  /** A finding's pages, each once with its lines. */
  const pagesOf = (f) => { const m = new Map(); for (const [p, l] of (f && f.pages) || []) { if (!m.has(p)) m.set(p, []); if (l != null) m.get(p).push(l) } return [...m] }
  const where = (p, lines) => `${p}${lines.length ? ` · ${lines.length === 1 ? 'line' : 'lines'} ${lines.join(', ')}` : ''}`
  const hunkRows = (hunks) => (hunks || []).map((h, k) => (k ? '<div class="hd">…</div>' : '') + h.map(([o, l]) => `<div class="${o === '+' ? 'add' : o === '-' ? 'del' : ''}">${o === '+' ? '+' : o === '-' ? '−' : ' '} ${esc(l)}</div>`).join('')).join('')
  const pmOf = (x) => `<span class="kn-pm">${x.plus ? `<span class="a">+${x.plus}</span>` : ''}${x.plus && x.minus ? ' ' : ''}${x.minus ? `<span class="d">−${x.minus}</span>` : ''}</span>`
  function healthView() {
    const h = hl(), d = h.data
    if (!d) return `<div class="kn-hl">${h.error ? `<div class="kn-none"><p class="error">${esc(h.error)}</p></div>` : '<div class="skeleton" style="padding:24px"><i></i><i></i><i></i></div>'}</div>`
    const list = hlList(), sel = knHealthSelect(list, st().hlKey)
    return `<div class="kn-hl" data-keep-scroll><div class="kn-hl-in">${hlTop(list)}<div class="kn-hl-f${sel ? '' : ' kn-hl-none'}">${hlFindings(list, sel)}${sel ? hlDetail(sel) : ''}</div></div></div>`
  }
  /** The score card and *How Health checks* (K16-01). */
  function hlTop(list) {
    const h = hl(), d = h.data, v = vault(), sc = d.score || {}, total = sc.total == null ? 100 : sc.total
    const today = isoDay(), tr = knHealthTrend(d.history, today, 30), p = knTrendPath(tr.points, { w: 300, h: 44 })
    const word = total >= 80 ? 'Tidy' : total >= 60 ? 'Mostly tidy' : 'Needs tidying'
    const head = list.needs ? `${word}, with ${many(list.needs, 'thing')} only you can decide` : word
    const delta = tr.delta == null ? '' : `<span class="tag ${tr.delta >= 0 ? 'ok' : 'warn'}">${tr.delta >= 0 ? '+' : '−'}${Math.abs(tr.delta)} in 30 days</span>`
    const day = (x) => (x.day === today ? 'today' : new Date(`${x.day}T12:00:00`).toLocaleDateString([], { day: 'numeric', month: 'short' }))
    const since = tr.first && tr.first.day !== tr.last.day ? `${day(tr.first)} ${tr.first.score} · ${day(tr.last)} ${tr.last.score}` : 'The trend starts today: one point a day.'
    const dot = ([x, y], r) => `<circle cx="${x}" cy="${y}" r="${r}" fill="var(--accent)"/>`
    const spark = `<svg class="kn-spark" viewBox="0 0 300 44" preserveAspectRatio="none" aria-hidden="true"><path d="${p.area}" fill="var(--accent-soft)"/><path d="${p.line}" fill="none" stroke="var(--accent)" stroke-width="1.8" vector-effect="non-scaling-stroke"/>${p.dots.map((x) => dot(x, 2)).join('')}${p.end ? dot(p.end, 3) : ''}</svg>`
    const num = (x) => nf(Math.round(x * 10) / 10)
    const bars = (sc.categories || []).map(([name, pts, w]) => `<span>${esc(name)}</span><div class="kn-hbar" role="img" aria-label="${esc(name)}: ${num(pts)} of ${w}"><i style="width:${Math.max(0, Math.min(100, Math.round((pts / w) * 100)))}%"></i></div><span>${num(pts)}/${w}</span>`).join('')
    const prof = (typeof profileOf === 'function' && profileOf(v.profile)) || {}
    const lastAi = d.ai ? `last ${esc(whenOf(d.ai.at))}${d.ai.cost != null ? `, ${esc(usd(d.ai.cost))}` : ''}` : 'not run yet'
    return `<div class="kn-hl-top">
      <div class="kn-card"><div class="kn-hl-score"><div class="kn-ring" style="--p:${total}%" role="img" aria-label="Score ${total} of 100"><span>${total}<small>of 100</small></span></div>
        <div class="kn-trend"><b>${esc(head)} ${delta}${sc.code_only ? '<span class="tag">code checks only</span>' : ''}</b>${spark}<div class="set-sub">${esc(since)} · checked ${esc(hm(new Date(h.at)))}</div></div>
        <div class="kn-hbars">${bars}</div></div></div>
      <div class="kn-card"><div class="kn-card-h"><h3>How Health checks</h3></div><div class="kn-hrun">
        <div class="r"><div><b>By code <span class="kn-mtag">no model</span></b><div class="set-sub">after every change Echo sees · checked ${esc(hm(new Date(h.at)))}</div></div><span class="tag ok">always</span></div>
        <div class="r"><div><b>AI checks <span class="kn-mtag ai">AI</span></b><div class="set-sub">when you ask · ${esc(prof.name || v.profile || 'the vault\'s AI profile')} · ${lastAi}</div></div></div>
      </div></div></div>`
  }
  /** The findings by group and kind, with the filters and *Show ignored*. */
  function hlFindings(list, sel) {
    const h = hl(), d = h.data
    const f = (id, label) => `<button class="${h.filter === id ? 'on' : ''}" data-kn="hl-filter" data-kn-filter="${id}" aria-pressed="${h.filter === id}">${label}</button>`
    const fi = (k) => `<button class="kn-fi${sel && sel.kind === k ? ' on' : ''}${k.group === 'needs' && !k.ignored ? ' need' : ''}" data-kn="hl-pick" data-kn-key="${esc(k.keys[0])}"${sel && sel.kind === k ? ' aria-current="true"' : ''}><span class="nb">${nf(k.n)}</span><b>${esc(k.name)}</b><span class="tg"><span class="kn-mtag${k.by === 'ai' ? ' ai' : ''}">${k.by === 'ai' ? 'AI' : 'code'}</span></span><span class="m">${esc(k.meta)}</span></button>`
    const groups = list.groups.map((g) => `<div class="kn-fg"><span>${esc(g.label)}</span><span>${esc(g.sub)}</span></div>${g.kinds.map(fi).join('')}`).join('')
    const none = list.groups.length ? '' : `<p class="set-sub kn-fnone">${h.filter === 'ai' && !d.ai ? 'AI checks haven\'t run yet.' : (d.findings || []).length ? 'Nothing under this filter.' : 'Nothing to fix. Echo checks again after every change it sees.'}</p>`
    const ign = (d.ignored || []).length ? `<button class="kn-lnk kn-fign" data-kn="hl-ignored" aria-pressed="${h.showIgnored}">${h.showIgnored ? 'Hide ignored' : `Show ignored (${nf(d.ignored.length)})`}</button>` : ''
    return `<div class="kn-flist"><div class="top"><span class="set-sub">${many(list.kinds, 'kind')} · ${nf(list.needs)} ${list.needs === 1 ? 'needs' : 'need'} you</span><div class="kn-filters" role="group" aria-label="Show">${f('all', 'All')}${f('needs', 'Needs you')}${f('code', 'By code')}${f('ai', 'AI')}</div></div>${groups}${none}${ign}</div>`
  }
  /** The kind shown: what it means, *Fix all* where it applies, its findings — the one shown open with its fix. */
  function hlDetail(sel) {
    const k = sel.kind, x = KN_HEALTH_KINDS[k.kind]
    const fixAll = x && x[2] && !k.ignored && k.fixAll > 1 ? `<button class="btn sm primary" data-kn="hl-fixall" data-kn-kind="${esc(k.kind)}">${esc(x[2](k.fixAll))}…</button>` : ''
    const keys = k.keys.slice(0, 50)
    if (!keys.includes(sel.key)) keys.push(sel.key)
    const more = k.n > keys.length ? `<div class="set-sub">and ${nf(k.n - keys.length)} more</div>` : ''
    const d = hl().data || {}, by = new Map([...(d.findings || []), ...(d.ignored || [])].map((f) => [f.key, f]))
    return `<div class="kn-fdet"><div class="kn-fdet-h"><div class="row"><h3>${esc(x ? x[1](k.n) : k.name)}</h3>${k.by === 'ai' ? '<span class="kn-mtag ai">AI</span>' : ''}${k.ignored ? '<span class="kn-mtag">ignored</span>' : k.group === 'needs' ? '<span class="kn-mtag you">needs you</span>' : ''}${fixAll}</div><p>${esc(k.why || '')}</p></div>
      <div class="kn-fdet-b">${keys.map((key) => (key === sel.key ? hlOpen(by.get(key)) : hlRow(by.get(key)))).join('')}${more}</div></div>`
  }
  /** A finding of the kind, not the one shown: a row to open it. */
  function hlRow(f) {
    if (!f) return ''
    const [p, lines] = pagesOf(f)[0] || ['', []]
    return `<button class="kn-fx kn-fx-row" data-kn="hl-pick" data-kn-key="${esc(f.key)}">${IC.file}<span><code>${esc(where(p, lines))}</code><span class="set-sub">${esc(f.title)}</span></span></button>`
  }
  /** The finding shown (K16-07): its pages with their lines (*Open*), the ways to fix it, the fix's diff — or why there
   *  is none —, and *Ask about it* · *Ignore* · *Fix*. */
  function hlOpen(f) {
    if (!f) return ''
    const s = st(), h = hl(), dt = h.detail, v = vault()
    const pages = pagesOf(f).map(([p, lines]) => `<div class="kn-fx-p"><code>${esc(where(p, lines))}</code><button class="btn ghost sm" data-kn="open" data-path="${esc(p)}">Open</button></div>`).join('')
    const head = `<div class="kn-fx-h">${IC.file}<div><b>${esc(f.title)}</b>${pages}</div></div>`
    if (!dt || dt.key !== f.key) return `<div class="kn-fx on">${head}<div class="skeleton" style="padding:12px"><i></i><i></i></div></div>`
    const choices = dt.choices || [], fixes = choices.filter((c) => c.choice !== 'create' && c.choice !== 'taxonomy')
    const fx = dt.fix || {}, cur = fx.error ? h.choice[f.key] || '' : fx.choice || ''
    const listed = fixes.some((c) => c.choice === cur)
    // A single way to fix it isn't offered as a choice when it is Echo's own (properties, an index row).
    const pick = !fixes.length || (fixes.length === 1 && listed && !h.choice[f.key]) ? ''
      : fixes.length > 4 ? `<div class="kn-fx-ch"><select class="set-in" data-kn-hl-choice="${esc(f.key)}" aria-label="How to fix it">${listed ? '' : '<option value="">Pick one…</option>'}${fixes.map((c) => `<option value="${esc(c.choice)}"${c.choice === cur ? ' selected' : ''}>${esc(c.label)}</option>`).join('')}</select></div>`
        : `<div class="kn-fx-ch" role="group" aria-label="How to fix it">${fixes.map((c) => `<button class="btn sm${c.choice === cur ? ' on' : ''}" data-kn="hl-choice" data-kn-key="${esc(f.key)}" data-kn-choice="${esc(c.choice)}" aria-pressed="${c.choice === cur}">${esc(c.label)}</button>`).join('')}</div>`
    const to = choices.find((c) => c.choice === cur && c.to)
    const only = !pick && listed && fixes.find((c) => c.choice === cur)
    const toLine = to ? `<div class="kn-fx-to set-sub">→ <b>${esc(to.to)}</b>${to.why ? ` · ${esc(to.why)}` : ''}${fx.guess ? ' · a guess' : ''}</div>`
      : only ? `<div class="kn-fx-to set-sub">${esc(only.label)}</div>` : ''
    const diff = (fx.hunks || []).length ? `<div class="kn-diff">${hunkRows(fx.hunks)}</div>` : ''
    const why = dt.error || (fx.error && (f.fixable || fixes.length) ? fx.error : '')
    const reason = why ? `<div class="kn-fx-why set-sub">${esc(String(why).replace(/\.$/, ''))}.</div>` : ''
    const tax = choices.some((c) => c.choice === 'taxonomy') && ((v.detected || {}).taxonomies || []).find((t) => /TAG/i.test(t))
    const ok = !!(diff && fx.base && fx.sha), busy = s.busy.hlfix
    const note = ok ? (fx.guess ? 'A guess, so Fix all leaves it out. Written only if the page still reads as it does now.' : 'Written only if the page still reads as it does now.') : ''
    const acts = `<button class="btn ghost sm" data-kn="hl-ask" data-kn-key="${esc(f.key)}">${IC.ask} Ask about it</button>`
      + `<button class="btn ghost sm" data-kn="hl-ignore" data-kn-key="${esc(f.key)}" data-kn-on="${!dt.ignored}">${dt.ignored ? 'List again' : 'Ignore'}</button>`
      + (choices.some((c) => c.choice === 'create') && dt.new_page ? `<button class="btn sm" data-kn="hl-create" data-name="${esc(dt.new_page.name)}" data-kn-folder="${esc(dt.new_page.folder)}">Create the page</button>` : '')
      + (tax ? `<button class="btn sm" data-kn="open" data-path="${esc(tax)}">Open the taxonomy</button>` : '')
      + (ok ? `<button class="btn primary sm" data-kn="hl-fix"${busy ? ' disabled' : ''}>${busy ? 'Fixing…' : 'Fix'}</button>` : '')
    return `<div class="kn-fx on">${head}${pick}${toLine}${diff ? `<div class="kn-fx-d">${pmOf(fx)}</div>${diff}` : ''}${reason}<div class="kn-fx-f"><span class="grow">${note}</span>${acts}</div></div>`
  }
  /** *Fix all* (K16-04): every fix of the kind as Echo found it, ticked; a guess is never one (R36). */
  async function openFixAll(kind) {
    const s = st(), v = vault()
    const x = (s.sheet = { kind: 'fixall', fixKind: kind, fixes: null, findings: [], off: new Set(), error: null })
    render()
    try {
      const d = await get(`api/kn/${enc(v.id)}/health?fixes=${enc(kind)}`)
      if (d.error) throw new Error(d.error)
      x.fixes = d.fixes || []
      x.findings = d.findings || []
    } catch (e) { x.error = String(e.message || e); x.fixes = [] }
    if (st().sheet === x) render()
  }
  function fixAllSheet() {
    const s = st(), x = s.sheet, v = vault(), k = KN_HEALTH_KINDS[x.fixKind] || []
    if (!x.fixes) return sheet('Fix all', '<div class="skeleton"><i></i><i></i><i></i></div>', '<span class="grow"></span><button class="btn ghost" data-kn="sheet-close">Cancel</button>', IC.check)
    const fa = knFixAll(x.fixes, x.findings, x.off), n = fa.items.length
    const row = (r) => `<div class="kn-fixall-row"><input type="checkbox" data-kn-fixall="${esc(r.key)}"${r.on ? ' checked' : ''}${r.error ? ' disabled' : ''} aria-label="${esc(`Fix ${r.title || r.path}`)}"><div class="kn-fx"><div class="kn-fx-h">${IC.file}<div><code>${esc(where(r.path, r.line == null ? [] : [r.line]))}</code><div class="set-sub">${esc(r.title)}</div></div>${r.error ? '' : pmOf(r)}</div>${r.error ? `<div class="kn-fx-why set-sub">${esc(String(r.error).replace(/\.$/, ''))}.</div>` : `<div class="kn-diff">${hunkRows(r.hunks)}</div>`}</div></div>`
    const body = `<p class="set-sub">Each fix as Echo found it. Untick any you'd rather leave.</p>${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}${fa.rows.map(row).join('') || '<p class="set-sub">Nothing left to fix here.</p>'}`
    return sheet(esc(k[2] ? k[2](fa.rows.length) : 'Fix all'), body, `<span class="kn-fa-foot" id="kn-fixall-f">${fixAllFoot(fa, x.fixKind)}</span>`, IC.check)
  }
  /** The sheet's foot: how many files (index rows also write the vault's log line), whether they are committed, *Apply
   *  N* — redrawn alone when a fix is (un)ticked, so the checkbox keeps the focus. */
  function fixAllFoot(fa, kind) {
    const s = st(), v = vault(), n = fa.items.length, log = kind === 'not_in_index' && n && (v.detected || {}).log
    const commits = v.commit ? 'one commit of these files' : v.git ? 'commits are off for this vault: the files are written, nothing is committed' : 'the files are written'
    return `<span class="set-sub grow">${many(fa.files, 'file')}${log ? ` and a line in ${esc(log)}` : ''} · ${commits}</span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="hl-fixall-go"${!n || s.busy.fixall ? ' disabled' : ''}>${s.busy.fixall ? 'Applying…' : `Apply ${n}`}</button>`
  }
  /** After *Fix* or *Fix all*: Health checked again, and what was written and what wasn't, said. */
  async function fixed(r) {
    const d = hl().data || {}, m = knFixSummary(r, [...(d.findings || []), ...(d.ignored || [])])
    await loadHealth()
    render()
    if (m.wait) say(m.text); else done(m.text)
  }
  async function fixOne() {
    const v = vault(), dt = hl().detail, fx = (dt && dt.fix) || {}
    if (!fx.base || !fx.sha) return
    try { await fixed(await run('kn_fix', { id: v.id, key: dt.key, choice: fx.choice || '', base: fx.base, sha: fx.sha }, 'hlfix')) } catch (err) { say(`Not fixed: ${err.message || err}`) }
  }
  async function fixAllGo() {
    const s = st(), x = s.sheet, v = vault(), items = knFixAll(x.fixes, x.findings, x.off).items
    if (!items.length) return
    try {
      const r = await run('kn_fix_all', { id: v.id, items }, 'fixall')
      if (st().sheet === x) s.sheet = null
      await fixed(r)
    } catch (err) { x.error = String(err.message || err); render() }
  }
  async function ignoreFinding(key, on) {
    const v = vault()
    try { await run('kn_ignore', { id: v.id, key, on }, 'hlignore'); await loadHealth(); render(); done(on ? 'Ignored: it no longer counts. Show ignored lists it.' : 'Listed again.') } catch (err) { say(String(err.message || err)) }
  }
  /** *Ask about it*: a new conversation with the finding — what it is, its pages as chips — in the composer. */
  function askAboutFinding(key) {
    const f = hlFinding(key), v = vault()
    if (!f || !v) return
    const line = `Health found this in ${v.name}: ${f.title}. ${f.why}`
    const titleOf = (p) => ((st().tree && st().tree.pages.find((x) => x.path === p)) || {}).title || p.split('/').pop().replace(/\.md$/i, '')
    askHere((c) => {
      for (const [p] of pagesOf(f)) if (!c.chips.some((x) => x.page === p)) c.chips.push({ page: p, title: titleOf(p) })
      if (!c.draft.includes(line)) c.draft = c.draft.trim() ? `${line}\n\n${c.draft}` : `${line}\n\n`
    }, false)
  }
  async function pickChoice(key, choice) {
    const h = hl()
    h.choice[key] = choice
    render()
    await loadFinding()
    render()
  }

  // ── sheets: connect, new vault, new page, rows, remove, global twin, save minutes ──
  /** Ask before an action, in the page: the window's webview shows no confirm() dialogs. */
  function askFirst(title, text, yes, run, danger = false) { st().asking = { title, text, yes, run, danger }; render() }
  function askSheet() {
    const a = st().asking
    return `<div class="kn-sheet-back kn-ask-back" data-kn="ask-back"><div class="kn-sheet kn-ask" role="alertdialog" aria-modal="true" aria-label="${esc(a.title)}"><div class="kn-sheet-h"><h2>${esc(a.title)}</h2></div><div class="kn-sheet-b"><p>${esc(a.text)}</p></div><div class="kn-sheet-f"><span class="grow"></span><button class="btn ghost" data-kn="ask-no">Cancel</button><button class="btn ${a.danger ? 'danger' : 'primary'}" data-kn="ask-yes">${esc(a.yes)}</button></div></div></div>`
  }
  function sheet(titleHtml, body, foot, icon = IC.gem, steps = '') {
    return `<div class="kn-sheet-back" data-kn="sheet-back"><div class="kn-sheet" role="dialog" aria-modal="true" aria-label="${esc(titleHtml.replace(/<[^>]+>/g, ''))}"><div class="kn-sheet-h"><span class="kn-gem">${icon}</span><h2>${titleHtml}</h2><button class="icon-btn" data-kn="sheet-close" aria-label="Close">${IC.close}</button></div>${steps}<div class="kn-sheet-b">${body}</div><div class="kn-sheet-f">${foot}</div></div></div>`
  }
  function connectSheet() {
    const s = st(), x = s.sheet
    const steps = ['Folder', 'What Echo found', 'Meetings & AI'].map((l, i) => `<span class="${i + 1 === x.step ? 'on' : i + 1 < x.step ? 'done' : ''}"><i>${i + 1 < x.step ? '✓' : i + 1}</i>${l}</span>`).join('')
    const stepsHtml = `<div class="kn-steps-bar">${steps}</div>`
    const p = x.picked
    let body = '', foot = ''
    if (x.step === 1) {
      const chosen = p ? `<div class="kn-field"><span>Folder</span><div class="kn-inline"><span class="kn-path" title="${esc(p.path)}"><code><bdi>${esc(short(p.path))}</bdi></code></span><button class="btn" data-kn="pick"${x.picking ? ' disabled' : ''}>${x.picking ? 'Choosing…' : 'Choose another…'}</button></div></div>` : `<div class="kn-field"><span>Folder</span><div class="kn-inline"><button class="btn primary" data-kn="pick"${x.picking ? ' disabled' : ''}>${x.picking ? 'Choosing…' : 'Choose…'}</button></div></div>`
      const refused = p && p.refused ? `<div class="kn-warn">${IC.warn}<span>${esc(p.refused)}</span></div>` : ''
      const empty = p && p.empty && !p.refused ? `<div class="kn-info">${IC.info}<span>This folder is empty. Start a new vault in it instead — Echo's twin starter puts the structure and skills there. <button class="btn sm" data-kn="new-here">Start a vault here…</button></span></div>` : ''
      const nomd = p && !p.empty && !p.refused && p.detected && p.detected.md_files === 0 ? `<div class="kn-info">${IC.info}<span>No Markdown pages here yet. It can be connected anyway; pages appear as you add them.</span></div>` : ''
      body = `${chosen}${refused}${empty}${nomd}<div class="kn-info">${IC.info}<span>Any folder of Markdown: an Obsidian vault, a git repository, a shared drive's folder. Echo only reads it now; nothing in it changes until you save something from Echo.</span></div>`
      foot = `<span class="set-sub grow">Nothing changes until you press Connect.</span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="step" data-step="2"${p && !p.refused && !p.empty ? '' : ' disabled'}>Next</button>`
    } else if (x.step === 2) {
      const d = (p && p.detected) || {}
      const f = (k, label, val, sub) => `<div class="kn-found-r"><span class="${val ? 'ok' : 'none'}">${val ? IC.check : '—'}</span><span class="k">${label}</span><span class="v">${val ? `<code>${esc(val)}</code>` : '<span class="set-sub">none found</span>'}${sub ? ` <span class="set-sub">${sub}</span>` : ''}</span></div>`
      const size = p.index_bytes ? `${kb(p.index_bytes)}${p.index_bytes > 60000 ? ' — large: Claude will search it' : ''}` : ''
      body = `<div class="kn-found">
        ${f('pages', 'Pages', d.pages === '' ? '(the folder itself)' : d.pages, `${many(d.md_files, 'page')}${d.obsidian ? ' · Obsidian settings found' : ''}`)}
        ${f('index', 'Index', d.index, size)}
        ${f('notes', 'Meeting notes', d.notes, d.notes ? `${many(d.notes_count, 'note')} · ${esc(d.naming)}` : 'Echo makes <code>notes/</code> when it saves the first')}
        ${f('tax', 'Tags and terms', (d.taxonomies || []).map((t) => t.split('/').pop()).join(' · '), '')}
        ${f('schema', 'Rules for agents', (d.schema || []).join(' · '), '')}
        ${f('skills', 'Skills', (d.skills || []).length ? String(d.skills.length) : '', (d.skills || []).slice(0, 3).map((k) => esc(k.name)).join(', ') + ((d.skills || []).length > 3 ? ', …' : ''))}
        ${f('sources', 'Sources', [d.sources, d.staging].filter(Boolean).join(' · '), d.sources || d.staging ? 'never edited by Echo' : '')}
        ${f('ledgers', 'Ledgers', (d.ledgers || []).length ? String(d.ledgers.length) : '', esc((d.ledgers || []).join(', ')))}
        ${f('git', 'Git', p.git ? `${(p.git.remotes || []).join(', ') || 'no remote'} · ${p.git.branch || 'detached'}` : '', '')}
      </div>${p.git && p.git.changed ? `<div class="kn-warn">${IC.warn}<span><b>${p.git.changed} file${p.git.changed === 1 ? ' is' : 's are'} changed and not committed.</b> They're yours: Echo never commits them.</span></div>` : ''}
      <p class="note-line">${IC.info}<span>Echo keeps its index of these pages in its own data folder — the vault gets no new files. Anything here can be changed later in the vault's Settings.</span></p>`
      foot = `<span class="set-sub grow">Nothing in your folder has changed.</span><button class="btn ghost" data-kn="step" data-step="1">Back</button><button class="btn primary" data-kn="step" data-step="3">Next</button>`
    } else {
      const profiles = (typeof ai === 'function' ? ai().profiles : []) || []
      const def = (typeof ai === 'function' ? ai().default : '') || 'system'
      const radio = (val, label, sub) => `<label class="kn-radio"><input type="radio" name="kn-minutes" value="${val}" data-kn-form="minutes"${x.minutes === val ? ' checked' : ''}><span><b>${label}</b>${sub ? `<span class="set-sub">${sub}</span>` : ''}</span></label>`
      body = `<div class="kn-lbl">Meeting minutes</div>
        ${radio('auto', 'Save each call\'s minutes here automatically', `Right after Echo writes them, in <code>${esc((p.detected && p.detected.notes) || 'notes')}/</code>, with a row in the index.`)}
        ${radio('ask', 'Only when I ask', 'A call\'s ⋯ → Save to Knowledge.')}
        ${radio('off', 'Never', '')}
        <div class="kn-field"><span>Name</span><input class="search kn-in" id="kn-name" value="${esc(x.name || p.name || '')}" aria-label="The vault's name"></div>
        <div class="kn-field"><span>Read by</span><select class="set-in" data-kn-form="profile" aria-label="AI profile">${profiles.map((q) => `<option value="${esc(q.id)}"${q.id === (x.profile || def) ? ' selected' : ''}>${esc(q.name)}</option>`).join('')}</select></div>
        <p class="set-sub kn-indent">Ask sessions in this vault use this profile. Calls recorded with another profile are never saved here by themselves.</p>
        ${p.git ? `<label class="kn-check"><input type="checkbox" data-kn-form="commit"${x.commit ? ' checked' : ''}><span><b>Commit what Echo saves</b> <span class="set-sub">— off: Echo writes the files and you commit. On: one commit per note, only Echo's files, never a push.</span></span></label>` : ''}
        ${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}`
      foot = `<span class="set-sub grow">You can change all of this later.</span><button class="btn ghost" data-kn="step" data-step="2">Back</button><button class="btn primary" data-kn="connect-go"${s.busy.connect ? ' disabled' : ''}>${s.busy.connect ? 'Connecting…' : 'Connect'}</button>`
    }
    return sheet('Connect a folder', body, foot, IC.folder, stepsHtml)
  }
  function newSheet() {
    const s = st(), x = s.sheet, d = st().data || {}
    // As the app names the folder (recorder::slugify): letters and digits kept, the rest a dash.
    const slug = String(x.name || 'my-twin').toLowerCase().replace(/[^\p{Alphabetic}\p{N}]+/gu, '-').replace(/^-+|-+$/g, '').slice(0, 60) || 'vault'
    const folder = x.picked ? x.picked.path : `${d.home || '~/echo/knowledge'}/${slug}`
    const tree = `my-vault/
├── AGENTS.md · CLAUDE.md      <em>the rules: structure, page format, index, tags</em>
├── README.md                  <em>getting started</em>
├── log.md                     <em>what changed, one line each</em>
├── raw_inbox/                 <em>sources, never edited</em>
├── .claude/skills/
│   ├── twin/                  <em>/twin — find, answer, remember</em>
│   └── process-inbox/         <em>/process-inbox — sources → pages</em>
└── knowledge/                 <em>the pages · Obsidian opens this</em>
    ├── CATALOG.md             <em>the index, a table per section</em>
    ├── TAG_TAXONOMY.md · TERMS_TAXONOMY.md
    ├── profile/DIGITAL_PROFILE.md   <em>who you are — yours to fill</em>
    ├── topics/ clients/ activities/ notes/ media/ action-items/
    └── templates/             <em>meeting note, page</em>`.replace('my-vault', esc(x.picked ? (x.picked.name || 'vault') : slug))
    const noGit = !d.git
    const body = `<div class="kn-field"><span>Name</span><input class="search kn-in" id="kn-name" value="${esc(x.name || '')}" placeholder="My twin" aria-label="The vault's name"></div>
      <div class="kn-field"><span>Folder</span><div class="kn-inline"><span class="kn-path" title="${esc(folder)}"><code><bdi>${esc(short(folder))}</bdi></code></span><button class="btn" data-kn="pick-new"${x.picking ? ' disabled' : ''}>${x.picking ? 'Choosing…' : 'Choose…'}</button>${x.picked ? '<button class="btn ghost sm" data-kn="pick-default">Echo\'s data folder</button>' : ''}</div></div>
      <p class="set-sub kn-indent">${x.picked ? (x.picked.refused ? `<span class="warn">${esc(x.picked.refused)}</span>` : x.picked.empty ? 'An empty folder — the vault goes in it.' : '<span class="warn">This folder isn\'t empty — choose a new or empty one.</span>') : 'In Echo\'s data folder unless you choose another. It must be empty or new.'}</p>
      <div><div class="kn-lbl">What Echo puts in it</div><pre class="kn-tree-pre">${tree}</pre></div>
      <label class="kn-check"><input type="checkbox" data-kn-form="git"${x.git && !noGit ? ' checked' : ''}${noGit ? ' disabled' : ''}><span><b>Make it a git repository</b> <span class="set-sub">${noGit ? '— git isn\'t installed; a plain folder works the same, without history.' : '— one first commit, and Echo commits what it saves here (a switch in Settings). Add a remote later to back it up.'}</span></span></label>
      <label class="kn-check"><input type="checkbox" data-kn-form="auto"${x.auto ? ' checked' : ''}><span><b>Save my meeting minutes here automatically</b></span></label>
      <p class="note-line">${IC.info}<span>Your name, from Settings → About me, goes into the profile and the README. Nothing else is yours yet.</span></p>
      ${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}`
    const blocked = x.picked && (!x.picked.empty || x.picked.refused)
    const foot = `<span class="set-sub grow">23 files in a new folder.</span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="new-go"${blocked || s.busy.new ? ' disabled' : ''}>${s.busy.new ? 'Creating…' : 'Create'}</button>`
    return sheet('Start a new vault', body, foot, IC.plus)
  }
  function newPageSheet() {
    const s = st(), x = s.sheet, t = s.tree || { pages: [], root: '' }
    const folders = [...new Set(t.pages.map((p) => p.path.split('/').slice(0, -1).join('/')))].sort()
    const root = t.root || ''
    if (!folders.includes(root)) folders.unshift(root)
    const templates = x.templates || []
    const body = `<div class="kn-field"><span>Name</span><input class="search kn-in" id="kn-pname" value="${esc(x.name || '')}" placeholder="Pricing model" aria-label="The page's name"></div>
      <div class="kn-field"><span>In</span><select class="set-in" data-kn-form="folder" aria-label="Folder">${folders.map((f) => `<option value="${esc(f)}"${f === x.folder ? ' selected' : ''}>${esc(f || '(the vault)')}</option>`).join('')}</select></div>
      ${templates.length ? `<div class="kn-field"><span>From</span><select class="set-in" data-kn-form="template" aria-label="Template"><option value="">The vault's page format</option>${templates.map((tp) => `<option value="${esc(tp)}"${tp === x.template ? ' selected' : ''}>${esc(tp.split('/').pop())}</option>`).join('')}</select></div>` : ''}
      <label class="kn-check"><input type="checkbox" data-kn-form="row"${x.row ? ' checked' : ''}><span><b>Add a row to the index</b> <span class="set-sub">— in the table for its folder, when the index has one.</span></span></label>
      ${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}`
    const foot = `<span class="set-sub grow">It opens in the editor.</span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="newpage-go"${s.busy.newpage ? ' disabled' : ''}>Create page</button>`
    return sheet('New page', body, foot, IC.file)
  }
  function rowsSheet() {
    const s = st(), x = s.sheet
    const body = !x.rows ? '<div class="skeleton"><i></i><i></i></div>' : `<p class="set-sub">Each row goes in the table of its folder's section in the index${vault() && vault().commit ? ', in one commit' : ''}. Nothing else changes.</p><div class="kn-rows">${x.rows.map((r) => `<div class="kn-rowp"><div class="kn-rowp-h"><code>${esc(r.path.replace(/^[^/]*\//, ''))}</code><span class="tag">${esc(r.section || '—')}</span></div><div class="kn-rowp-b" title="${esc(r.row || '')}"><span>${esc(r.desc || '')}</span><span class="set-sub">${esc(r.date || '')}</span></div></div>`).join('')}</div>${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}`
    const ok = x.rows ? x.rows.filter((r) => !/^skipped|already/.test(r.section)).length : 0
    return sheet('Add to the index', body, `<span class="set-sub grow">${x.rows ? `${ok} of ${x.rows.length} will be added` : ''}</span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="rows-go"${!ok || s.busy.rows ? ' disabled' : ''}>Add ${ok} row${ok === 1 ? '' : 's'}</button>`, IC.index)
  }
  function removeSheet() {
    const v = vault()
    return sheet(`Remove ${esc(v.name)} from Echo?`, `<p>Echo forgets this vault and its own index of it. <b>The folder and every file in it stay as they are</b> — connect it again any time.</p><p class="set-sub"><code>${esc(v.root)}</code></p>`, `<span class="grow"></span><button class="btn ghost" data-kn="sheet-close">Keep it</button><button class="btn danger" data-kn="remove-go">Remove from Echo</button>`, IC.gem)
  }
  function twinSheet() {
    const s = st(), x = s.sheet, v = vault()
    const t = x.text
    const name = esc(x.name || (t && t.name) || '')
    const body = `<div class="kn-field"><span>Name</span><span class="kn-inline"><code class="kn-dim">~/.claude/skills/</code><input class="search kn-in sm" id="kn-twin-name" value="${name}" aria-label="The skill's name" spellcheck="false"><code class="kn-dim">/SKILL.md</code></span></div>
      <p class="set-sub">${t && t.own ? 'This vault\'s own <code>twin</code> skill' : 'Echo\'s starter <code>twin</code> skill'}, its paths made absolute to the vault, so <code>/${name}</code> works in any folder. Read it before you install it: every Claude Code session of yours will be able to use it.</p>
      ${!t ? '<div class="skeleton"><i></i><i></i></div>' : `<pre class="kn-rowpre kn-twin-text" tabindex="0" aria-label="The file Echo writes">${esc(t.text)}</pre>`}
      ${v && v.profile !== 'system' ? `<div class="kn-warn">${IC.warn}<span><b>This vault is read on ${esc(profName(v.profile))}.</b> From your own Claude Code the skill reads it with that Claude Code's own account and settings, not this profile's.</span></div>` : ''}
      <p class="set-sub">It goes into your own Claude Code (<code>~/.claude</code>); sessions on other AI profiles don't see it. Remove takes out only Echo's copy.</p>${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}`
    return sheet('Use /twin from any Claude Code session', body, `<span class="grow"></span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="twin-global"${t ? '' : ' disabled'}>Install</button>`, IC.ask)
  }
  async function loadTwinText() {
    const s = st(), x = s.sheet, v = vault()
    if (!x || x.kind !== 'twin' || !v) return
    const want = x.name || ''
    try { const t = await get(`api/kn/${enc(v.id)}/twin-text?name=${enc(want)}`); if (st().sheet === x && (x.name || '') === want) { x.text = t; if (!x.name) x.name = t.name } } catch (err) { x.error = String(err.message || err) }
    render()
  }
  function sampleSheet() {
    const x = st().sheet, p = x.preview
    const body = p === undefined ? '<div class="skeleton"><i></i><i></i></div>'
      : !p ? '<p class="set-sub">No call with minutes on this vault\'s AI profile yet. After the next one, this shows how it lands here.</p>'
        : `<p class="set-sub">The newest call, <b>${esc(p.title)}</b>, as the vault's rule would save it. Nothing is written.</p>
      <div class="kn-field"><span>File</span><code>${esc(p.path)}</code></div>
      <div class="kn-field"><span>Index row</span>${p.row ? `<pre class="kn-rowpre">${esc(p.row)}</pre>` : '<span class="set-sub">none — the index has no table for notes</span>'}</div>
      <pre class="kn-rowpre kn-twin-text" tabindex="0" aria-label="The note">${esc(p.note || '')}</pre>`
    return sheet('How a call lands here', body, '<span class="grow"></span><button class="btn primary" data-kn="sheet-close">Close</button>', IC.gem)
  }
  function saveSheet() {
    const s = st(), x = s.sheet, pv = x.preview
    const vs = ((S.call && S.call.knowledge && S.call.knowledge.vaults) || []).filter((v) => v.available)
    const body = `<div class="kn-field"><span>Vault</span><select class="set-in" data-kn-form="svault" aria-label="Vault">${vs.map((v) => `<option value="${esc(v.id)}"${v.id === x.vault ? ' selected' : ''}>${esc(v.name)}</option>`).join('')}</select></div>
      ${!pv ? '<div class="skeleton"><i></i><i></i></div>' : `<div class="kn-field"><span>As</span><span class="kn-inline"><code class="kn-dim">${esc(pv.path.split('/').slice(0, -1).join('/'))}/</code><input class="search kn-in sm kn-stem" id="kn-stem" value="${esc(x.stem != null ? x.stem : pv.stem)}" aria-label="File name">.md</span></div>
      ${pv.exists && !x.stemEdited ? '<p class="set-sub kn-indent warn">A note with this name is there — it gets -2.</p>' : ''}
      <div class="kn-field"><span>Index row</span>${pv.row ? `<pre class="kn-rowpre">${esc(pv.row)}</pre>` : '<span class="set-sub">This vault has no index.</span>'}</div>
      ${pv.index ? `<div class="kn-field"><span></span><span class="set-sub">→ <code>${esc(pv.index)}</code></span></div>` : ''}
      <div class="kn-field"><span>Include</span><div class="kn-inline"><label class="kn-check"><input type="checkbox" checked disabled> Minutes</label><label class="kn-check"><input type="checkbox" data-kn-form="transcript"${x.transcript ? ' checked' : ''}> Transcript</label><label class="kn-check"><input type="checkbox" data-kn-form="chat"${x.chat ? ' checked' : ''}> Chat</label></div></div>
      ${pv.profile_ok ? '' : `<div class="kn-warn">${IC.warn}<span><b>This call's text goes only to ${esc(profName(pv.call_profile))}.</b> ${esc(vs.find((v) => v.id === x.vault) ? vs.find((v) => v.id === x.vault).name : 'This vault')} is read by <b>${esc(profName(pv.vault_profile))}</b> — saving it there lets that profile's sessions read these minutes.</span></div>`}
      <p class="set-sub">${pv.commit ? `Commits only this note and the index row${pv.branch ? ` on <code>${esc(pv.branch)}</code>` : ''}.` : 'Commits are off for this vault — you commit.'}</p>`}
      ${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}`
    const saved = ((S.call && S.call.knowledge && S.call.knowledge.record && S.call.knowledge.record.saved) || []).map((e) => e.vault)
    const foot = `<span class="set-sub grow">${saved.length ? `Already in ${esc(saved.map((id) => (vs.find((v) => v.id === id) || { name: id }).name).join(', '))}.` : 'Nothing is written before you save.'}</span><button class="btn ghost" data-kn="sheet-close">Cancel</button><button class="btn primary" data-kn="save-minutes-go"${!pv || s.busy.saveMinutes ? ' disabled' : ''}>${pv && !pv.profile_ok ? 'Save anyway' : 'Save'}</button>`
    return sheet('Save the minutes to Knowledge', body, foot, IC.gem)
  }
  const profName = (id) => { const p = typeof profileOf === 'function' && profileOf(id); return p ? p.name : (id || 'another profile') }
  function overlay() {
    const s = st()
    return sheetView() + (s.asking ? askSheet() : '')
  }
  function sheetView() {
    const s = st()
    if (!s.sheet) return ''
    switch (s.sheet.kind) {
      case 'connect': return connectSheet()
      case 'new': return newSheet()
      case 'newpage': return newPageSheet()
      case 'rows': return rowsSheet()
      case 'remove': return removeSheet()
      case 'twin': return twinSheet()
      case 'save': return saveSheet()
      case 'sample': return sampleSheet()
      case 'addto': return addToSheet()
      case 'fixall': return fixAllSheet()
    }
    return ''
  }

  // ── the view ──
  function view() {
    const s = st(), v = vault()
    if (!s.data) { load().then(render); return '<div class="kn"><div class="skeleton" style="padding:20px"><i></i><i></i><i></i></div></div>' }
    // No vault named in the route (Knowledge just opened): the one last open, else the first.
    if (!v && vaults().length) {
      let last = null
      try { last = localStorage.getItem('echo.kn.vault') } catch { /* off */ }
      const id = (vaults().find((x) => x.id === (s.lastVault || last)) || vaults()[0]).id
      s.vault = id
      s.source = 'vault'
      history.replaceState(null, '', `#${knRouteOf({ vault: id, tab: s.tab || 'pages', path: here() })}`)
      loadVault().then(render)
      return `<div class="kn">${head()}${tabs()}<div class="kn-body"><div class="skeleton" style="padding:20px"><i></i><i></i><i></i></div></div></div>`
    }
    if (!v) return `<div class="kn">${head()}<div class="kn-tabs kn-tabs-empty"></div><div class="kn-body">${emptyView()}</div></div>`
    if (v.available === false) return `<div class="kn">${head()}<div class="kn-tabs kn-tabs-empty"></div><div class="kn-body">${missingView(v)}</div></div>`
    const body = s.tab === 'graph' ? graphView() : s.tab === 'settings' ? settingsView() : s.tab === 'ask' ? chatView() : s.tab === 'health' ? healthView() : pagesView()
    return `<div class="kn">${head()}${tabs()}<div class="kn-body">${body}</div></div>`
  }
  function after() {
    const s = st()
    if (s.tab === 'graph') drawGraph()
    if (s.tab === 'pages' && s.rail === 'links') drawMini()
    if (s.tab === 'ask' && vault()) { syncThread(); fitQ(); pollChat() }
    if (s.scrollTo) { const el = document.getElementById(s.scrollTo); if (el) el.scrollIntoView({ block: 'start' }); s.scrollTo = null }
    if (s.focus) {
      const el = document.getElementById(s.focus)
      if (el) { el.focus(); if (el.select && s.focusSelect) el.select(); else if (el.id === 'kn-chat-q') el.setSelectionRange(el.value.length, el.value.length) }
      s.focus = null; s.focusSelect = false
    }
  }

  // ── actions ──
  function openPage(path) {
    const s = st()
    const go = () => { s.edit = null; s.ver = null; s.menu = null; goVault(s.vault, 'pages', path) }
    if (s.edit && s.edit.dirty) return askFirst('Leave this page?', 'Your text isn\'t saved. It stays in this window: Edit on this page brings it back.', 'Leave', go)
    go()
  }
  async function startEdit() {
    const s = st(), p = s.page, v = vault()
    if (!p || !p.editable) return
    const draft = store() && knRestoreDraft(store(), v.id, p.path, p.base)
    s.edit = { path: p.path, base: p.base, text: draft != null ? draft : (p.text || ''), dirty: draft != null, preview: p.html, message: `Edit ${p.path}` }
    s.focus = 'kn-edit'
    render()
  }
  let previewTimer = null
  function onEditInput(text) {
    const s = st(), v = vault()
    if (!s.edit) return
    s.edit.text = text
    s.edit.dirty = true
    if (store()) knKeepDraft(store(), v.id, s.edit.path, s.edit.base, text)
    clearTimeout(previewTimer)
    previewTimer = setTimeout(async () => {
      const e = st().edit
      if (!e) return
      try { const r = await run('kn_preview', { id: v.id, path: e.path, text: e.text }); if (st().edit === e) { e.preview = r.html; const pane = document.querySelector('.kn-preview'); if (pane) pane.innerHTML = r.html } } catch { /* the preview waits */ }
    }, 600)
    const crumbs = document.querySelector('.kn-editing .kn-crumbs')
    if (crumbs && !crumbs.textContent.includes('not saved')) crumbs.insertAdjacentText('beforeend', ' · not saved')
  }
  async function save(force) {
    const s = st(), v = vault(), e = s.edit
    if (!e) return
    const msg = document.getElementById('kn-msg')
    if (msg) e.message = msg.value
    const base = force && e.conflict ? e.conflict.base : e.base
    try {
      const r = await run('kn_save_page', { id: v.id, path: e.path, text: e.text, base, message: e.message }, 'save')
      if (store()) knDropDraft(store(), v.id, e.path)
      s.edit = null
      s.page = null
      await loadPage()
      await loadTree()
      // Why a commit was skipped is said once per vault and reason (X-G4); after that the save is just saved.
      const why = /^skipped/.test(r.commit || '') ? r.commit.replace(/^skipped: /, '') : null
      const told = (s.toldSkip = s.toldSkip || new Set()), key = `${v.id}:${why}`
      if (why && !told.has(key)) { told.add(key); say(`Saved. Not committed: ${why}.`) } else done(why ? 'Saved, not committed.' : r.commit && r.commit !== 'off' ? `Saved and committed (${r.commit}).` : 'Saved.')
    } catch (err) {
      const m = String(err.message || err)
      if (/changed_on_disk/.test(m)) {
        try { const p = await get(`api/kn/${enc(v.id)}/page?path=${enc(e.path)}`); e.conflict = { base: p.base, text: p.text, mtime: p.mtime } } catch { /* gone */ }
        render()
      } else say(`Not saved: ${m}`)
    }
  }
  async function openConnect() {
    const s = st()
    s.switcher = false
    s.sheet = { kind: 'connect', step: 1, picked: null, minutes: 'auto', commit: false, name: '' }
    render()
  }
  async function doPick(forNew) {
    const s = st(), x = s.sheet
    if (!x) return
    x.picking = true
    render()
    const p = await pick(forNew ? 'new' : 'connect')
    x.picking = false
    if (p) { x.picked = p; if (!forNew) { x.name = p.name || ''; x.error = null } }
    render()
  }
  async function connectGo() {
    const s = st(), x = s.sheet
    const name = (document.getElementById('kn-name') || {}).value || x.name
    try {
      const r = await run('kn_add', { pick: x.picked.seq, name, minutes: x.minutes, commit: !!x.commit, profile: x.profile || '' }, 'connect')
      s.sheet = null
      await refreshAll()
      goVault(r.vault.id, 'pages')
      done(`${r.vault.name} is connected — nothing in its folder changed.`)
    } catch (err) { x.error = String(err.message || err); render() }
  }
  async function newGo() {
    const s = st(), x = s.sheet
    const name = ((document.getElementById('kn-name') || {}).value || '').trim()
    if (!name) { x.error = 'A vault needs a name.'; s.focus = 'kn-name'; return render() }
    try {
      const r = await run('kn_new', { name, pick: x.picked ? x.picked.seq : undefined, git: !!x.git, minutes: x.auto ? 'auto' : 'ask' }, 'new')
      s.sheet = null
      await refreshAll()
      goVault(r.vault.id, 'pages')
      done(r.git === 'committed' ? `${r.vault.name} is ready, in git.` : r.git === 'no identity' ? `${r.vault.name} is ready. git has no name and email set, so the first commit wasn't made.` : `${r.vault.name} is ready.`)
    } catch (err) { x.error = String(err.message || err); render() }
  }
  /** K6-02's sheet; `answer` (`{ md, source }`): an answer from Ask, put into the new page's editor (K13-06). */
  async function openNewPage(name, answer) {
    const s = st(), v = vault()
    // From Ask the tree may not be loaded yet: the sheet lists its folders.
    if (!s.tree || s.tree.vault !== v.id) await loadTree()
    const folder = s.path ? s.path.split('/').slice(0, -1).join('/') : (s.tree && s.tree.root) || ''
    s.sheet = { kind: 'newpage', name: name || '', folder: name ? (s.tree && s.tree.root) || folder : folder, template: '', row: true, templates: await listTemplates(v), answer }
    s.focus = 'kn-pname'
    render()
  }
  /** The vault's page templates (the pages tree hides their folder). */
  async function listTemplates(v) {
    if (!v.paths || !v.paths.templates) return []
    try { return (await get(`api/kn/${enc(v.id)}/settings`)).templates || [] } catch { return [] }
  }
  async function newPageGo() {
    const s = st(), x = s.sheet, v = vault()
    const name = ((document.getElementById('kn-pname') || {}).value || '').trim()
    if (!name) { x.error = 'A page needs a name.'; s.focus = 'kn-pname'; return render() }
    try {
      const r = await run('kn_new_page', { id: v.id, folder: x.folder, name, template: x.template || null, row: !!x.row }, 'newpage')
      s.sheet = null
      await loadTree()
      s.path = r.path
      s.page = null
      goVault(v.id, 'pages', r.path)
      await loadPage()
      await startEdit()
      // An answer from Ask: in the editor with its source, saved when the user saves.
      if (x.answer && s.edit) { onEditInput(knAnswerInto(s.edit.text, x.answer.md, { source: x.answer.source })); render() }
    } catch (err) { x.error = String(err.message || err); render() }
  }
  async function openRows(paths) {
    const s = st(), v = vault()
    s.sheet = { kind: 'rows', paths, rows: null }
    render()
    try { const r = await get(`api/kn/${enc(v.id)}/rows?paths=${enc(JSON.stringify(paths))}`); if (r.error) throw new Error(r.error); s.sheet.rows = r.rows } catch (e) { s.sheet.error = String(e.message || e); s.sheet.rows = [] }
    render()
  }
  async function rowsGo() {
    const s = st(), x = s.sheet, v = vault()
    try {
      const r = await run('kn_index_add', { id: v.id, paths: x.paths }, 'rows')
      s.sheet = null
      await loadTree()
      if (s.tab === 'settings') await loadSettings()
      if (s.path) await loadPage()
      done(`${r.added} row${r.added === 1 ? '' : 's'} added to the index${r.commit && !/^(off|skipped)/.test(r.commit) ? ' and committed' : ''}.`)
    } catch (e) { x.error = String(e.message || e); render() }
  }
  async function setVault(key, value) {
    const v = vault()
    try {
      const r = await run('kn_set', { id: v.id, key, value })
      const i = st().data.vaults.findIndex((x) => x.id === v.id)
      if (i >= 0) st().data.vaults[i] = r.vault
      if (st().tab === 'settings') await loadSettings()
      render()
    } catch (e) { say(String(e.message || e)) }
  }
  async function ask(prompt) {
    const s = st(), v = vault()
    if (!v || typeof claudePanel !== 'function') return
    const key = `claude:vault:${v.id}`
    const e = typeof Terms !== 'undefined' && Terms.get(key), running = !!(e && e.id != null && !e.exited)
    claudePanel('claude-vault')
    if (!prompt) return
    prompt = knPlainPrompt(prompt)
    if (running) { send({ cmd: 'pty_input', id: e.id, data: `${prompt}\r` }); return }
    s.queued = { key, prompt, at: Date.now() }
    if (typeof termStart === 'function') setTimeout(() => { const t = Terms.get(key); if (!t || (t.id == null && !t.starting)) termStart('claude-vault', key, { vault: v.id }) }, 120)
  }
  /** A prompt queued for a vault session that was starting: typed once Claude has drawn its screen. */
  function feedQueued() {
    const s = st(), q = s.queued
    if (!q || typeof Terms === 'undefined') return
    const e = Terms.get(q.key)
    if (e && e.id != null && !e.exited && e.from > 400 && Date.now() - q.at > 2500) { send({ cmd: 'pty_input', id: e.id, data: `${q.prompt}\r` }); s.queued = null }
    else if (Date.now() - q.at > 60000) s.queued = null
  }

  // ── Ask: a conversation with Claude Code in the vault (spec K13) ──
  // The thread is drawn apart from the rest of the view: `view()` leaves it empty and `syncThread` fills it block by
  // block, replacing only a block whose HTML changed (K13-10: a streaming block re-renders alone). The other parts that
  // change with the stream (the header, the rail, the composer's bar) are patched by `refreshChat`, each by the same
  // function `view()` draws it with. Text and thinking are others' words: `esc()`, or the HTML Echo's writer made.
  const MOD_KEY = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent || '') ? '⌘↵' : 'Ctrl+↵'
  const MODES = [['ask', 'Ask'], ['curate', 'Curate'], ['research', 'Research']]
  const MODE_LINE = {
    ask: [IC.lock, 'Ask reads your pages. It changes nothing.'],
    curate: [IC.edit, 'Curate reads, and edits pages after you allow each edit.'],
    research: [IC.warn, 'Research also searches the web. Searches and pages it opens may receive words from your pages.'],
  }
  const TOOL_IC = { Read: IC.file, Grep: IC.search, Glob: IC.search, Skill: IC.bolt, Edit: IC.edit, Write: IC.edit, WebSearch: IC.globe, WebFetch: IC.globe, Agent: IC.agent, Task: IC.agent }
  const SPIN = '<span class="kn-spin" aria-hidden="true"></span>'
  const researchOn = () => !(vault() && vault().ask && vault().ask.research === false)
  const askShown = () => S.view === 'brain' && st().source === 'vault' && st().tab === 'ask' && !!vault()
  const kTok = (n) => (n >= 1000 ? `${(n / 1000).toFixed(1)}K` : String(n || 0))
  const usd = (c) => (c < 0.01 ? '<$0.01' : `$${c.toFixed(2)}`)
  const patchEl = (id, html) => { const el = document.getElementById(id); if (el) patch(el, html) }
  /** The conversations list: as the user left it, else open when the window is wide enough. */
  const listOpen = () => { const o = chat().listOpen; return o === null ? window.innerWidth >= 860 : o }
  /** Ask's own conversation in the open vault (D44: not the panel's current one) and everything the view keeps of it. */
  function chat() {
    const s = st()
    if (!s.chat || s.chat.vault !== s.vault) s.chat = { vault: s.vault, id: '', events: [], next: 0, live: false, busy: false, waiting: false, renders: {}, loaded: false, gone: null, mode: 'ask', draft: '', chips: [], picker: null, error: null, sending: false, pending: null, open: {}, always: {}, local: {}, whyOpen: null, follow: true, top: 0, jump: false, list: null, listQ: '', listOpen: null, command: null, polling: null, gen: 0 }
    return s.chat
  }
  /** Show conversation `id` ('' a new one): a new open (`knChatOpen`), so a loop polling the one before stops; the
   *  composer keeps what is typed in it. */
  function openChat(id) {
    const c = chat()
    if (c.id === (id || '')) return
    knChatOpen(c, id)
    Object.assign(c, { gone: null, error: null, open: {}, always: {}, local: {}, whyOpen: null, follow: true, top: 0, jump: false, command: null })
  }
  async function loadChats() {
    const c = chat(), v = vault()
    if (!v) return
    try { const r = await get(`api/kn/${enc(v.id)}/chats`); if (st().chat === c && Array.isArray(r)) { c.list = r; patchEl('kn-convs-l', convItems()); patchEl('kn-conv-h', convHead()) } } catch { /* the app restarting */ }
  }
  /** The conversation's events by long poll (K12-06) while Ask shows it: one loop per open (`gen`), each answer
   *  merged only into the open and the point it was asked for (`knChatTake`). */
  async function pollChat() {
    const c = chat(), v = vault()
    if (!c.id || c.gone || c.polling === c.gen) return
    const gen = c.polling = c.gen, id = c.id
    try {
      while (st().chat === c && knPollKeep(c, gen) && askShown()) {
        const from = c.next
        let r
        try { r = await get(`api/kn/${enc(v.id)}/chat/${enc(id)}?from=${from}`) } catch { await sleep(1500); continue }
        if (st().chat !== c || !knPollKeep(c, gen)) return
        if (r.error) { c.gone = /no such/.test(r.error) ? 'This conversation isn\'t one of this vault\'s any more.' : r.error; refreshChat(); return }
        tookChat(c, gen, from, r)
      }
    } finally { if (c.polling === gen) c.polling = null }
  }
  /** A chunk of events, merged once (`knChatTake`) and drawn. */
  function tookChat(c, gen, from, r) {
    const before = chatMeasure()
    const got = knChatTake(c, gen, from, r, Date.now())
    if (!got) return
    // The message sent from here: the composer empties once it lands; a sign-in error keeps it to send again.
    const landed = knSentLanded(c.pending, got.events)
    if (landed === 'landed') {
      const q = document.getElementById('kn-chat-q')
      if (q && q.value.trim() === c.pending.text) { q.value = ''; c.draft = ''; fitQ() }
      c.chips = []
    }
    if (landed) c.pending = null
    if (got.ended || got.sent) loadChats()
    // A message the user just sent is always shown, wherever the thread was scrolled to.
    refreshChat(got.events[got.events.length - 1] || null, got.sent && before ? { ...before, top: before.height - before.view } : before)
  }
  function turnsOf(c) {
    if (c._ev !== c.events || c._n !== c.events.length) { c._turns = knChatTurns(c.events); c._ev = c.events; c._n = c.events.length }
    return c._turns
  }
  /** The first card still waiting for the user (⌘↵ allows it, Esc denies it). */
  const firstWaiting = (c) => knWaitingCard(turnsOf(c), c.live, c.local)
  function chatMeasure() {
    const el = document.getElementById('kn-thread')
    return el ? { top: el.scrollTop, height: el.scrollHeight, view: el.clientHeight, jump: chat().jump } : null
  }
  function setJump(on) { const c = chat(); c.jump = !!on; const b = document.getElementById('kn-jump'); if (b) b.hidden = !on }
  function onThreadScroll(el) {
    const c = chat(), m = { top: el.scrollTop, height: el.scrollHeight, view: el.clientHeight, jump: c.jump }
    c.follow = knFollow(m, true).scroll
    c.top = el.scrollTop
    setJump(knFollow(m, null).jump)
  }
  /** What changed with the stream, drawn where it is — never the whole view (the composer keeps its text and caret). */
  function refreshChat(event = null, before = null) {
    if (!askShown() || !document.getElementById('kn-thread')) return
    const c = chat()
    patchEl('kn-conv-h', convHead())
    patchEl('kn-crail', railBody())
    patchEl('kn-convs-l', convItems())
    patchEl('kn-cmp-chips', chipsHtml())
    patchEl('kn-cmp-note', composerNote())
    patchEl('kn-cmp-f', composerFoot())
    drawPicker()
    const q = document.getElementById('kn-chat-q')
    if (q) q.placeholder = placeholder()
    syncThread(event, before, c)
  }
  /** The thread, block by block: a block whose HTML changed is replaced alone, new ones appended; it follows the
   *  bottom only while the user is there (K13-10). */
  function syncThread(event = null, before = null, c = chat()) {
    const box = document.getElementById('kn-thread-in'), sc = document.getElementById('kn-thread')
    if (!box || !sc) return
    const fresh = !box._parts, parts = threadParts(c), old = box._parts || []
    let i = 0
    for (; i < parts.length && i < old.length && old[i][0] === parts[i][0]; i++) {
      if (old[i][1] === parts[i][1]) continue
      const t = document.createElement('template')
      t.innerHTML = parts[i][1]
      box.children[i].replaceWith(t.content)
    }
    while (box.children.length > i) box.lastElementChild.remove()
    if (i < parts.length) box.insertAdjacentHTML('beforeend', parts.slice(i).map((x) => x[1]).join(''))
    box._parts = parts
    if (fresh) sc.scrollTop = c.follow ? sc.scrollHeight : c.top
    else if (before) { const f = knFollow(before, event); if (f.scroll) sc.scrollTop = sc.scrollHeight; setJump(f.jump) }
    tickRunning()
  }
  /** Seconds on the calls still running, counted here (the events carry no clock). */
  let runTimer = null
  function tickRunning() {
    const draw = () => { const now = Date.now(); for (const el of document.querySelectorAll('[data-kn-since]')) el.textContent = `${Math.max(0, Math.round((now - Number(el.dataset.knSince)) / 1000))} s` }
    draw()
    if (runTimer || !document.querySelector('[data-kn-since]')) return
    runTimer = setInterval(() => { if (!document.querySelector('[data-kn-since]')) { clearInterval(runTimer); runTimer = null } else draw() }, 1000)
  }
  function fitQ() { const q = document.getElementById('kn-chat-q'); if (q) { q.style.height = 'auto'; q.style.height = `${Math.min(q.scrollHeight, 220)}px` } }

  function chatView() {
    const c = chat(), narrow = window.innerWidth < 1100, open = listOpen()
    const list = open ? `<div class="kn-convs"><div class="kn-convs-h"><div class="kn-convs-row"><b>Conversations</b><button class="btn sm" data-kn="chat-new">${IC.plus} New</button></div><input class="search" id="kn-chat-find" placeholder="Find a conversation" aria-label="Find a conversation" value="${esc(c.listQ)}" autocomplete="off" spellcheck="false"></div><div class="kn-convs-l" id="kn-convs-l" data-keep-scroll>${convItems()}</div></div>` : ''
    const conv = `<div class="kn-conv"><div class="kn-conv-h" id="kn-conv-h">${convHead()}</div><div class="kn-thread" id="kn-thread"><div class="kn-thread-in" id="kn-thread-in"></div></div>${composerHtml()}</div>`
    return `<div class="kn-chat${open ? '' : ' no-list'}${narrow ? ' no-rail' : ''}">${list}${conv}${narrow ? '' : `<aside class="kn-crail" id="kn-crail" data-keep-scroll>${railBody()}</aside>`}</div>`
  }
  function whenOf(at) {
    if (!at) return ''
    const d = new Date(at)
    if (isNaN(d)) return ''
    const days = (Date.now() - d) / 864e5
    return d.toDateString() === new Date().toDateString() ? hm(d) : days < 6 ? d.toLocaleDateString([], { weekday: 'short' }) : d.toLocaleDateString([], { day: 'numeric', month: 'short' })
  }
  function convItems() {
    const c = chat()
    if (!c.list) return '<div class="skeleton" style="padding:12px"><i></i><i></i></div>'
    const q = c.listQ.trim().toLowerCase()
    const rows = c.list.filter((x) => !q || String(x.title || '').toLowerCase().includes(q))
    const today = (x) => x.at && new Date(x.at).toDateString() === new Date().toDateString()
    const item = (x) => `<button class="kn-cvi${x.id === c.id ? ' on' : ''}" data-kn="conv" data-id="${esc(x.id)}" title="${esc(x.title || '')}"><b>${esc(x.title || 'A conversation')}</b><span>${x.live ? '<i class="kn-live" aria-label="running"></i>' : ''}${x.busy ? 'answering · ' : ''}${esc(whenOf(x.at))}</span></button>`
    const fresh = !c.id ? `<button class="kn-cvi on" data-kn="chat-new"><b>New conversation</b><span>${esc((MODES.find((m) => m[0] === c.mode) || MODES[0])[1])}</span></button>` : ''
    const now = rows.filter(today), earlier = rows.filter((x) => !today(x))
    return `${now.length || fresh ? '<div class="kn-cvg">Today</div>' : ''}${fresh}${now.map(item).join('')}${earlier.length ? `<div class="kn-cvg">Earlier</div>${earlier.map(item).join('')}` : ''}${!rows.length && !fresh ? `<p class="set-sub kn-cv-none">${q ? 'None matches.' : 'No conversations yet.'}</p>` : ''}`
  }
  function convHead() {
    const s = st(), c = chat(), narrow = window.innerWidth < 1100
    const first = turnsOf(c).find((t) => t.user)
    const listed = (c.list || []).find((x) => x.id === c.id)
    const title = !c.id ? 'New conversation' : String((first && first.user.text) || (listed && listed.title) || 'A conversation').split('\n')[0].slice(0, 120)
    const mode = researchOn() || c.mode !== 'research' ? c.mode : 'ask'
    const tag = { ask: 'tag', curate: 'tag acc', research: 'tag warn' }[mode]
    const menu = narrow && c.id ? `<span class="kn-rel"><button class="icon-btn" data-kn="menu" data-menu="conv" aria-haspopup="menu" aria-expanded="${s.menu === 'conv'}" aria-label="Continue elsewhere">${IC.more}</button>${s.menu === 'conv' ? `<div class="menu kn-head-menu" role="menu">${continueItems(true)}</div>` : ''}</span>` : ''
    return `<button class="icon-btn" data-kn="convs-fold" aria-label="${listOpen() ? 'Hide' : 'Show'} the conversations" aria-expanded="${listOpen()}" title="Conversations">${IC.side}</button><b title="${esc(title)}">${esc(title)}</b><span class="${tag}">${esc((MODES.find((m) => m[0] === mode) || MODES[0])[1])}</span>${menu}`
  }
  const sysProfile = () => { const v = vault(), p = v && typeof profileOf === 'function' && profileOf(v.profile); return !!(p && p.kind === 'system') }
  /** Continue elsewhere (K13-08): the terminal panel; on Claude Code (as configured) also the command for a terminal. */
  function continueItems(menu) {
    const c = chat(), off = c.id ? '' : ' disabled'
    return menu ? `<button class="menu-item" role="menuitem" data-kn="chat-terminal"${off}>${IC.term} Open in the terminal panel</button>${sysProfile() ? `<button class="menu-item" role="menuitem" data-kn="chat-command"${off}>${IC.copy} Copy the terminal command</button>` : ''}`
      : `<div class="kn-acts"><button class="btn sm" data-kn="chat-terminal"${off}>${IC.term} Open in the terminal panel</button>${sysProfile() ? `<button class="btn ghost sm" data-kn="chat-command"${off}>${IC.copy} Copy the command</button>` : ''}</div>`
  }
  function railBody() {
    const c = chat(), v = vault(), turns = turnsOf(c)
    const root = String((v.paths || {}).pages || '')
    const shown = (p) => (root && p.startsWith(`${root}/`) ? p.slice(root.length + 1) : p)
    const mode = researchOn() || c.mode !== 'research' ? c.mode : 'ask'
    const modeName = (MODES.find((m) => m[0] === mode) || MODES[0])[1]
    const al = (k, t) => `<div class="kn-al ${k}">${k === 'y' ? IC.check : k === 'q' ? IC.asks : IC.no}<span>${t}</span></div>`
    const skills = (((v.detected || {}).skills) || []).filter((x) => x.kind !== 'agents').map((x) => `<code>${esc(x.name)}</code>`)
    const sk = skills.length ? al('y', `Its skills: ${skills.join(', ')}`) : ''
    const cards = turns.flatMap((t) => t.blocks.filter((b) => b.kind === 'permission' && b.answer && b.answer.allow && b.answer.always))
    const edits = cards.some((b) => b.tool !== 'WebFetch')
    const domains = [...new Set(cards.filter((b) => b.tool === 'WebFetch' && b.domain).map((b) => b.domain))]
    const read = al('y', 'Read and search inside the vault')
    const allowed = mode === 'curate' ? read + sk + al(edits ? 'y' : 'q', edits ? 'Edit and write pages — allowed for this conversation' : 'Edit and write pages — asks each time') + al('n', 'Never <code>CLAUDE.md</code>, <code>AGENTS.md</code>, <code>.claude/</code>') + al('n', 'No web, no commands')
      : mode === 'research' ? read + sk + al('y', 'Search the web') + al('q', 'Open a web page — asks per site') + domains.map((d) => al('y', `Open pages on <code>${esc(d)}</code> — allowed here`)).join('') + al('n', 'No edits, no commands')
        : read + sk + al('n', 'No edits, no web, no commands')
    const allowBox = `<div><h4>Allowed to <span>${esc(modeName)}</span></h4><div class="kn-allow">${allowed}</div></div>`
    const ai = `<div><h4>AI</h4><div class="set-sub">${esc(profName(v.profile))} — the vault's profile. Change it in Settings → Git &amp; AI.</div></div>`
    if (!c.id) return `<div><h4>This conversation</h4><div class="set-sub">Starts when you send. It is a Claude Code session: it also shows in <code>claude --resume</code> and in the terminal panel.</div></div>${allowBox}${ai}`
    const reads = new Map()
    for (const e of c.events) if (e.type === 'tool' && e.name === 'Read' && e.path) { const r = reads.get(e.path) || []; r.push(e.detail ? e.detail.replace(/^lines /, '') : 'all'); reads.set(e.path, r) }
    const readList = [...reads].map(([p, rs]) => {
      const r = rs.includes('all') ? 'all' : [...new Set(rs)].join(', ')
      return /\.md$/i.test(p) ? `<button class="kn-rf" data-kn="open" data-path="${esc(p)}" title="${esc(p)}"><b>${esc(shown(p))}</b><span>${esc(r)}</span></button>` : `<div class="kn-rf" title="${esc(p)}"><b>${esc(shown(p))}</b><span>${esc(r)}</span></div>`
    }).join('') || '<div class="set-sub">Nothing yet.</div>'
    const res = turns.map((t) => t.result).filter(Boolean)
    const sum = (k) => res.reduce((a, r) => a + (Number(r[k]) || 0), 0)
    const priced = res.some((r) => r.cost), last = res[res.length - 1]
    const cost = !res.length ? '<div class="set-sub">Nothing yet.</div>'
      : priced ? `<div class="kn-cost"><b>${esc(usd(sum('cost')))}</b><span>${kTok(sum('tokens_in'))} in · ${kTok(sum('tokens_out'))} out · ${many(res.length, 'turn')}</span></div>`
        : `<div class="kn-cost"><b>${kTok(sum('tokens_in'))} in · ${kTok(sum('tokens_out'))} out</b><span>${many(res.length, 'turn')}${last.cache_read ? ` · cache ${Math.round(last.cache_read * 100)} %` : ''}</span></div>`
    return `<div><h4>This conversation${c.live ? ' <span class="tag ok">live</span>' : ''}</h4><div class="set-sub">A Claude Code session in <code>${esc(short(v.root))}</code></div></div>
      <div><h4>Read so far <span>${reads.size}</span></h4>${readList}</div>${allowBox}<div><h4>Cost</h4>${cost}</div>
      <div><h4>Continue elsewhere</h4>${c.command ? `<div class="kn-cmdbox selectable">${esc(c.command)}</div>` : ''}${continueItems(false)}<div class="set-sub kn-cont-sub">${sysProfile() ? 'It stops here first: a conversation runs in one place at a time.' : 'This profile keeps its own settings, so it continues in Echo\'s terminal panel.'}</div></div>`
  }
  const placeholder = () => { const c = chat(); return firstWaiting(c) ? 'Waiting for your answer above' : c.busy ? 'Claude is answering — Esc stops it' : `Ask ${(vault() || {}).name || 'the vault'} — @ to mention a page` }
  function composerHtml() {
    const c = chat()
    return `<div class="kn-cmp-wrap"><button class="btn sm kn-jump" id="kn-jump" data-kn="jump"${c.jump ? '' : ' hidden'}>Jump to the latest ↓</button><div class="kn-cmp" id="kn-cmp"><div id="kn-picker" class="kn-picker-slot">${pickerHtml()}</div><div id="kn-cmp-note">${composerNote()}</div><div class="kn-cmp-chips" id="kn-cmp-chips">${chipsHtml()}</div>
      <textarea id="kn-chat-q" rows="1" placeholder="${esc(placeholder())}" aria-label="Your message" spellcheck="true">${esc(c.draft)}</textarea><div id="kn-cmp-f">${composerFoot()}</div></div></div>`
  }
  function composerNote() { const c = chat(); return c.error ? `<div class="kn-cmp-err" role="alert">${IC.warn}<span class="grow">${esc(c.error)}</span><button class="icon-btn" data-kn="chat-err-x" aria-label="Dismiss">${IC.close}</button></div>` : '' }
  function composerFoot() {
    const c = chat(), v = vault()
    const modes = MODES.filter(([id]) => id !== 'research' || researchOn())
    const mode = modes.some(([id]) => id === c.mode) ? c.mode : 'ask'
    const seg = `<div class="seg kn-modesw" role="radiogroup" aria-label="Mode">${modes.map(([id, l]) => `<button role="radio" aria-checked="${mode === id}" aria-pressed="${mode === id}" data-kn="mode" data-kn-mode="${id}">${l}</button>`).join('')}</div>`
    const p = typeof profileOf === 'function' && profileOf(v.profile)
    const model = p && (p.notes_model_label || p.notes_model)
    const go = c.busy ? `<button class="kn-send stop" data-kn="chat-stop" title="Stop (Esc)" aria-label="Stop">${IC.stop}</button>`
      : `<span class="kn-kbd">${MOD_KEY}</span><button class="kn-send" data-kn="chat-send" title="Send (${MOD_KEY})" aria-label="Send"${c.sending ? ' disabled' : ''}>${c.sending ? SPIN : IC.up}</button>`
    const [icon, line] = MODE_LINE[mode]
    return `<div class="kn-cmp-f"><button class="kn-iconish" data-kn="mention" title="Mention a page" aria-label="Mention a page">${IC.at}</button><button class="kn-iconish" data-kn="attach" title="Attach a call or memo" aria-label="Attach a call or memo">${IC.clip}</button>${seg}<span class="grow"></span><button class="kn-prof" data-kn="tab" data-kn-tab="settings" title="This vault's AI profile — in its Settings"><i>C</i>${esc(profName(v.profile))}${model ? ` · ${esc(model)}` : ''}</button>${go}</div><div class="kn-cmp-line${mode === 'research' ? ' warn' : ''}">${icon}<span>${line}</span></div>`
  }
  function chipsHtml() {
    return chat().chips.map((x, i) => {
      const rm = `<button class="kn-ctx-x" data-kn="chip-x" data-kn-chip="${i}" aria-label="Remove">${IC.close}</button>`
      if (x.page) return `<span class="kn-ctx" title="${esc(x.page)}">${IC.at}${esc(x.title || x.page.split('/').pop())}${rm}</span>`
      const why = x.mismatch ? `<span class="kn-ctx-why">${IC.warn}<span>This call's text goes only to ${esc(x.mismatch.call_name || 'its own profile')}; this vault is read by ${esc(x.mismatch.vault_name || 'another profile')}.</span><button class="btn sm" data-kn="chip-anyway" data-kn-chip="${i}">Attach anyway</button></span>` : ''
      return `<span class="kn-ctx call${x.mismatch ? ' warn' : ''}" title="${esc(x.title || x.call)}">${IC.call}${esc(x.title || x.call)}${x.anyway ? ' · anyway' : ''}${rm}</span>${why}`
    }).join('')
  }
  function pickerHtml() {
    const p = chat().picker
    if (!p) return ''
    if (p.kind === 'pages') {
      const hits = (p.hits || []).slice(0, 8)
      const rows = hits.map((h, i) => `<button class="kn-pk${i === p.sel ? ' on' : ''}" data-kn="pick-page" data-kn-chip="${i}" role="option" aria-selected="${i === p.sel}">${IC.file}<b>${esc(h.title || h.path.split('/').pop())}</b><span>${esc(h.path)}</span></button>`).join('')
      const none = !p.query ? '<div class="kn-pk-none">Type to find a page.</div>' : p.hits && !hits.length ? `<div class="kn-pk-none">No page matches “${esc(p.query)}”.</div>` : !p.hits ? '<div class="kn-pk-none">Finding…</div>' : ''
      return `<div class="kn-picker" role="listbox" aria-label="Pages"><div class="kn-menu-h">Pages${p.query ? ` · “${esc(p.query)}”` : ''}</div>${rows}${none}<div class="menu-sep" role="separator"></div><button class="kn-pk${p.sel === hits.length ? ' on' : ''}" data-kn="attach">${IC.call}<b>Attach a call or memo…</b><span>its minutes</span></button></div>`
    }
    return `<div class="kn-picker" role="listbox" aria-label="Calls and memos"><div class="kn-menu-h">A call or memo — its minutes go with the message</div><input class="search kn-pk-q" id="kn-attach-q" placeholder="Find a call or memo" aria-label="Find a call or memo" value="${esc(p.query)}" autocomplete="off" spellcheck="false"><div id="kn-pk-list">${callRows()}</div></div>`
  }
  function callRows() {
    const p = chat().picker, hits = (p.hits || []).slice(0, 10)
    const rows = hits.map((x, i) => `<button class="kn-pk${i === p.sel ? ' on' : ''}" data-kn="pick-call" data-kn-chip="${i}" role="option" aria-selected="${i === p.sel}">${x.memo ? IC.lock : IC.call}<b>${esc(x.title)}</b><span>${esc(x.started_at ? new Date(x.started_at).toLocaleDateString([], { day: 'numeric', month: 'short' }) : '')}${(x.people || []).length ? ` · ${esc(x.people.slice(0, 2).join(', '))}` : ''}</span></button>`).join('')
    return `${rows}${p.hits && !hits.length ? '<div class="kn-pk-none">No call with minutes matches.</div>' : !p.hits ? '<div class="kn-pk-none">Finding…</div>' : ''}`
  }
  /** The picker drawn again — the calls' list alone while its search field is there (it keeps its focus and text). */
  function drawPicker() {
    const p = chat().picker, list = document.getElementById('kn-pk-list')
    if (p && p.kind === 'calls' && list) return patch(list, callRows())
    patchEl('kn-picker', pickerHtml())
  }
  function heroHtml() {
    const c = chat(), v = vault()
    const mode = (id, name, what) => `<button class="kn-mode${c.mode === id ? ' on' : ''}" data-kn="mode" data-kn-mode="${id}" aria-pressed="${c.mode === id}"><b>${name}</b>${what}</button>`
    return `<div class="kn-hero"><span class="kn-av big">C</span><h2>Ask ${esc(v.name)}</h2><p>A Claude Code session in the vault's folder, with its skills and rules. Answers link the pages they used.</p>
      <div class="kn-modes${researchOn() ? '' : ' two'}">${mode('ask', 'Ask', 'Reads your pages. Changes nothing.')}${mode('curate', 'Curate', 'Edits pages, each edit after your yes.')}${researchOn() ? mode('research', 'Research', 'Also searches and opens websites.') : ''}</div></div>`
  }
  /** The thread as `[key, html]`, one element each, in order. */
  function threadParts(c) {
    if (c.gone) return [['gone', `<div class="kn-none"><p class="error">${esc(c.gone)}</p><button class="btn" data-kn="chat-new">New conversation</button></div>`]]
    if (!c.id) return [['hero', heroHtml()]]
    const turns = turnsOf(c), out = [], wait = firstWaiting(c)
    if (!turns.length) return [['load', '<div class="skeleton" style="padding:8px 0"><i></i><i></i><i></i></div>']]
    turns.forEach((t, i) => {
      const last = i === turns.length - 1, live = last && c.busy, ended = !!t.result || !live
      if (t.user) out.push([`u${i}`, msgHtml(t.user)])
      if (t.user && (t.blocks.length || live)) out.push([`w${i}`, '<div class="kn-who"><span class="kn-av">C</span><b>Claude</b></div>'])
      t.blocks.forEach((b, j) => out.push([`b${i}.${j}`, blockHtml(b, `${i}.${j}`, { ended, live, first: b === wait })]))
      if (live && !t.blocks.some((b) => b.kind !== 'notice')) out.push([`p${i}`, '<div class="kn-think live"><span class="kn-shim">Thinking…</span></div>'])
      if (t.result) out.push([`r${i}`, resultHtml(t.result)])
      if (t.result && knAnswerMd(t)) out.push([`a${i}`, actsHtml(t, i, last && !c.busy)])
    })
    return out
  }
  function msgHtml(u) {
    const ctx = (u.context || []).map((x) => x.page ? `<button class="kn-ctx" data-kn="open" data-path="${esc(x.page)}" title="${esc(x.page)}">${IC.at}${esc(x.page.split('/').pop())}</button>` : `<span class="kn-ctx call" title="${esc(x.date || '')}">${IC.call}${esc(x.title || x.call || 'A call')}</span>`).join('')
    return `<div class="kn-msg-u"><div class="kn-msg-t selectable">${esc(u.text)}</div>${ctx ? `<div class="kn-msg-meta">${ctx}</div>` : ''}</div>`
  }
  function blockHtml(b, key, o) {
    const c = chat()
    switch (b.kind) {
      case 'thinking': {
        const can = b.done && String(b.text || '').trim(), open = can && c.open[`t:${key}`]
        const label = b.done ? `Thought for ${b.seconds || 0} s` : '<span class="kn-shim">Thinking…</span>'
        return `<div class="kn-think-w"><button class="kn-think${b.done || !o.live ? '' : ' live'}" data-kn="think" data-id="${key}" aria-expanded="${!!open}"${can ? '' : ' disabled'}><span class="kn-car">${can ? (open ? '▾' : '▸') : ''}</span>${b.done || o.live ? label : 'Thinking stopped'}</button>${open ? `<div class="kn-think-body selectable">${esc(b.text)}</div>` : ''}</div>`
      }
      case 'text': return `<div class="kn-ans selectable${!b.done && o.live ? ' live' : ''}">${b.html || ''}</div>`
      case 'tools': return toolsHtml(b, o.ended)
      case 'permission': return cardHtml(b, o.first)
      case 'notice': {
        const icon = { committed: IC.git, 'not-committed': IC.warn, 'attached-anyway': IC.call, stopped: IC.stop }[b.what] || IC.info
        return `<div class="kn-notice${b.what === 'not-committed' ? ' warn' : ''}">${icon}<span>${esc(b.text)}</span></div>`
      }
      case 'error': {
        const act = b.action === 'signin' ? `<button class="btn primary sm" data-kn="signin" data-kn-profile="${esc(b.profile || vault().profile)}">Sign in…</button>` : b.action === 'terminal' ? `<button class="btn sm" data-kn="chat-terminal">${IC.term} Open in the terminal panel</button>` : ''
        return `<div class="kn-warn kn-chat-err" role="alert">${IC.warn}<span class="grow">${esc(b.text)}</span>${act}</div>`
      }
      case 'todo': return `<div class="kn-todo">${(b.items || []).map((x) => `<div class="${esc(x.state)}">${x.state === 'done' ? IC.check : x.state === 'active' ? SPIN : '<i></i>'}<span>${esc(x.text)}</span></div>`).join('')}</div>`
    }
    return '<div hidden></div>'
  }
  function toolsHtml(b, ended) {
    const c = chat()
    const secs = (ms) => `${(ms / 1000).toFixed(1)} s`
    const where = (w) => (w ? ` ${/^in /.test(w) ? `in <code>${esc(w.slice(3))}</code>` : esc(w)}` : '')
    const pm = (t) => esc(t).replace(/(\+\d+)/, '<span class="a">$1</span>').replace(/(−\d+)/, '<span class="d">$1</span>')
    const detail = (r) => `<div class="kn-tdet">${r.input && Object.keys(r.input).length ? `<div class="kn-tdet-h">Asked</div><pre>${esc(JSON.stringify(r.input, null, 2))}</pre>` : ''}${r.preview ? `<div class="kn-tdet-h">Result</div><pre>${esc(r.preview)}</pre>` : ''}${r.path && /\.md$/i.test(r.path) ? `<button class="btn ghost sm" data-kn="open" data-path="${esc(r.path)}">${IC.file} Open the page</button>` : ''}</div>`
    const one = (r, sub) => {
      const open = c.open[r.id]
      const icon = r.running ? SPIN : r.error ? IC.warn : TOOL_IC[r.name] || IC.bolt
      const target = r.target ? ` ${r.code ? `<code>${esc(r.target)}</code>` : esc(r.target)}` : ''
      const time = r.running && r.at ? `<span data-kn-since="${r.at}"></span>` : r.at && r.done ? secs(r.done - r.at) : ''
      return `<button class="kn-trow${sub ? ' sub' : ''}${r.running ? ' run' : ''}${r.error ? ' err' : ''}" data-kn="tool-open" data-id="${esc(r.id)}" aria-expanded="${!!open}"><span class="ti">${icon}</span><span class="tx"><span class="tv">${esc(r.verb)}</span>${target}${where(r.where)}${r.pm ? ` <span class="kn-pm">${pm(r.pm)}</span>` : ''}${r.note ? ` <span class="td">· ${esc(r.note)}</span>` : ''}</span><span class="tt">${time}</span></button>${open ? detail(r) : ''}`
    }
    const rows = knToolRows(b.items, { ended }).map((r) => {
      if (r.kind !== 'fold') return one(r)
      const open = c.open[`f:${r.id}`]
      return `<button class="kn-trow fold${r.running ? ' run' : ''}" data-kn="tool-open" data-id="f:${esc(r.id)}" aria-expanded="${!!open}"><span class="ti">${r.running ? SPIN : IC.files}</span><span class="tx"><span class="tv">${esc(r.text)}</span>${r.errors ? ` <span class="td err">· ${r.errors} failed</span>` : ''}</span><span class="tt"><span class="kn-car">${open ? '▾' : '▸'}</span></span></button>${open ? r.rows.map((x) => one(x, true)).join('') : ''}`
    }).join('')
    return `<div class="kn-tools">${rows}</div>`
  }
  /** A permission card (K13-04), inline where Claude asked; answered, one line. */
  function cardHtml(b, first) {
    const c = chat(), req = esc(b.request), path = b.path || (b.input && b.input.file_path) || ''
    const kbd = (k) => (first ? ` <span class="kn-kbd">${k}</span>` : '')
    // Answered from this page: shown as answered once Echo took it, before its `answered` comes; asked once.
    const local = c.local[b.request], a = b.answer || (local && !local.pending ? local : null), off = local ? ' disabled' : ''
    if (a) {
      const web = b.tool === 'WebFetch'
      const what = web ? (b.domain || b.url || 'a web page') : path || b.tool
      const how = !a.allow ? 'Denied' : !a.always ? 'Allowed' : web ? `Allowed ${b.domain || 'this site'} for this conversation` : 'Allowed — edits are allowed for the rest of this conversation'
      return `<div class="kn-perm-done ${a.allow ? 'ok' : 'no'}">${a.allow ? IC.check : IC.close}<span><b>${esc(how)}</b> · ${web ? 'open' : b.tool === 'Write' ? 'write' : b.tool === 'Edit' ? 'edit' : 'use'} <code>${esc(what)}</code></span></div>`
    }
    const why = c.whyOpen === b.request ? `<div class="kn-perm-why"><textarea id="kn-why" rows="2" placeholder="What Claude should do instead" aria-label="Tell Claude why"></textarea><button class="btn sm" data-kn="permit" data-kn-req="${req}" data-kn-how="deny">Deny and tell Claude</button></div>` : ''
    if (b.tool === 'WebFetch') {
      const url = String(b.url || (b.input && b.input.url) || '')
      const m = b.domain && /^(https?:\/\/)([^/?#]*)([\s\S]*)$/i.exec(url)
      const shown = m ? `${esc(m[1])}<b>${esc(m[2])}</b>${esc(m[3])}` : esc(url)
      return `<div class="kn-perm"><div class="kn-perm-h"><span class="pi">${IC.globe}</span><div><b>Claude wants to open a web page</b>${b.domain ? `<code>${esc(b.domain)}</code>` : ''}</div></div>
        <div class="kn-perm-url selectable">${shown}</div><div class="kn-perm-note">${IC.warn}<span>Words from your vault may be in this address. Echo shows it exactly as Claude asked for it.</span></div>
        <div class="kn-perm-f"><button class="btn primary sm" data-kn="permit" data-kn-req="${req}" data-kn-how="once"${off}>Allow once${kbd(MOD_KEY)}</button>${b.domain ? `<button class="btn sm" data-kn="permit" data-kn-req="${req}" data-kn-how="always"${off}>Allow ${esc(b.domain)} here</button>` : ''}<button class="btn sm" data-kn="permit" data-kn-req="${req}" data-kn-how="deny"${off}>Deny${kbd('esc')}</button><button class="btn ghost sm" data-kn="why" data-kn-req="${req}">Tell Claude why…</button></div>${why}</div>`
    }
    const edit = b.tool === 'Edit' || b.tool === 'Write'
    const pmh = edit && !b.error ? `<span class="kn-pm">${b.plus ? `<span class="a">+${b.plus}</span>` : ''}${b.plus && b.minus ? ' ' : ''}${b.minus ? `<span class="d">−${b.minus}</span>` : ''}</span>` : ''
    const diff = (b.hunks || []).map((h, k) => (k ? '<div class="hd">…</div>' : '') + h.map(([o, l]) => `<div class="${o === '+' ? 'add' : o === '-' ? 'del' : ''}">${o === '+' ? '+' : o === '-' ? '−' : ' '} ${esc(l)}</div>`).join('')).join('')
    const body = b.error ? `<div class="kn-perm-note">${IC.warn}<span>${esc(b.error)}. If you allow it, Claude Code's own tool says whether it worked.</span></div>` : diff ? `<div class="kn-diff kn-perm-diff">${diff}</div>` : ''
    const title = b.tool === 'Edit' ? 'Claude wants to edit a page' : b.tool === 'Write' ? 'Claude wants to write a page' : `Claude wants to use ${esc(b.tool)}`
    const always = edit ? `<span class="grow"></span><label class="kn-check"><input type="checkbox" data-kn-always="${req}"${c.always[b.request] ? ' checked' : ''}> Allow edits for the rest of this conversation</label>` : ''
    return `<div class="kn-perm"><div class="kn-perm-h"><span class="pi">${edit ? IC.edit : IC.asks}</span><div><b>${title}</b>${path ? `<code>${esc(path)}</code>` : ''}</div>${pmh}</div>${body}
      <div class="kn-perm-f"><button class="btn primary sm" data-kn="permit" data-kn-req="${req}" data-kn-how="once"${off}>Allow${kbd(MOD_KEY)}</button><button class="btn sm" data-kn="permit" data-kn-req="${req}" data-kn-how="deny"${off}>Deny${kbd('esc')}</button><button class="btn ghost sm" data-kn="why" data-kn-req="${req}">Tell Claude why…</button>${always}</div>${why}</div>`
  }
  function resultHtml(r) {
    return `<div class="kn-res">${r.seconds != null ? `<span>${IC.clock}${r.seconds} s</span>` : ''}<span>${kTok(r.tokens_in)} in · ${kTok(r.tokens_out)} out</span>${r.cache_read ? `<span>cache ${Math.round(r.cache_read * 100)} %</span>` : ''}${r.cost ? `<span>${esc(usd(r.cost))}</span>` : ''}</div>`
  }
  function actsHtml(t, i, last) {
    const sug = last && t.suggestion ? `<span class="grow"></span><button class="kn-sug" data-kn="ans-sug" data-kn-turn="${i}">${IC.ask}<span>${esc(t.suggestion)}</span></button>` : ''
    return `<div class="kn-acts-row"><button class="btn ghost sm" data-kn="ans-copy" data-kn-turn="${i}">${IC.copy} Copy</button><button class="btn ghost sm" data-kn="ans-save" data-kn-turn="${i}">${IC.save} Save as a page</button><button class="btn ghost sm" data-kn="ans-add" data-kn-turn="${i}">${IC.edit} Add to a page</button>${sug}</div>`
  }
  /** A turn's answer for Copy, Save as a page and Add to a page: its Markdown as Claude wrote it (ruling R33). */
  function answerOf(i) {
    const t = turnsOf(chat())[i]
    return t ? { md: knAnswerMd(t), question: t.user ? t.user.text : '' } : null
  }
  async function sendChat() {
    const s = st(), c = chat(), v = vault()
    const q = document.getElementById('kn-chat-q')
    if (q) c.draft = q.value
    const m = knChatMessage(c.draft, c.chips)
    // One message at a time: the one just sent hasn't come back yet.
    if (!m.text || c.sending || !v || (c.pending && Date.now() - c.pending.at < 30000)) return
    if (c.busy) { c.error = 'Wait for this answer, or stop it.'; return refreshChat() }
    if (c.mode === 'research' && !researchOn()) c.mode = 'ask'
    c.sending = true; c.error = null; c.picker = null
    // The composer empties when the message comes back as a `user` event — which, in a conversation already polled, can
    // come before this call's answer; a sign-in error keeps it (M2).
    c.pending = { text: m.text, at: Date.now() }
    refreshChat()
    try {
      const r = await run('kn_chat_send', { id: v.id, conversation: c.id || undefined, text: m.text, mode: c.mode, context: m.context })
      if (st().chat !== c) return
      // A call on another AI profile (X-P3): *Attach anyway* on that call's chip, then it is sent again.
      if (r && r.mismatch) { c.pending = null; const x = c.chips.find((ch) => ch.call && ch.call === r.mismatch.attached) || c.chips.find((ch) => ch.call && !ch.anyway); if (x) x.mismatch = r.mismatch; return }
      if (!c.id && r && r.conversation) {
        openChat(r.conversation)
        s.askConv = r.conversation
        history.replaceState(null, '', `#${knRouteOf({ vault: v.id, tab: 'ask', path: r.conversation })}`)
      }
      c.follow = true
      loadChats()
    } catch (e) { if (st().chat === c) { c.pending = null; c.error = String(e.message || e) } } finally {
      c.sending = false
      if (st().chat === c) { refreshChat(); pollChat() }
    }
  }
  async function stopChat() {
    const c = chat()
    if (!c.id || !c.busy) return
    try { await run('kn_chat_stop', { conversation: c.id }) } catch (e) { c.error = String(e.message || e); refreshChat() }
  }
  /** Answer card `request` — once: a second click or ⌘↵ while the answer goes, or before Echo's `answered` comes, does
   *  nothing (`c.local`). */
  async function permitChat(request, how) {
    const c = chat()
    if (!request || c.local[request]) return
    const why = how === 'deny' && c.whyOpen === request ? ((document.getElementById('kn-why') || {}).value || '').trim() : ''
    const always = how === 'always' || (how === 'once' && !!c.always[request])
    const answer = c.local[request] = { allow: how !== 'deny', always: how !== 'deny' && always, pending: true }
    refreshChat()
    try {
      await run('kn_chat_permit', { conversation: c.id, request, allow: answer.allow, always: answer.always, message: why || undefined }, `permit:${request}`)
      answer.pending = false
      if (c.whyOpen === request) c.whyOpen = null
    } catch (e) { delete c.local[request]; c.error = String(e.message || e) }
    refreshChat()
  }
  /** Continue in the terminal panel (K13-08): this process stops, the panel resumes the same conversation; one that read
   *  web pages or others' words says first what changes there (D41). */
  async function toTerminal() {
    const c = chat(), v = vault()
    st().menu = null
    if (!c.id) return ask()
    const go = async () => {
      try {
        const r = await run('kn_chat_terminal', { id: v.id, conversation: c.id }, 'terminal')
        if (typeof claudePanel === 'function') claudePanel('claude-vault')
        if (typeof termRestart === 'function') termRestart('claude-vault', `claude:vault:${v.id}`, { vault: v.id }, r.conversation)
        done('It continues in the terminal panel.')
      } catch (e) { say(String(e.message || e)) }
    }
    if (knChatReadOutside(c.events)) return askFirst('Open it in the terminal panel?', 'This conversation read web pages or others\' words. In the terminal it can run commands, with your usual prompts.', 'Open in the terminal panel', go)
    go()
  }
  /** The command that continues it in a terminal (Claude Code as configured): it stops here first. */
  async function copyCommand() {
    const c = chat(), v = vault()
    st().menu = null
    try {
      const r = await run('kn_chat_terminal', { id: v.id, conversation: c.id }, 'terminal')
      if (!r.command) return say('This profile continues only in Echo\'s terminal panel.')
      c.command = r.command
      try { await navigator.clipboard.writeText(r.command) } catch { /* not allowed: it shows in the rail */ }
      refreshChat()
      done('Copied. It stopped here — run it in a terminal.')
    } catch (e) { say(String(e.message || e)) }
  }
  /** Ask, with the composer focused (K10-03); `fill` puts something in it first (K13-11). `current`: Ask's conversation,
   *  or a new one when there is none (`false`: always a new one). */
  function askHere(fill, current = true) {
    const s = st(), v = vault()
    if (!v) return
    s.menu = null
    const c = chat()
    if (fill) fill(c)
    s.focus = 'kn-chat-q'
    goVault(v.id, 'ask', current ? c.id : '')
  }
  function onChatInput(t) {
    const c = chat()
    c.draft = t.value
    fitQ()
    const m = knMention(t.value, t.selectionStart)
    if (m) {
      const was = c.picker && c.picker.kind === 'pages' ? c.picker : null
      if (!was || was.query !== m.query) { c.picker = { kind: 'pages', query: m.query, start: m.start, sel: 0, hits: was ? was.hits : null }; findPages(m.query) } else was.start = m.start
    } else if (c.picker && c.picker.kind === 'pages') c.picker = null
    patchEl('kn-picker', pickerHtml())
  }
  let findTimer = null
  function findPages(q) {
    clearTimeout(findTimer)
    const c = chat(), v = vault()
    if (!q) { if (c.picker) c.picker.hits = []; return }
    findTimer = setTimeout(async () => {
      let r
      try { r = await get(`api/kn/${enc(v.id)}/find?q=${enc(q)}`) } catch { return }
      const p = chat().picker
      if (!p || p.kind !== 'pages' || p.query !== q) return
      const seen = new Set()
      p.hits = (r.hits || []).filter((h) => h.path && !seen.has(h.path) && seen.add(h.path))
      p.sel = 0
      patchEl('kn-picker', pickerHtml())
    }, 120)
  }
  function openAttach() {
    const c = chat()
    c.picker = { kind: 'calls', query: '', sel: 0, hits: null }
    patchEl('kn-picker', pickerHtml())
    findCalls('')
    const q = document.getElementById('kn-attach-q')
    if (q) q.focus()
  }
  let callTimer = null
  function findCalls(q) {
    clearTimeout(callTimer)
    callTimer = setTimeout(async () => {
      let r
      try { r = await get(`api/calls?limit=40&q=${enc(q)}`) } catch { return }
      const p = chat().picker
      if (!p || p.kind !== 'calls' || p.query !== q) return
      // Only calls with minutes: their minutes are what goes with the message (K12-10).
      p.hits = (r.calls || []).filter((x) => x.status === 'notes')
      p.sel = 0
      drawPicker()
    }, q ? 150 : 0)
  }
  function pickPage(i) {
    const c = chat(), p = c.picker, h = p && p.hits && p.hits[i]
    const q = document.getElementById('kn-chat-q')
    if (!h || !q) return
    const caret = q.selectionStart
    const m = knMention(q.value, caret) || { start: caret, query: '' }
    const title = h.title || h.path.split('/').pop().replace(/\.md$/i, '')
    const out = knMentionPick(q.value, m, caret, title)
    q.value = out.text
    q.setSelectionRange(out.caret, out.caret)
    c.draft = out.text
    if (!c.chips.some((x) => x.page === h.path)) c.chips.push({ page: h.path, title })
    c.picker = null
    patchEl('kn-picker', '')
    patchEl('kn-cmp-chips', chipsHtml())
    q.focus()
    fitQ()
  }
  function pickCall(i) {
    const c = chat(), p = c.picker, x = p && p.hits && p.hits[i]
    if (!x) return
    if (!c.chips.some((ch) => ch.call === x.name)) c.chips.push({ call: x.name, title: x.title, memo: !!x.memo })
    c.picker = null
    patchEl('kn-picker', '')
    patchEl('kn-cmp-chips', chipsHtml())
    const q = document.getElementById('kn-chat-q')
    if (q) q.focus()
  }
  /** Keys in Ask: the picker's ↑↓ ↵ Esc; ⌘↵ allows the waiting card or sends; Esc denies it or stops the turn. */
  function chatKey(e) {
    // An input method composing owns Enter, Esc and the arrows (M10).
    if (e.isComposing || e.keyCode === 229) return false
    const c = chat(), mod = e.metaKey || e.ctrlKey, id = e.target && e.target.id
    const p = c.picker
    if (p && (id === 'kn-chat-q' || id === 'kn-attach-q')) {
      const n = (p.hits || []).slice(0, p.kind === 'pages' ? 8 : 10).length + (p.kind === 'pages' ? 1 : 0)
      if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && n) { e.preventDefault(); p.sel = (p.sel + (e.key === 'ArrowDown' ? 1 : n - 1)) % n; drawPicker(); return true }
      if ((e.key === 'Enter' && !mod && !e.shiftKey) || (e.key === 'Tab' && n)) {
        e.preventDefault()
        if (p.kind === 'pages') { if (p.sel < (p.hits || []).slice(0, 8).length) pickPage(p.sel); else openAttach() } else pickCall(p.sel)
        return true
      }
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); c.picker = null; patchEl('kn-picker', ''); if (id === 'kn-attach-q') { const q = document.getElementById('kn-chat-q'); if (q) q.focus() } return true }
    }
    if (id === 'kn-why') {
      if (mod && e.key === 'Enter') { e.preventDefault(); permitChat(c.whyOpen, 'deny'); return true }
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); c.whyOpen = null; refreshChat(); return true }
      return false
    }
    const card = firstWaiting(c), q = document.getElementById('kn-chat-q')
    const typed = !!((q && q.value.trim()) || c.chips.length)
    const act = knChatKey({ key: e.key, mod, card: !!card, inComposer: id === 'kn-chat-q', typed, busy: c.busy })
    if (!act) return false
    e.preventDefault()
    if (e.key === 'Escape') e.stopPropagation()
    if (act === 'allow') permitChat(card.request, 'once')
    else if (act === 'deny') permitChat(card.request, 'deny')
    else if (act === 'send') sendChat()
    else if (act === 'stop') stopChat()
    // The "send" keystroke while a card waits answers nothing (item 2): it says what waits.
    else if (act === 'blocked') { c.error = 'Answer the card above first.'; refreshChat() }
    return true
  }
  function addToList() {
    const x = st().sheet
    return !x.q.trim() ? '<p class="set-sub">Type a page\'s title or path.</p>' : !x.hits ? '<div class="skeleton"><i></i><i></i></div>'
      : x.hits.slice(0, 12).map((h) => `<button class="kn-bl" data-kn="addto-page" data-path="${esc(h.path)}"><b>${IC.file} ${esc(h.title || h.path.split('/').pop())}</b><span>${esc(h.path)}</span></button>`).join('') || '<p class="set-sub">No page matches.</p>'
  }
  function addToSheet() {
    const x = st().sheet
    const body = `<input class="search kn-in" id="kn-addq" placeholder="Find a page" value="${esc(x.q)}" autocomplete="off" spellcheck="false" aria-label="Find a page"><div class="kn-addto-list" id="kn-addto-list">${addToList()}</div><p class="set-sub">The answer goes at the end, under a heading with today's date. Nothing is saved until you press Save.</p>`
    return sheet('Add the answer to a page', body, '<span class="grow"></span><button class="btn ghost" data-kn="sheet-close">Cancel</button>', IC.edit)
  }
  /** Add to a page (K13-06): that page's editor, the answer appended under a dated heading, saved by Save (K6-01). */
  async function addToPage(path, md) {
    const s = st(), v = vault()
    s.sheet = null
    if (s.edit && s.edit.dirty) return say('Save or cancel the page you are editing first.')
    s.edit = null
    s.path = path
    s.page = null
    goVault(v.id, 'pages', path)
    await loadPage()
    const p = s.page
    if (!p || p.error || !p.editable) return say(p && p.error ? p.error : 'This page can\'t be edited in Echo.')
    const d = new Date(), day = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    // From the page's unsaved text kept in this window, as Edit does, so the kept draft isn't written over (M1).
    const text = knEditText(store(), v.id, p.path, p.base, p.text || '')
    s.edit = { path, base: p.base, text, dirty: true, preview: p.html, message: `Add an answer from Ask to ${path}` }
    s.focus = 'kn-edit'
    onEditInput(knAnswerInto(text, md, { heading: `${day} — from Ask` }))
    render()
  }

  // ── the call page: the chip, the ⋯ menu's entries, the countdown's line, the save sheet ──
  function callChip(c) {
    const k = c && c.knowledge
    const x = knChipState(k)
    if (x.state === 'none' && !(k && k.vaults && k.vaults.length && st().chip === c.name)) return ''
    const s = st()
    const open = s.chip === c.name
    const cls = { will: 'will', saved: 'done', kept: 'kept', failed: 'kept', stale: 'will', off: 'off', removed: 'off' }[x.state] || 'will'
    const arrow = x.state === 'saved' ? ' ↗' : x.state === 'will' || x.state === 'off' ? ' ▾' : ''
    const menu = open ? chipMenu(c, x) : ''
    return `<span class="kn-rel"><button class="kn-chip ${cls}" data-kn="chip" data-kn-call="${esc(c.name)}" aria-haspopup="menu" aria-expanded="${open}" title="${esc(x.error || x.label || '')}">${x.state === 'kept' || x.state === 'failed' ? IC.warn : IC.gem} ${esc(x.label)}${arrow}</button>${menu}</span>`
  }
  function chipMenu(c, x) {
    const k = c.knowledge || {}
    const vs = (k.vaults || []).filter((v) => v.available)
    const targets = new Set((k.targets || []).map((v) => v.id))
    const choice = (k.record || {}).choice
    if (x.state === 'kept') {
      return `<div class="kn-pop" role="dialog"><h4>The minutes were written again — ${esc(x.label.replace(' · not updated', ''))} kept your version</h4><p>You changed <code>${esc((x.path || '').split('/').pop())}</code> after Echo saved it, so Echo didn't write over it.</p><div class="kn-acts"><button class="btn sm" data-kn="chip-open" data-vault="${esc(x.vault)}" data-path="${esc(x.path)}">Open the note</button><button class="btn sm" data-kn="save-as-new" data-kn-call="${esc(c.name)}" data-vault="${esc(x.vault)}">Save as a new note</button><button class="btn ghost sm" data-kn="save-replace" data-kn-call="${esc(c.name)}" data-vault="${esc(x.vault)}">Replace with the new minutes</button></div></div>`
    }
    if (x.state === 'failed') {
      return `<div class="kn-pop" role="dialog"><h4>The minutes weren't saved to ${esc(x.label.replace("Couldn't save to ", ''))}</h4><p>${esc(x.error || '')}</p><div class="kn-acts"><button class="btn sm primary" data-kn="save-again" data-kn-call="${esc(c.name)}" data-vault="${esc(x.vault)}">Save again</button></div></div>`
    }
    if (x.state === 'stale') {
      return `<div class="kn-pop" role="dialog"><h4>The minutes changed since you saved them</h4><p>Update the note in ${esc(x.label.replace('Update in ', ''))} with the new minutes? A note you changed is never written over.</p><div class="kn-acts"><button class="btn sm primary" data-kn="save-again" data-kn-call="${esc(c.name)}" data-vault="${esc(x.vault)}">Update</button></div></div>`
    }
    // A vault on another AI profile never takes this call by itself (its text stays on the call's profile); the
    // call's ⋯ → Save to Knowledge… can, after a warning.
    const items = vs.map((v) => k.profile && v.profile !== k.profile
      ? `<button class="menu-item kn-mi" role="menuitemradio" aria-checked="false" disabled title="This call's text stays on its own AI profile; ⋯ → Save to Knowledge… can save it there after a warning."><span class="kn-sp"></span><span class="grow">${esc(v.name)}</span><span class="kn-mi-r">another AI profile</span></button>`
      : `<button class="menu-item kn-mi${(choice ? choice === v.id : targets.has(v.id)) ? ' on' : ''}" role="menuitemradio" aria-checked="${choice ? choice === v.id : targets.has(v.id)}" data-kn="choose" data-kn-call="${esc(c.name)}" data-vault="${esc(v.id)}">${(choice ? choice === v.id : targets.has(v.id)) ? IC.check : '<span class="kn-sp"></span>'}<span class="grow">${esc(v.name)}</span><span class="kn-mi-r">${v.mode === 'auto' ? 'automatically' : v.mode === 'ask' ? 'when I ask' : 'never'}</span></button>`).join('')
    return `<div class="menu kn-chip-menu" role="menu"><div class="kn-menu-h">Save this call's minutes to</div>${items}<div class="menu-sep"></div><button class="menu-item kn-mi${choice === 'none' ? ' on' : ''}" role="menuitem" data-kn="choose" data-kn-call="${esc(c.name)}" data-vault="none">${choice === 'none' ? IC.check : '<span class="kn-sp"></span>'}Don't save this call</button>${choice ? `<button class="menu-item kn-mi" role="menuitem" data-kn="choose" data-kn-call="${esc(c.name)}" data-vault="">Back to the vaults' rules</button>` : ''}<div class="kn-menu-foot">Saved after the minutes are written — never the audio, the transcript or the chat unless the vault asks for them.</div></div>`
  }
  function callMenuItems(c) {
    const k = c && c.knowledge
    if (!k || !(k.vaults || []).length || !c.notes) return ''
    const x = knChipState(k)
    const item = (cmd, label, extra = '') => `<button class="menu-item" role="menuitem" data-kn="${cmd}" data-kn-call="${esc(c.name)}"${extra}>${label}</button>`
    const saved = (k.record && k.record.saved) || []
    const open = saved.length && x.vault ? item('chip-open', `Open in ${esc(((k.vaults || []).find((v) => v.id === saved[saved.length - 1].vault) || { name: 'Knowledge' }).name)}`, ` data-vault="${esc(saved[saved.length - 1].vault)}" data-path="${esc(saved[saved.length - 1].path)}"`) : ''
    return `<div class="menu-sep" role="separator"></div>${open}${item('save-sheet', saved.length ? 'Save to another vault…' : 'Save to Knowledge…')}`
  }
  function countdownLine(c) {
    const x = knChipState(c && c.knowledge)
    if (x.state !== 'will') return ''
    const names = x.label.replace('Minutes → ', '')
    return `<p class="kn-then">${IC.gem}<span>Then saved to <b>${esc(names)}</b>, with a row in the index.</span></p>`
  }
  async function openSave(callName, vaultId) {
    const s = st(), k = (S.call && S.call.knowledge) || {}
    const saved = new Set(((k.record || {}).saved || []).map((e) => e.vault))
    const vs = (k.vaults || []).filter((v) => v.available)
    const first = vaultId || (vs.find((v) => !saved.has(v.id)) || vs[0] || {}).id
    if (!first) return say('No vault to save into — connect one in Knowledge first.')
    s.chip = null
    s.sheet = { kind: 'save', call: callName, vault: first, preview: null, transcript: false, chat: false }
    render()
    await loadPreview()
  }
  async function loadPreview() {
    const s = st(), x = s.sheet
    if (!x || x.kind !== 'save') return
    try {
      const pv = await get(`api/kn/${enc(x.vault)}/preview?call=${enc(x.call)}&stem=${enc(x.stem || '')}`)
      if (pv.error) throw new Error(pv.error)
      x.preview = pv; x.transcript = pv.transcript; x.chat = pv.chat
    } catch (e) { x.error = String(e.message || e) }
    render()
  }
  async function saveMinutes(args, busy = 'saveMinutes') {
    try {
      const r = await run('kn_save_minutes', args, busy)
      if (r && r.mismatch) return r
      st().sheet = null
      st().chip = null
      if (typeof loadCall === 'function' && S.selected) await loadCall(S.selected)
      render()
      if (r && r.state === 'kept') say('You changed that note in the vault, so Echo kept your version.')
      else if (r && r.path) done(`Saved to ${r.path}${r.commit && !/^(off|skipped)/.test(r.commit) ? ', committed' : ''}.`)
      return r
    } catch (e) { if (st().sheet) { st().sheet.error = String(e.message || e); render() } else say(`Not saved: ${e.message || e}`) }
  }

  // ── Settings → Knowledge: the vaults, above Recall ──
  function settingsVaults() {
    const d = st().data
    if (!d) { load().then(render); return '' }
    const rows = d.vaults.map((v) => `<div class="set-row"><div class="kn-vrow"><span class="kn-gem">${IC.gem}</span><div><b>${esc(v.name)}${v.minutes && v.minutes.mode === 'auto' ? ' <span class="tag acc">★ minutes</span>' : ''}${v.available === false ? ' <span class="tag warn">folder not found</span>' : ''}</b><div class="set-sub kn-path-line" title="${esc(v.root)}"><code><bdi>${esc(short(v.root))}</bdi></code></div><div class="set-sub">${[v.pages != null ? many(v.pages, 'page') : '', v.git ? 'git' : '', v.minutes.mode === 'ask' ? 'minutes when I ask' : v.minutes.mode === 'off' ? 'no minutes' : '', esc(profName(v.profile))].filter(Boolean).join(' · ')}</div></div></div><div class="set-ctl"><button class="btn sm" data-kn="settings-open" data-id="${esc(v.id)}">Open</button></div></div>`).join('')
    const add = `<div class="set-row"><div><b>Add a vault</b><div class="set-sub">Connect a folder you already have, or start a new one from Echo's twin starter.</div></div><div class="set-ctl"><button class="btn sm" data-kn="connect">Connect a folder…</button><button class="btn sm" data-kn="new">Start a new vault…</button></div></div>`
    return `<div class="set-sec"><h3>Vaults</h3><div class="set-card">${rows}${add}</div><p class="set-foot">Each vault's structure, meeting minutes, skills and checks are in Knowledge → the vault → Settings.</p></div>`
  }
  function paletteActions(add) {
    add('Knowledge', 'Connect a folder…', () => { go(home()); openConnect() }, { keywords: 'vault obsidian knowledge second brain add' })
    add('Knowledge', 'Start a new vault…', () => { go(home()); openNew() }, { keywords: 'vault twin knowledge create' })
    if (vault()) add('Knowledge', `Find a page in ${vault().name}…`, () => { goVault(vault().id, 'pages', st().path); st().focus = 'kn-find'; render() }, { keywords: 'open page knowledge search' })
    if (vault() && vault().available !== false) add('Knowledge', `Ask ${vault().name}…`, () => { st().source = 'vault'; askHere(null, false) }, { keywords: 'claude question chat conversation knowledge' })
    if (vault() && vault().available !== false) add('Knowledge', 'Open Health', () => { st().source = 'vault'; goVault(vault().id, 'health', st().hlKey) }, { keywords: 'checks score broken links fix index tidy knowledge' })
    if (S.view === 'calls' && S.call && S.call.notes && S.call.knowledge && (S.call.knowledge.vaults || []).length) add('Knowledge', 'Save the minutes to Knowledge…', () => openSave(S.call.name), { keywords: 'vault note export' })
  }
  function openNew() {
    const d = st().data || {}
    st().switcher = false
    st().sheet = { kind: 'new', name: '', picked: null, git: !!d.git, auto: true }
    st().focus = 'kn-name'
    render()
  }
  /** The bottom panel's Claude · <vault> view (app.js's PANEL_VIEWS). */
  function askView() {
    const v = vault()
    if (!v || typeof termView !== 'function') return ''
    const key = `claude:vault:${v.id}`
    const saved = S.sessions && S.sessions.has(`${v.profile}:vault:${v.id}`)
    const intro = `<p>Claude Code in <b>${esc(v.name)}</b>'s folder — its skills and its rules for agents (<code>AGENTS.md</code>) come with it. Ask <code>/twin …</code>, or run <code>/process-inbox</code>. A real session: it continues in a terminal.</p>${st().resume ? `<p class="set-sub">In a terminal: <code>${esc(st().resume)}</code> <button class="btn ghost sm" data-copy="${esc(st().resume)}">Copy</button></p>` : ''}`
    return termView(key, 'claude-vault', saved ? 'Resume Claude' : 'Start Claude', intro, { vault: v.id })
  }
  const askLabel = () => (vault() ? `Claude · ${vault().name}` : 'Claude · vault')

  // ── events ──
  document.addEventListener('click', async (e) => {
    const s = st()
    // A link in a page: another page, a missing one to make, the web.
    const a = e.target.closest && e.target.closest('.kn-doc a, .kn-props a, .kn-ans a')
    if (a && !a.matches('[data-echo-link], a[href^="echo://"]')) {
      e.preventDefault()
      const href = a.getAttribute('href') || ''
      // Echo writes these encoded; a broken escape is taken as written.
      if (href.startsWith('#kn:')) return openPage(knDecode(href.slice(4)) ?? href.slice(4))
      if (href.startsWith('#kn-new:')) return openNewPage(knDecode(href.slice(8)) ?? href.slice(8))
      if (href.startsWith('#kn-name:')) {
        const name = (knDecode(href.slice(9)) ?? href.slice(9)).toLowerCase()
        const t = (s.tree || { pages: [] }).pages.find((p) => p.path.split('/').pop().replace(/\.md$/i, '').toLowerCase() === name || String(p.title).toLowerCase() === name)
        return t ? openPage(t.path) : openNewPage(knDecode(href.slice(9)) ?? href.slice(9))
      }
      if (/^https?:\/\//i.test(href)) return send({ cmd: 'open_url', url: href.replace(/^http:/i, 'https:') })
      return
    }
    const t = e.target.closest && e.target.closest('[data-kn]')
    // A click outside an open menu closes it.
    if (!t || !t.closest('.menu, .kn-pop')) {
      if (s.switcher && !(t && t.dataset.kn === 'switcher')) { s.switcher = false; if (!t) return render() }
      if (s.menu && !(t && t.dataset.kn === 'menu')) { s.menu = null; if (!t) return render() }
      if (s.chip && !(t && t.dataset.kn === 'chip')) { s.chip = null; if (!t) return render() }
    }
    if (!t) return
    const d = t.dataset
    const v = vault()
    switch (d.kn) {
      case 'switcher': s.switcher = !s.switcher; return render()
      case 'vault': s.switcher = false; s.source = 'vault'; return goVault(d.id, s.tab === 'settings' ? 'pages' : s.tab || 'pages')
      case 'recall': s.switcher = false; s.source = 'recall'; return go(`brain/${['sources', 'terms', 'people'].includes((S.br || {}).tab) ? S.br.tab : 'sources'}`)
      case 'plugin': s.switcher = false; s.source = 'plugin'; S.br.tab = d.knTab; return go(`brain/${d.knTab}`)
      case 'connect': return openConnect()
      case 'new': return openNew()
      case 'import': try { const r = await run('kn_import_plugin', {}, 'import'); await refreshAll(); done(`${r.connected} vault${r.connected === 1 ? '' : 's'} connected from the Obsidian plugin.`) } catch (err) { say(String(err.message || err)) } return
      case 'tab': e.preventDefault(); s.menu = null; return goVault(s.vault, d.knTab, d.knTab === 'pages' ? s.path : d.knTab === 'ask' ? chat().id : d.knTab === 'health' ? s.hlKey : '')
      case 'menu': s.menu = s.menu === d.menu ? null : d.menu; return render()
      case 'reveal': s.menu = null; send({ cmd: 'kn_reveal', id: s.vault, path: d.path || '' }); return render()
      case 'obsidian': s.menu = null; send({ cmd: 'kn_obsidian', id: s.vault, path: d.path || '' }); return render()
      case 'copy': s.menu = null; try { await navigator.clipboard.writeText(d.text) } catch { /* not allowed */ } return done('Copied.')
      case 'reread': try { await run('kn_reread', { id: s.vault }, 'reread'); await refreshAll(); done('Read again.') } catch (err) { say(String(err.message || err)) } return
      // Ask (K10-03, K13-11): the header's Ask is a new conversation; a page or a skill goes into Ask's composer.
      case 'ask': return askHere(null, false)
      case 'ask-panel': s.menu = null; render(); return ask()
      case 'ask-about': {
        const title = ((s.tree && s.tree.pages.find((p) => p.path === d.path)) || {}).title || d.path.split('/').pop().replace(/\.md$/i, '')
        return askHere((c) => { if (!c.chips.some((x) => x.page === d.path)) c.chips.push({ page: d.path, title }) })
      }
      case 'run-skill': return askHere((c) => { const cmd = `/${knPlainPrompt(d.name)} `; if (!c.draft.startsWith(cmd)) c.draft = cmd + c.draft })
      case 'chat-new': s.menu = null; return askHere(null, false)
      case 'conv': return goVault(s.vault, 'ask', d.id)
      case 'convs-fold': chat().listOpen = !listOpen(); return render()
      case 'mode': { chat().mode = d.knMode; refreshChat(); const q = document.getElementById('kn-chat-q'); if (q) q.focus(); return }
      case 'chat-send': return sendChat()
      case 'chat-stop': return stopChat()
      case 'chat-err-x': chat().error = null; return refreshChat()
      case 'permit': return permitChat(d.knReq, d.knHow)
      case 'why': { const c = chat(); c.whyOpen = c.whyOpen === d.knReq ? null : d.knReq; refreshChat(); const w = document.getElementById('kn-why'); if (w) w.focus(); return }
      case 'tool-open': { const c = chat(); c.open[d.id] = !c.open[d.id]; return syncThread() }
      case 'think': { const c = chat(); c.open[`t:${d.id}`] = !c.open[`t:${d.id}`]; return syncThread() }
      case 'jump': { const sc = document.getElementById('kn-thread'); if (sc) sc.scrollTop = sc.scrollHeight; return setJump(false) }
      case 'mention': {
        const q = document.getElementById('kn-chat-q')
        if (!q) return
        const at = q.selectionStart, pre = q.value.slice(0, at), ins = !pre || /\s$/.test(pre) ? '@' : ' @'
        q.value = pre + ins + q.value.slice(q.selectionEnd)
        q.focus()
        q.setSelectionRange(at + ins.length, at + ins.length)
        return onChatInput(q)
      }
      case 'attach': return openAttach()
      case 'pick-page': return pickPage(Number(d.knChip))
      case 'pick-call': return pickCall(Number(d.knChip))
      case 'chip-x': chat().chips.splice(Number(d.knChip), 1); return patchEl('kn-cmp-chips', chipsHtml())
      case 'chip-anyway': { const x = chat().chips[Number(d.knChip)]; if (x) { x.anyway = true; x.mismatch = null } return sendChat() }
      case 'chat-terminal': return toTerminal()
      case 'chat-command': return copyCommand()
      case 'signin': send({ cmd: 'sign_in', profile: d.knProfile }); return done('Sign in in your browser, then send again.')
      case 'ans-copy': { const a = answerOf(Number(d.knTurn)); if (!a) return; try { await navigator.clipboard.writeText(a.md) } catch { /* not allowed */ } return done('Copied.') }
      case 'ans-save': {
        const a = answerOf(Number(d.knTurn)), c = chat()
        if (!a) return
        const name = String(a.question || 'An answer').split('\n')[0].replace(/[?.!\s]+$/, '').slice(0, 60)
        return openNewPage(name, { md: a.md, source: `echo-ask ${c.id}` })
      }
      case 'ans-add': { const a = answerOf(Number(d.knTurn)); if (!a) return; s.sheet = { kind: 'addto', md: a.md, q: '', hits: null }; s.focus = 'kn-addq'; return render() }
      case 'addto-page': return addToPage(d.path, (s.sheet || {}).md || '')
      case 'ans-sug': {
        const t = turnsOf(chat())[Number(d.knTurn)], q = document.getElementById('kn-chat-q')
        if (!t || !t.suggestion || !q) return
        chat().draft = q.value = t.suggestion
        q.focus(); q.setSelectionRange(q.value.length, q.value.length); return fitQ()
      }
      case 'newpage': return openNewPage(d.name)
      case 'filter': s.filter = d.filter; s.q = ''; s.hits = null; return render()
      case 'fold': if (s.open.has(d.path)) s.open.delete(d.path); else s.open.add(d.path); return render()
      case 'open': return openPage(d.path)
      case 'open-tab': return openPage(d.path)
      case 'edit': return startEdit()
      case 'edit-cancel':
        if (s.edit && s.edit.dirty) return askFirst('Stop editing?', 'Your text isn\'t saved. It stays in this window: Edit brings it back.', 'Stop editing', () => { s.edit = null; render() })
        s.edit = null; return render()
      case 'save': return save(false)
      case 'keep-mine': return askFirst('Save your text over the change on disk?', `The change made outside Echo is replaced by your text${v && v.commit ? '; the history keeps both' : ''}.`, 'Save mine', () => save(true), true)
      case 'take-theirs': if (s.edit && s.edit.conflict) { if (store()) knDropDraft(store(), v.id, s.edit.path); s.edit = null; s.page = null; await loadPage(); render() } return
      case 'compare': if (s.edit) { s.edit.compare = !s.edit.compare; render() } return
      case 'rail': s.menu = null; s.rail = d.rail; if (d.rail === 'history') await loadHistory(); return render()
      case 'version': s.ver = { rev: d.rev, loading: true }; render(); try { s.ver = await get(`api/kn/${enc(v.id)}/version?path=${enc(s.path)}&rev=${enc(d.rev)}`) } catch (err) { s.ver = null; say(String(err.message || err)) } return render()
      case 'ver-close': s.ver = null; return render()
      case 'restore': return askFirst('Write this version back?', 'The current text stays in the history.', 'Restore', async () => { try { const r = await run('kn_restore', { id: v.id, path: s.path, rev: d.rev }, 'restore'); s.ver = null; s.page = null; await loadPage(); await loadHistory(); done(r.commit && !/^(off|skipped)/.test(r.commit) ? 'Restored, in a new commit.' : 'Restored.') } catch (err) { say(String(err.message || err)) } })
      case 'gopt': {
        const val = d.val === 'true' ? true : d.val === 'false' ? false : /^\d+$/.test(d.val) ? Number(d.val) : d.val
        s.gopt[d.key] = val
        if (d.key === 'depth' && val && !s.gopt.sel) s.gopt.depth = 0
        if (d.key === 'tags') { s.graph = null; await loadGraph() }
        return render()
      }
      case 'gsel': s.gopt.sel = d.id || null; if (!d.id) s.gopt.depth = 0; return render()
      case 'gzoom': if (gview) { if (d.f === 'fit') gview.fit(); else gview.zoom(d.f === 'in' ? 1.4 : 1 / 1.4) } return
      case 'gfold': { const h = new Set(s.gopt.hidden || []); if (h.has(d.folder)) h.delete(d.folder); else h.add(d.folder); s.gopt.hidden = [...h]; return render() }
      case 'vs-go': { const el = document.getElementById(d.to); if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' }); return }
      case 'ovr': s.ovr = { key: d.key, value: d.value }; s.focus = 'kn-ovr'; s.focusSelect = true; return render()
      case 'ovr-cancel': s.ovr = {}; return render()
      case 'ovr-reset': { const k = s.ovr.key; s.ovr = {}; await setVault(k, ''); return }
      case 'ovr-save': {
        const val = (document.getElementById('kn-ovr') || {}).value || ''
        try { const r = await run('kn_set', { id: v.id, key: s.ovr.key, value: val }); const i = s.data.vaults.findIndex((x) => x.id === v.id); if (i >= 0) s.data.vaults[i] = r.vault; s.ovr = {}; s.tree = null; s.graph = null; await loadSettings(); render() } catch (err) { s.ovr.error = String(err.message || err); render() }
        return
      }
      // Health (K16-01, -04, -07)
      case 'hl-filter': hl().filter = d.knFilter; render(); await loadFinding(); return render()
      case 'hl-pick': return goVault(s.vault, 'health', d.knKey)
      case 'hl-ignored': hl().showIgnored = !hl().showIgnored; render(); await loadFinding(); return render()
      case 'hl-choice': return pickChoice(d.knKey, d.knChoice)
      case 'hl-fix': return fixOne()
      case 'hl-ignore': return ignoreFinding(d.knKey, d.knOn === 'true')
      case 'hl-ask': return askAboutFinding(d.knKey)
      case 'hl-create': {
        const folder = d.knFolder
        await openNewPage(d.name)
        const x = st().sheet
        // In the linking page's folder, when the sheet lists it.
        if (x && x.kind === 'newpage' && (st().tree || { pages: [] }).pages.some((p) => p.path.split('/').slice(0, -1).join('/') === folder)) { x.folder = folder; render() }
        return
      }
      case 'hl-fixall': return openFixAll(d.knKind)
      case 'hl-fixall-go': return fixAllGo()
      case 'set-bool': return setVault(d.key, d.val)
      case 'scope-clear': await setVault('clients', ''); return setVault('threads', '')
      case 'rows': return openRows([d.path])
      case 'rows-go': return rowsGo()
      case 'past': {
        const n = ((s.settings || {}).past || {}).count
        return askFirst(`Save the minutes of ${n} past call${n === 1 ? '' : 's'} into ${v.name}?`, `One note each${v.commit ? ', in one commit' : ''}.`, 'Save', async () => {
          try { const r = await run('kn_past', { id: v.id, apply: true }, 'past'); await loadSettings(); await loadTree(); done(`${r.saved} note${r.saved === 1 ? '' : 's'} saved${r.skipped ? `; ${r.skipped} skipped` : ''}.`) } catch (err) { say(String(err.message || err)) }
          render()
        })
      }
      case 'twin-add': return askFirst(`Add the twin skill to ${v.name}?`, 'Echo writes only .claude/skills/twin/SKILL.md.', 'Add', async () => { try { await run('kn_twin', { id: v.id, op: 'add' }, 'twin'); await run('kn_reread', { id: v.id }); await refreshAll(); await loadSettings(); render(); done('The twin skill is in the vault.') } catch (err) { say(String(err.message || err)) } })
      case 'sample': { const x = s.sheet = { kind: 'sample' }; render(); try { x.preview = await get(`api/kn/${enc(v.id)}/sample`) } catch (err) { x.preview = null; say(String(err.message || err)) } return st().sheet === x ? render() : undefined }
      case 'twin-global-ask': s.sheet = { kind: 'twin', name: '', text: null }; render(); return loadTwinText()
      case 'twin-global': try { const r = await run('kn_twin', { id: v.id, op: 'global', name: (s.sheet && s.sheet.name) || '' }, 'twin'); s.sheet = null; await loadSettings(); render(); done(`Installed: ${short(r.installed)}`) } catch (err) { if (s.sheet) { s.sheet.error = String(err.message || err); render() } else say(String(err.message || err)) } return
      case 'twin-remove': try { await run('kn_twin', { id: v.id, op: 'global_remove' }, 'twin'); await loadSettings(); render(); done('Echo\'s copy is out of ~/.claude/skills.') } catch (err) { say(String(err.message || err)) } return
      case 'remove-ask': s.sheet = { kind: 'remove' }; return render()
      case 'remove-go': try { const name = v.name; await run('kn_remove', { id: v.id }, 'remove'); s.sheet = null; s.vault = ''; s.lastVault = null; try { localStorage.removeItem('echo.kn.vault') } catch { /* off */ } await refreshAll(); go('kn'); done(`${name} is no longer in Echo. Its folder is as it was.`) } catch (err) { say(String(err.message || err)) } return
      case 'welcome-done': try { localStorage.setItem(`echo.kn.welcome:${v.id}`, '1') } catch { /* off */ } return render()
      case 'settings-open': S.view = 'brain'; return goVault(d.id, 'settings')
      // sheets
      case 'sheet-close': s.sheet = null; return render()
      case 'sheet-back': if (e.target === t) { s.sheet = null; render() } return
      case 'ask-back': if (e.target === t) { s.asking = null; render() } return
      case 'ask-no': s.asking = null; return render()
      case 'ask-yes': { const a = s.asking; s.asking = null; render(); return a && a.run() }
      case 'pick': return doPick(false)
      case 'pick-new': return doPick(true)
      case 'pick-default': s.sheet.picked = null; return render()
      case 'new-here': { const p = s.sheet.picked; s.sheet = { kind: 'new', name: p.name || '', picked: p, git: !!(s.data || {}).git, auto: true }; s.focus = 'kn-name'; return render() }
      case 'step': { const x = s.sheet; if (d.step === '3') x.name = x.name || (x.picked && x.picked.name) || ''; x.step = Number(d.step); return render() }
      case 'connect-go': return connectGo()
      case 'new-go': return newGo()
      case 'newpage-go': return newPageGo()
      // the call page
      case 'chip': s.chip = s.chip === d.knCall ? null : d.knCall; {
        const x = knChipState(S.call && S.call.knowledge)
        if (x.state === 'saved' && s.chip) { s.chip = null; return openSavedNote(x.vault, x.path) }
      } return render()
      case 'chip-open': s.chip = null; S.menuOpen = false; return openSavedNote(d.vault, d.path)
      case 'choose': try { await run('kn_call_choice', { call: d.knCall, vault: d.vault || null }); s.chip = null; if (typeof loadCall === 'function') await loadCall(S.selected); render() } catch (err) { say(String(err.message || err)) } return
      case 'save-sheet': S.menuOpen = false; return openSave(d.knCall)
      case 'save-as-new': return saveMinutes({ call: d.knCall, vault: d.vault, new: true })
      case 'save-replace': return askFirst('Write the new minutes over your version?', 'Your changes to the note are replaced by the new minutes.', 'Replace', () => saveMinutes({ call: d.knCall, vault: d.vault, replace: true }), true)
      case 'save-again': return saveMinutes({ call: d.knCall, vault: d.vault })
      case 'save-minutes-go': {
        const x = s.sheet, stem = (document.getElementById('kn-stem') || {}).value
        const r = await saveMinutes({ call: x.call, vault: x.vault, stem: stem && stem !== (x.preview || {}).stem ? stem : '', transcript: !!x.transcript, chat: !!x.chat, anyway: !(x.preview || {}).profile_ok })
        if (r && r.mismatch) { x.error = 'Not saved: the profiles differ.'; render() }
        return
      }
    }
  })
  function openSavedNote(vaultId, path) {
    st().source = 'vault'
    goVault(vaultId, 'pages', path)
  }
  document.addEventListener('input', (e) => {
    const t = e.target
    if (t.id === 'kn-edit') return onEditInput(t.value)
    if (t.id === 'kn-find') {
      const s = st(), v = vault(), q = t.value
      s.q = q
      clearTimeout(Kn._findTimer)
      Kn._findTimer = setTimeout(async () => {
        if (!q.trim()) { s.hits = null; return render() }
        try { const r = await get(`api/kn/${enc(v.id)}/find?q=${enc(q)}`); if (st().q === q) { s.hits = r; render() } } catch { /* app restarting */ }
      }, 120)
      return
    }
    if (t.id === 'kn-gfind') { const s = st(); s.gq = t.value; const n = gview ? gview.search(t.value) : 0, el = document.getElementById('kn-gfind-n'); if (el) el.textContent = t.value.trim() ? (n ? nf(n) : 'none') : ''; return }
    if (t.id === 'kn-stem') { const x = st().sheet; if (x) x.stemEdited = true; return }
    if (t.id === 'kn-name') { const x = st().sheet; if (x) { x.name = t.value; if (x.kind === 'new') render() } return }
    if (t.id === 'kn-twin-name') { const x = st().sheet; if (x) { x.name = t.value.trim(); x.text = null; clearTimeout(Kn._twinTimer); Kn._twinTimer = setTimeout(loadTwinText, 250) } return }
    // Ask
    if (t.id === 'kn-chat-q') return onChatInput(t)
    if (t.id === 'kn-chat-find') { chat().listQ = t.value; return patchEl('kn-convs-l', convItems()) }
    if (t.id === 'kn-attach-q') { const p = chat().picker; if (p && p.kind === 'calls') { p.query = t.value; p.hits = null; drawPicker(); findCalls(t.value) } return }
    if (t.id === 'kn-addq') {
      const x = st().sheet, v = vault(), q = t.value
      if (!x || x.kind !== 'addto') return
      x.q = q; x.hits = null
      clearTimeout(Kn._addTimer)
      Kn._addTimer = setTimeout(async () => {
        if (q.trim()) { try { const r = await get(`api/kn/${enc(v.id)}/find?q=${enc(q)}`); if (st().sheet !== x || x.q !== q) return; const seen = new Set(); x.hits = (r.hits || []).filter((h) => h.path && !seen.has(h.path) && seen.add(h.path)) } catch { return } }
        patchEl('kn-addto-list', addToList())
      }, 120)
    }
  })
  document.addEventListener('change', async (e) => {
    const t = e.target, s = st()
    if (t.dataset.knSet) {
      const key = t.dataset.knSet
      const val = t.type === 'checkbox' ? String(t.checked) : t.value
      if (key === 'name' && !val.trim()) { t.value = vault().name; return }
      return setVault(key, val)
    }
    if (t.dataset.knAdd) {
      if (!t.value) return
      const v = vault(), key = t.dataset.knAdd
      const list = new Set((v.minutes[key] || []).concat([t.value]))
      return setVault(key, [...list].join(','))
    }
    if (t.dataset.knAlways) { chat().always[t.dataset.knAlways] = t.checked; return }
    if (t.dataset.knHlChoice && t.value) return pickChoice(t.dataset.knHlChoice, t.value)
    if (t.dataset.knFixall && s.sheet && s.sheet.kind === 'fixall') {
      const x = s.sheet
      if (t.checked) x.off.delete(t.dataset.knFixall); else x.off.add(t.dataset.knFixall)
      return patchEl('kn-fixall-f', fixAllFoot(knFixAll(x.fixes, x.findings, x.off), x.fixKind))
    }
    if (t.dataset.knForm && s.sheet) {
      const k = t.dataset.knForm
      if (k === 'minutes') s.sheet.minutes = t.value
      else if (k === 'profile') s.sheet.profile = t.value
      else if (k === 'commit') s.sheet.commit = t.checked
      else if (k === 'git') s.sheet.git = t.checked
      else if (k === 'auto') s.sheet.auto = t.checked
      else if (k === 'folder') s.sheet.folder = t.value
      else if (k === 'template') s.sheet.template = t.value
      else if (k === 'row') s.sheet.row = t.checked
      else if (k === 'transcript') s.sheet.transcript = t.checked
      else if (k === 'chat') s.sheet.chat = t.checked
      else if (k === 'svault') { s.sheet.vault = t.value; s.sheet.preview = null; s.sheet.stem = null; s.sheet.stemEdited = false; render(); return loadPreview() }
      if (k === 'name') return
    }
  })
  document.addEventListener('keydown', (e) => {
    const s = st()
    if (e.key === 'Escape' && s.asking) { s.asking = null; e.stopPropagation(); return render() }
    if (e.key === 'Escape' && (s.sheet || s.switcher || s.menu || s.chip)) { s.sheet = null; s.switcher = false; s.menu = null; s.chip = null; e.stopPropagation(); return render() }
    // Ask's keys, inside its view (a terminal or another field keeps its own).
    if (s.tab === 'ask' && askShown() && (e.target === document.body || e.target === document.documentElement || (e.target.closest && e.target.closest('.kn-chat'))) && chatKey(e)) return
    if (e.target.id === 'kn-addq' && e.key === 'Enter') { const x = s.sheet; const h = x && x.hits && x.hits[0]; if (h) addToPage(h.path, x.md) }
    if (e.target.id === 'kn-edit' && (e.metaKey || e.ctrlKey) && e.key === 's') { e.preventDefault(); return save(false) }
    if (e.target.id === 'kn-find' && e.key === 'Enter') { const h = s.hits && s.hits.hits[0]; if (h) openPage(h.path) }
    if (e.target.id === 'kn-gfind' && e.key === 'Enter' && gview) { const id = gview.best(); if (id) { s.gopt.sel = id; gview.center(id); render() } }
    if (e.target.id === 'kn-name' && e.key === 'Enter' && s.sheet) { if (s.sheet.kind === 'new') newGo(); else if (s.sheet.kind === 'connect' && s.sheet.step === 3) connectGo() }
    if (e.target.id === 'kn-pname' && e.key === 'Enter') newPageGo()
    if (e.target.id === 'kn-ovr' && e.key === 'Enter') { const b = document.querySelector('[data-kn="ovr-save"]'); if (b) b.click() }
  }, true)
  // Ask's thread: follow the bottom only while the user is there (scroll events don't bubble).
  document.addEventListener('scroll', (e) => { if (e.target && e.target.id === 'kn-thread') onThreadScroll(e.target) }, true)
  // Leaving the window with unsaved text: the draft is kept; ask anyway.
  window.addEventListener('beforeunload', (e) => { if (S && S.kn && S.kn.edit && S.kn.edit.dirty) { e.preventDefault(); e.returnValue = '' } })

  return {
    view, after, tick, load, route: routeTo, active, home, title, overlay, callChip, callMenuItems, countdownLine,
    settingsVaults, paletteActions, askView, askLabel, feedQueued,
    vaultId: () => (st().source === 'vault' || active()) ? st().vault : '',
    setSource: (src) => { st().source = src },
    editing: () => !!(S.kn && S.kn.edit),
    resume: async () => { const v = vault(); if (!v) return; try { st().resume = (await get(`api/kn-resume?vault=${enc(v.id)}`)).command } catch { /* none */ } },
  }
})()
