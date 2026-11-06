// Knowledge in the Echo window (spec tasks/knowledge/spec.md): your vaults — folders of Markdown you own — read,
// edited, graphed and asked, and the minutes of your calls saved into them. Loaded before app.js; it uses app.js's
// helpers (S, esc, get, send, render, go, ICON …) only when called, never while it loads. Everything shown that isn't
// Echo's own words is escaped; a page's HTML comes from Echo's safe writer (`minutes::page_html`).
'use strict'

// ── kn-route: `#kn/<vault>/<tab>/<path>` (a page's path encoded as one part) ──
const KN_TABS = ['pages', 'graph', 'settings']
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

/** Knowledge's views and state (`S.kn`). */
const Kn = (() => {
  const st = () => {
    if (!S.kn) S.kn = { data: null, vault: '', tab: 'pages', path: '', tree: null, page: null, filter: 'all', q: '', hits: null, open: new Set(), rail: 'links', hist: null, ver: null, edit: null, graph: null, gopt: { hideIndex: true, isolated: true, tags: false, color: 'folder', depth: 0, sel: null, hidden: [] }, gq: '', settings: null, checks: null, sheet: null, switcher: false, menu: null, busy: {}, error: null, chip: null, lastVault: null, ovr: {} }
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
    if (s.tab === 'pages') {
      if (!s.tree || s.tree.vault !== v.id) await loadTree()
      if (!s.path) s.path = (s.tree && s.tree.index) || ''
      if (s.path && (!s.page || s.page.path !== s.path || s.page.vault !== v.id)) await loadPage()
    } else if (s.tab === 'graph') await loadGraph()
    else if (s.tab === 'settings') await loadSettings()
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
    try { s.settings = await get(`api/kn/${enc(v.id)}/settings`); s.checks = await get(`api/kn/${enc(v.id)}/checks`) } catch (e) { s.error = String(e.message || e) }
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
    const known = s.tree ? s.tree.rev : s.graph ? s.graph.rev : null
    if (!known || rev === known) return
    await loadTree()
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
    if (r.path !== undefined && r.path !== s.path) { s.path = r.path; s.ver = null }
    s.source = 'vault'
  }
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
    return id ? knRouteOf({ vault: id, tab: s.tab || 'pages', path: s.path || '' }) : 'kn'
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
    const menu = s.menu === 'vault' ? `<div class="menu kn-head-menu" role="menu"><button class="menu-item" role="menuitem" data-kn="reveal" data-path="">${/Windows/.test(navigator.userAgent) ? 'Show in Explorer' : 'Show in Finder'}</button><button class="menu-item" role="menuitem" data-kn="obsidian" data-path="${esc((s.tree || {}).index || '')}">Open in Obsidian</button><button class="menu-item" role="menuitem" data-kn="reread">Read the folder again</button><div class="menu-sep" role="separator"></div><button class="menu-item" role="menuitem" data-kn="tab" data-kn-tab="settings">Vault settings</button></div>` : ''
    const acts = v.available === false ? '' : `<button class="btn sm" data-kn="ask" title="Claude in this vault's folder">${IC.ask} Ask</button><button class="btn sm" data-kn="newpage">${IC.plus} New page</button><span class="kn-rel"><button class="icon-btn" data-kn="menu" data-menu="vault" aria-haspopup="menu" aria-expanded="${s.menu === 'vault'}" aria-label="More">${IC.more}</button>${menu}</span>`
    return `<header class="kn-head">${title()}${where}<span class="grow"></span>${acts}</header>`
  }
  function tabs() {
    const s = st()
    const t = (id, label) => `<button class="tab" role="tab" data-kn="tab" data-kn-tab="${id}" aria-selected="${s.tab === id}">${label}</button>`
    return `<div class="kn-tabs"><div class="tabs" role="tablist">${t('pages', 'Pages')}${t('graph', 'Graph')}${t('settings', 'Settings')}</div></div>`
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
    const s = st(), x = s.settings, v = vault(), c = s.checks
    if (!x || !v || x.vault.id !== v.id) return '<div class="kn-vset"><div class="skeleton" style="padding:24px"><i></i><i></i><i></i></div></div>'
    const d = v.detected || {}, P = v.paths || {}
    const row = (t, sub, ctl, id) => `<div class="set-row"${id ? ` id="${id}"` : ''}><div><b>${t}</b><div class="set-sub">${sub}</div></div><div class="set-ctl">${ctl}</div></div>`
    const editing = s.ovr.key
    const change = (key, label, value, sub) => editing === key
      ? `<div class="set-row"><div><b>${label}</b><div class="kn-inline"><input class="search" id="kn-ovr" value="${esc(s.ovr.value)}" aria-label="${esc(label)}" spellcheck="false"><button class="btn primary sm" data-kn="ovr-save">Save</button><button class="btn ghost sm" data-kn="ovr-cancel">Cancel</button>${(v.overrides || {})[key] ? '<button class="btn ghost sm" data-kn="ovr-reset">As detected</button>' : ''}</div>${s.ovr.error ? `<div class="error">${esc(s.ovr.error)}</div>` : ''}</div><div></div></div>`
      : row(label, `${value ? `<code>${esc(value)}</code>` : '<span class="set-sub">none found</span>'}${sub ? ` — ${sub}` : ''}${(v.overrides || {})[key] ? ' <span class="tag">changed by you</span>' : ''}`, `<button class="btn ghost sm" data-kn="ovr" data-key="${key}" data-value="${esc(value || '')}">Change</button>`)
    const nav = [['vs-structure', 'Structure'], ['vs-minutes', 'Meeting minutes'], ['vs-schema', 'Schema & vocabulary'], ['vs-skills', 'Skills'], ['vs-checks', 'Checks'], ['vs-git', 'Git & AI']]
    const findings = c ? (c.not_in_index.length ? 1 : 0) + (c.broken.length ? 1 : 0) + (c.isolated.length ? 1 : 0) + (c.no_props.length ? 1 : 0) + (c.unknown_tags.length ? 1 : 0) + (c.meeting_tag_missing ? 1 : 0) : 0
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
    const show = (k) => s.showCheck === k
    const list = (k, items, fmt) => show(k) ? `<div class="kn-check-list">${items.slice(0, 60).map(fmt).join('')}${items.length > 60 ? `<div class="set-sub">and ${items.length - 60} more</div>` : ''}</div>` : ''
    const openP = (p, sub) => `<button class="kn-check-item" data-kn="open-tab" data-path="${esc(p)}"><span>${esc(p.replace(/^[^/]*\//, ''))}</span>${sub ? `<span class="set-sub">${esc(sub)}</span>` : ''}</button>`
    const checkRow = (k, n, t, sub, extra = '') => n ? `${row(`${t}`, sub, `${extra}<button class="btn ghost sm" data-kn="show-check" data-check="${k}">${show(k) ? 'Hide' : 'Show'}</button>`)}` : ''
    const checks = !c ? '<div class="skeleton" style="padding:12px"><i></i></div>' : findings === 0 && !c.no_index ? row('Nothing to fix', 'Every page is in the index, every link leads somewhere, every page has its properties.', '') : [
      c.no_index ? row('No index page', 'Claude finds pages through an index (<code>CATALOG.md</code>, <code>index.md</code>…). Set one under Structure.', '') : '',
      checkRow('index', c.not_in_index.length, `${c.not_in_index.length} page${c.not_in_index.length === 1 ? ' isn\'t' : 's aren\'t'} in the index`, 'Claude finds pages through the index, so these are hard to find.', `<button class="btn sm" data-kn="rows-all">Add ${c.not_in_index.length} row${c.not_in_index.length === 1 ? '' : 's'}…</button>`) + list('index', c.not_in_index, (p) => openP(p)),
      checkRow('broken', c.broken.length, `${c.broken.length} broken link${c.broken.length === 1 ? '' : 's'}`, 'Links to pages or files that aren\'t there.') + list('broken', c.broken, ([p, l]) => openP(p, `→ ${l}`)),
      checkRow('isolated', c.isolated.length, `${c.isolated.length} isolated page${c.isolated.length === 1 ? '' : 's'}`, 'No links in or out.') + list('isolated', c.isolated, (p) => openP(p)),
      checkRow('props', c.no_props.length, `${c.no_props.length} page${c.no_props.length === 1 ? '' : 's'} without properties`, 'No frontmatter: no tags, date or source.') + list('props', c.no_props, (p) => openP(p)),
      checkRow('tags', c.unknown_tags.length, `${c.unknown_tags.length} tag${c.unknown_tags.length === 1 ? '' : 's'} not in the taxonomy`, `Tags missing from <code>${esc(x.tag_file || 'the tag taxonomy')}</code>.`) + list('tags', c.unknown_tags, ([p, t]) => openP(p, `#${t}`)),
      c.meeting_tag_missing ? row(`Meeting notes get #${esc(c.meeting_tag_missing)}`, `It isn't in <code>${esc(x.tag_file || 'the taxonomy')}</code>: add it there, or choose another tag under Meeting minutes.`, '') : '',
    ].join('')
    const g = v.git
    const profiles = (typeof ai === 'function' ? ai().profiles : []) || []
    return `<div class="kn-vset"><nav class="kn-vset-nav">${nav.map(([id, l]) => `<button class="set-nav" data-kn="vs-go" data-to="${id}">${l}${id === 'vs-checks' && findings ? `<span class="set-warn">${findings}</span>` : ''}</button>`).join('')}<span class="grow"></span><button class="set-nav danger" data-kn="remove-ask">Remove from Echo…</button></nav>
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
      <section class="set-sec" id="vs-checks"><h3>Checks <span class="kn-h3-sub">read just now · changes nothing</span></h3><div class="set-card">${checks}</div></section>
      <section class="set-sec" id="vs-git"><h3>Git & AI</h3><div class="set-card">
        ${row('Git', g ? `${(g.remotes || []).length ? `<code>${esc(g.remotes.join(', '))}</code> · ` : ''}${IC.git} ${esc(g.branch || 'detached HEAD')}${g.changed ? ` · ${g.changed} file${g.changed === 1 ? '' : 's'} changed and not committed — yours; Echo never commits them` : ' · clean'}${g.busy ? ` · <span class="warn">${esc(g.busy)}</span>` : ''}` : (st().data || {}).git ? 'Not a git repository — pages have no history in Echo.' : 'git isn\'t installed — pages have no history in Echo.', `<button class="btn ghost sm" data-kn="reveal" data-path="">Reveal</button>`)}
        ${g ? row('Commit what Echo writes', v.commit ? 'One commit per change, only Echo\'s files; never a push, pull or branch switch.' : 'Off: Echo writes the files and leaves committing to you.', `<button class="sw${v.commit ? ' on' : ''}" role="switch" aria-checked="${!!v.commit}" data-kn="set-bool" data-key="commit" data-val="${!v.commit}" aria-label="Commit what Echo writes"></button>`) : ''}
        ${row('AI for this vault', 'Ask sessions here use this profile. Only calls recorded on it are saved here by themselves.', `<select class="set-in" data-kn-set="profile" aria-label="AI profile">${profiles.map((p) => `<option value="${esc(p.id)}"${p.id === v.profile ? ' selected' : ''}>${esc(p.name)}</option>`).join('') || `<option>${esc(v.profile)}</option>`}</select>`)}
        ${x.in_home && v.profile !== (st().data || {}).default_profile ? row('Inside Echo\'s data folder', 'Claude · Echo (the folder session) can read every file of the data folder, this vault too, whatever its profile.', '') : ''}
        ${row('Echo\'s index of this vault', `In Echo's data folder, not in the vault${x.cache_bytes ? ` · ${kb(x.cache_bytes)}` : ''} · rebuilt on its own`, `<button class="btn ghost sm" data-kn="reread">Rebuild</button>`)}
      </div></section>
    </div></div>`
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
      history.replaceState(null, '', `#${knRouteOf({ vault: id, tab: s.tab || 'pages', path: s.path || '' })}`)
      loadVault().then(render)
      return `<div class="kn">${head()}${tabs()}<div class="kn-body"><div class="skeleton" style="padding:20px"><i></i><i></i><i></i></div></div></div>`
    }
    if (!v) return `<div class="kn">${head()}<div class="kn-tabs kn-tabs-empty"></div><div class="kn-body">${emptyView()}</div></div>`
    if (v.available === false) return `<div class="kn">${head()}<div class="kn-tabs kn-tabs-empty"></div><div class="kn-body">${missingView(v)}</div></div>`
    const body = s.tab === 'graph' ? graphView() : s.tab === 'settings' ? settingsView() : pagesView()
    return `<div class="kn">${head()}${tabs()}<div class="kn-body">${body}</div></div>`
  }
  function after() {
    const s = st()
    if (s.tab === 'graph') drawGraph()
    if (s.tab === 'pages' && s.rail === 'links') drawMini()
    if (s.scrollTo) { const el = document.getElementById(s.scrollTo); if (el) el.scrollIntoView({ block: 'start' }); s.scrollTo = null }
    if (s.focus) { const el = document.getElementById(s.focus); if (el) { el.focus(); if (el.select && s.focusSelect) el.select() } s.focus = null; s.focusSelect = false }
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
  async function openNewPage(name) {
    const s = st(), v = vault()
    const folder = s.path ? s.path.split('/').slice(0, -1).join('/') : (s.tree && s.tree.root) || ''
    s.sheet = { kind: 'newpage', name: name || '', folder: name ? (s.tree && s.tree.root) || folder : folder, template: '', row: true, templates: await listTemplates(v) }
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
      s.checks = null
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
    const a = e.target.closest && e.target.closest('.kn-doc a, .kn-props a')
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
      case 'tab': e.preventDefault(); s.menu = null; return goVault(s.vault, d.knTab, d.knTab === 'pages' ? s.path : '')
      case 'menu': s.menu = s.menu === d.menu ? null : d.menu; return render()
      case 'reveal': s.menu = null; send({ cmd: 'kn_reveal', id: s.vault, path: d.path || '' }); return render()
      case 'obsidian': s.menu = null; send({ cmd: 'kn_obsidian', id: s.vault, path: d.path || '' }); return render()
      case 'copy': s.menu = null; try { await navigator.clipboard.writeText(d.text) } catch { /* not allowed */ } return done('Copied.')
      case 'reread': try { await run('kn_reread', { id: s.vault }, 'reread'); await refreshAll(); done('Read again.') } catch (err) { say(String(err.message || err)) } return
      case 'ask': return ask()
      case 'ask-about': return ask(`/twin What do the notes say about ${d.path}? Read it first.`)
      case 'run-skill': return ask(`/${d.name}`)
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
      case 'set-bool': return setVault(d.key, d.val)
      case 'scope-clear': await setVault('clients', ''); return setVault('threads', '')
      case 'show-check': s.showCheck = s.showCheck === d.check ? null : d.check; return render()
      case 'rows-all': return openRows((s.checks || {}).not_in_index || [])
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
    if (e.target.id === 'kn-edit' && (e.metaKey || e.ctrlKey) && e.key === 's') { e.preventDefault(); return save(false) }
    if (e.target.id === 'kn-find' && e.key === 'Enter') { const h = s.hits && s.hits.hits[0]; if (h) openPage(h.path) }
    if (e.target.id === 'kn-gfind' && e.key === 'Enter' && gview) { const id = gview.best(); if (id) { s.gopt.sel = id; gview.center(id); render() } }
    if (e.target.id === 'kn-name' && e.key === 'Enter' && s.sheet) { if (s.sheet.kind === 'new') newGo(); else if (s.sheet.kind === 'connect' && s.sheet.step === 3) connectGo() }
    if (e.target.id === 'kn-pname' && e.key === 'Enter') newPageGo()
    if (e.target.id === 'kn-ovr' && e.key === 'Enter') { const b = document.querySelector('[data-kn="ovr-save"]'); if (b) b.click() }
  }, true)
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
