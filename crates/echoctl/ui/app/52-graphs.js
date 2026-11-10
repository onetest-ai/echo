// ── graphs: Sigma.js (WebGL) over graphology, vendored (`ui/vendor/README.md`) ─────────────────────
// One instance per graph box: built once per data set (ForceAtlas2 run in the page, then spaced
// apart), kept across renders — a render that replaces the box remounts the same graph, positions
// and all. Search focuses: the matches and their neighbours stay lit, the rest is dimmed, the camera
// flies to them; hovering does the same for one node.
const CLIENT_COLORS = ['#9b78ff', '#5aa9ff', '#3ccf91', '#f0a24a', '#e8603c', '#d36bd1', '#4cc3c8', '#c9b458']
const GraphView = (() => {
  const live = new Map() // box id → { sigma, graph, key, box, match, focus, hover, sel, opts }
  const cache = new Map() // data key → graph (laid out once)
  const css = (v, f) => (getComputedStyle(document.documentElement).getPropertyValue(v) || '').trim() || f
  function layout(graph) {
    const n = graph.order
    if (!n || typeof graphologyLibrary === 'undefined') return
    const L = graphologyLibrary
    const fa2 = L.layoutForceAtlas2
    const settings = { ...fa2.inferSettings(graph), barnesHutOptimize: n > 400, gravity: 1, scalingRatio: n > 400 ? 6 : 12, slowDown: 2 }
    fa2.assign(graph, { iterations: n > 3000 ? 60 : n > 800 ? 120 : 250, settings, getEdgeWeight: 'weight' })
    if (L.layoutNoverlap) L.layoutNoverlap.assign(graph, { maxIterations: n > 2000 ? 20 : 80, settings: { margin: 4, ratio: 1.3 } })
  }
  /** Draw `box` (an element with an id) from `build()` → a graphology graph whose nodes carry x, y,
   *  size, color, label, `group` (seeds the layout) and `pinLabel` (always labelled); edges weight,
   *  size, color. `key` names the data: the same key keeps the same graph. */
  function draw(box, key, build, opts) {
    if (!box || typeof Sigma === 'undefined' || typeof graphology === 'undefined') return null
    const id = box.id
    let v = live.get(id)
    if (v && v.key === key && v.box === box) return v
    // The same data in a new box (a render replaced it): the camera stays where it was.
    const cam = v && v.key === key ? v.sigma.getCamera().getState() : null
    if (v) { try { v.sigma.kill() } catch { /* gone with its box */ } }
    let graph = cache.get(key)
    if (!graph) { graph = build(); layout(graph); cache.set(key, graph); if (cache.size > 8) cache.delete(cache.keys().next().value) }
    box.innerHTML = ''
    const st = { key, box, graph, match: v && v.key === key ? v.match : null, focus: v && v.key === key ? v.focus : null, hover: null, sel: null, opts }
    // WebGL reads plain colours only: the theme's hex values, a dimmed node by the background's lightness.
    const bg = css('--bg', '#f6f6f8'), dark = /^#/.test(bg) && parseInt(bg.slice(1, 3), 16) < 128
    const dim = dark ? '#33333d' : '#e1e1e8', text = css('--text', dark ? '#eee' : '#222'), muted = css('--text-3', '#8a8a95'), soft = css('--text-2', '#55555f')
    // The hovered node's label sits on a card (sigma's backdrop, white by default): the theme's.
    const card = dark
      ? { backdropColor: '#26262e', backdropBorderColor: '#3a3a45', backdropBorderWidth: 1, backdropShadowColor: 'rgba(0, 0, 0, 0.55)', backdropCornerRadius: 6 }
      : { backdropColor: '#ffffff', backdropBorderColor: '#e1e1e8', backdropBorderWidth: 1, backdropShadowColor: 'rgba(17, 17, 34, 0.18)', backdropCornerRadius: 6 }
    const near = (k) => new Set([k, ...graph.neighbors(k)])
    const sigma = new Sigma(graph, box, {
      settings: { labelDensity: 0.08, labelGridCellSize: 120, labelRenderedSizeThreshold: 7, hideEdgesOnMove: graph.size > 3000, zIndex: true, minCameraRatio: 0.02, maxCameraRatio: 4, stagePadding: 24, labelFont: getComputedStyle(document.body).fontFamily, labelSize: 11 },
      nodeReducer: (k, d, a) => {
        const hov = st.hover && near(st.hover)
        const out = { ...d, ...card, labelColor: a.pinLabel ? text : muted }
        const lit = (st.focus && st.focus.has(k)) || (hov && hov.has(k)) || k === st.sel
        const off = (st.focus && !st.focus.has(k)) || (hov && !hov.has(k))
        if (off) return { ...out, color: dim, label: null, backdropVisibility: 'hidden', zIndex: 0 }
        if ((st.match && st.match.has(k)) || k === st.sel || k === st.hover) return { ...out, highlighted: true, labelVisibility: 'visible', labelColor: text, zIndex: 3 }
        // Labels where they fit (sigma's grid, the biggest first: topics and threads); lit ones always.
        if (lit) return { ...out, labelVisibility: 'visible', labelColor: a.pinLabel ? text : soft, zIndex: 2 }
        if (a.pinLabel) return { ...out, zIndex: 1 }
        return out
      },
      edgeReducer: (k, d) => {
        const [s, t] = graph.extremities(k)
        const hov = st.hover && near(st.hover)
        const on = (x) => (!st.focus || st.focus.has(x)) && (!hov || hov.has(x))
        if (!on(s) || !on(t)) return { ...d, visibility: 'hidden' }
        return (st.hover && (s === st.hover || t === st.hover)) || (st.sel && (s === st.sel || t === st.sel)) ? { ...d, color: css('--accent', '#8b62ff'), size: Math.max(d.size, 1.5), zIndex: 2 } : d
      },
    })
    st.sigma = sigma
    sigma.on('enterNode', ({ node }) => { st.hover = node; box.style.cursor = 'pointer'; sigma.refresh({ skipIndexation: true }) })
    sigma.on('leaveNode', () => { st.hover = null; box.style.cursor = ''; sigma.refresh({ skipIndexation: true }) })
    sigma.on('clickNode', ({ node }) => opts.click && opts.click(node, graph.getNodeAttributes(node)))
    live.set(id, st)
    if (cam) sigma.getCamera().setState(cam)
    return st
  }
  /** Fly the camera to these nodes (their bounding box), or back to the whole graph. */
  function fly(st, keys) {
    const cam = st.sigma.getCamera()
    const pts = [...(keys || [])].map((k) => st.sigma.getNodeDisplayData(k)).filter(Boolean)
    if (!pts.length) return cam.animate({ x: 0.5, y: 0.5, ratio: 1, angle: 0 }, { duration: 400 })
    const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y)
    const span = Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys))
    cam.animate({ x: (Math.max(...xs) + Math.min(...xs)) / 2, y: (Math.max(...ys) + Math.min(...ys)) / 2, ratio: Math.min(1, Math.max(0.08, span * 1.4)) }, { duration: 500 })
  }
  /** Light these nodes (null: none) and their neighbours, dim the rest, fly to them. */
  function focus(id, keys) {
    const st = live.get(id)
    if (!st) return 0
    const g = st.graph
    const m = keys ? new Set([...keys].filter((k) => g.hasNode(k))) : null
    st.match = m && m.size ? m : null
    st.focus = st.match ? new Set([...st.match].flatMap((k) => [k, ...g.neighbors(k)])) : null
    st.sigma.refresh({ skipIndexation: true })
    fly(st, st.match)
    return st.match ? st.match.size : 0
  }
  function select(id, key) {
    const st = live.get(id)
    if (!st || st.sel === key) return
    st.sel = key && st.graph.hasNode(key) ? key : null
    st.sigma.refresh({ skipIndexation: true })
  }
  /** Nodes whose label (or `search` text) has every word typed. */
  function find(id, q) {
    const st = live.get(id)
    const words = String(q || '').toLowerCase().split(/\s+/).filter(Boolean)
    if (!st || !words.length) return null
    const hits = new Set()
    st.graph.forEachNode((k, a) => { const t = `${a.label || ''} ${a.search || ''}`.toLowerCase(); if (words.every((w) => t.includes(w))) hits.add(k) })
    return hits
  }
  /** Change the drawn graph in bulk (`fn(graph)`) with the renderer off — sigma answers each dropped node and edge
   *  one by one, which takes minutes for a few hundred sources with their links — then draw it again as it was seen. */
  function batch(id, fn) {
    const st = live.get(id)
    if (!st) return null
    const cam = st.sigma.getCamera().getState()
    try { st.sigma.kill() } catch { /* gone with its box */ }
    live.delete(id)
    fn(st.graph)
    const v = draw(st.box, st.key, () => st.graph, st.opts)
    if (v) v.sigma.getCamera().setState(cam)
    return v
  }
  const resize = () => { for (const st of live.values()) if (st.box.isConnected) st.sigma.resize() }
  /** The theme changed (the system's appearance): drawn again in its colours, laid out as they were. */
  function retheme() {
    for (const [id, st] of live) { try { st.sigma.kill() } catch { /* gone with its box */ } live.delete(id) }
  }
  return { draw, batch, focus, select, find, resize, retheme, live }
})()

// The Brain's graph, drawn as you go (it may hold millions of sections): topics first, laid out once — each
// sized by its sections, coloured by whose it is, joined to the topics their sections are related to. A click on a
// topic fans out its 50 most tagged sources; a client's chip in the legend (or zooming in on its topics) brings in its
// sources; a search's sources come in too. At most `GRAPH_MAX` sources on screen: the oldest opened set goes first.
// Sources are added to the drawn graph where they belong (round their topic, or amid their topics), never laid out again.
const GRAPH_MAX = 400
function brainGraph() {
  const g = S.br.graph
  if (!g || g.q !== brainQuery()) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  if (g.error) return `<p class="error">${esc(g.error)}</p>`
  if (!g.nodes || !g.nodes.length) return '<p class="set-sub">Nothing to draw in this scope yet: topics come first, then their sources.</p>'
  const layers = [...new Set(g.nodes.map((n) => n.layer).filter((l) => l && l !== 'shared'))]
  const sets = S.br.gsets || []
  const shown = new Set(sets.flatMap((s) => s.nodes.map((n) => n.id))).size
  const chip = (s) => `<button class="fchip on" data-gset-close="${esc(s.key)}" title="Take these sources off the graph">${esc(s.label)} · ${s.nodes.length}${s.total > s.nodes.length ? ` of ${nf(s.total)}` : ''} ×</button>`
  const legend = `<span class="pp-leg"><i class="br-dot" style="background:#8a8a95"></i>Your practice</span>` + layers.map((c, i) => `<button class="pp-leg link-btn" data-gset-client="${esc(c)}" title="Bring ${esc(clientName(c))}'s sources onto the graph"><i class="br-dot" style="background:${CLIENT_COLORS[i % CLIENT_COLORS.length]}"></i>${esc(clientName(c))}</button>`).join('')
  const box = `<input class="wp-in br-pq" id="br-gq" type="search" placeholder="Find a topic or source — Enter searches Recall" value="${esc(S.br.gq || '')}" aria-label="Find in the graph">`
  const res = S.br.gqRes ? `<span class="set-sub br-gq-res">${esc(S.br.gqRes)}</span>` : ''
  const busy = S.br.gbusy ? `<span class="set-sub">${esc(S.br.gbusy)}</span>` : ''
  return `<div class="br-graph-bar">${box}${res}${busy}<span class="grow"></span><span class="set-sub">${g.nodes.length} topics · ${shown} of ${nf(g.sources_total)} sources shown</span></div>
    ${sets.length ? `<div class="br-graph-bar">${sets.map(chip).join('')}</div>` : ''}<div class="br-graph-bar">${legend}</div><div class="br-graph" id="br-graph"></div>
    <p class="set-foot">Topics first. Click a topic: its sources fan out (the 50 most tagged with it). Click a client, or zoom in on its topics: its sources fill in. At most ${GRAPH_MAX} sources on screen. Scroll to zoom, drag to move; hover for a node's links.</p>`
}
function graphKey(g) { return `brain|${g.q}|${g.v || ''}|${g.nodes.length}|${g.edges.length}|${g.nodes.slice(0, 3).map((n) => n.id).join(',')}` }
function graphColors(g) {
  const layers = [...new Set(g.nodes.map((n) => n.layer).filter((l) => l && l !== 'shared'))]
  return (c) => c && c !== '-' && c !== 'shared' && layers.indexOf(c) >= 0 ? CLIENT_COLORS[layers.indexOf(c) % CLIENT_COLORS.length] : '#8a8a95'
}
/** Bring a set of sources onto the graph (a topic's, a client's, a search's); the oldest go past `GRAPH_MAX`. */
async function openGraphSet(key, label, url, select) {
  const sets = S.br.gsets || (S.br.gsets = [])
  if (sets.some((s) => s.key === key)) { S.br.gsets = sets.filter((s) => s.key !== key).concat(sets.filter((s) => s.key === key)); return render() }
  S.br.gbusy = `Loading ${label}…`; render()
  try {
    const r = await get(url)
    if (r.error) throw new Error(r.error)
    const set = { key, label, nodes: r.nodes || [], edges: r.edges || [], total: r.total || 0, select }
    let next = (S.br.gsets || []).concat([set])
    while (next.length > 1 && new Set(next.flatMap((s) => s.nodes.map((n) => n.id))).size > GRAPH_MAX) next = next.slice(1)
    S.br.gsets = next
    S.br.gbusy = ''
  } catch (e) { S.br.gbusy = String(e.message || e) }
  render()
}
function openTopicSources(id, label) {
  const sc = brainQuery().replace('?', '&')
  return openGraphSet(`topic:${id}`, label, `api/brain/browse/graph-topic?id=${encodeURIComponent(id)}&limit=50${sc}`)
}
function openClientSources(client) {
  return openGraphSet(`client:${client}`, clientName(client), `api/brain/browse/graph-client?id=${encodeURIComponent(client)}&limit=${GRAPH_MAX}`)
}
/** The drawn graph made to hold exactly the open sets' sources (added where they belong, the rest taken off), in
 *  one batch with the renderer off. Returns the view as drawn now. */
function syncGraphSets(st, g) {
  const want = new Map()
  for (const s of S.br.gsets || []) for (const n of s.nodes) if (!want.has(n.id)) want.set(n.id, { n, s })
  const gone = st.graph.filterNodes((k, a) => a.kind === 'doc' && !want.has(k))
  const adds = [...want.keys()].filter((k) => !st.graph.hasNode(k))
  if (!gone.length && !adds.length) return st
  const color = graphColors(g)
  return GraphView.batch('br-graph', (G) => {
    for (const k of gone) G.dropNode(k)
    const xs = [], ys = []
    G.forEachNode((k, a) => { if (a.kind === 'topic') { xs.push(a.x); ys.push(a.y) } })
    const cx = xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0, cy = ys.length ? ys.reduce((a, b) => a + b, 0) / ys.length : 0
    const span = xs.length ? Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys), 1) : 100
    const per = new Map()
    const about = new Map()
    for (const s of S.br.gsets || []) for (const e of s.edges) { if (!about.has(e.source)) about.set(e.source, []); about.get(e.source).push(e) }
    for (const id of adds) {
      const { n, s } = want.get(id)
      const tops = (about.get(id) || []).filter((e) => G.hasNode(e.target))
      let x, y
      if (s.key.startsWith('topic:') && G.hasNode(s.key)) {
        // A topic's sources on an arc round it, facing away from the middle.
        const t = G.getNodeAttributes(s.key), i = per.get(s.key) || 0
        per.set(s.key, i + 1)
        const out = Math.atan2(t.y - cy, t.x - cx), a = out + ((i % 25) - 12) * 0.11, r = span * (0.09 + 0.035 * Math.floor(i / 25))
        x = t.x + r * Math.cos(a); y = t.y + r * Math.sin(a)
      } else if (tops.length) {
        // Amid the topics it is about (weighted), a little apart.
        const w = tops.reduce((a, e) => a + (e.weight || 1), 0)
        x = tops.reduce((a, e) => a + G.getNodeAttribute(e.target, 'x') * (e.weight || 1), 0) / w
        y = tops.reduce((a, e) => a + G.getNodeAttribute(e.target, 'y') * (e.weight || 1), 0) / w
        const h = ProjectArt.hash(id)
        x += ((h % 101) - 50) / 50 * span * 0.04; y += (((h >> 7) % 101) - 50) / 50 * span * 0.04
      } else {
        const h = ProjectArt.hash(id), a = (h % 360) * Math.PI / 180
        x = cx + span * 0.12 * Math.cos(a); y = cy + span * 0.12 * Math.sin(a)
      }
      G.addNode(id, { x, y, size: 5 + Math.log10(1 + (n.sections || 1)) * 4, color: color(n.client || '-'), label: n.label || id, search: [n.kind, n.date, n.client && clientName(n.client)].filter(Boolean).join(' '), kind: 'doc' })
    }
    for (const s of S.br.gsets || []) {
      for (const e of s.edges) {
        if (!G.hasNode(e.source) || !G.hasNode(e.target) || G.hasEdge(e.source, e.target)) continue
        G.addEdge(e.source, e.target, { weight: e.weight || 1, size: Math.min(2, 0.4 + (e.weight || 1) / 10), color: 'rgba(130,130,145,0.25)' })
      }
    }
  }) || st
}
function drawGraph() {
  const box = $('#br-graph'), g = S.br.graph
  if (!box || !g || !g.nodes) return
  const color = graphColors(g)
  const st = GraphView.draw(box, graphKey(g), () => {
    const G = new graphology.Graph({ multi: false, type: 'undirected' })
    // Seeds: topics on a ring, a layer's together (ForceAtlas2 does the rest, once).
    const topics = [...g.nodes].sort((a, b) => (a.layer || '').localeCompare(b.layer || ''))
    topics.forEach((t, i) => { const a = (2 * Math.PI * i) / Math.max(1, topics.length); G.addNode(t.id, { x: 100 * Math.cos(a), y: 100 * Math.sin(a), size: 8 + Math.log10(1 + (t.sections || 0)) * 6, color: color(t.layer), label: t.label, pinLabel: true, forceLabel: true, kind: 'topic', layer: t.layer, search: `${t.sources} sources` }) })
    for (const e of g.edges) {
      if (!G.hasNode(e.source) || !G.hasNode(e.target) || G.hasEdge(e.source, e.target)) continue
      G.addEdge(e.source, e.target, { weight: e.weight || 1, size: 0.6, color: 'rgba(139,98,255,0.22)' })
    }
    return G
  }, { click: (node, a) => {
    if (a.kind === 'topic') { const id = node.replace(/^topic:/, ''); openTopicSources(node.slice(6), a.label); return press({ brNode: id }) }
    return press({ graphDoc: node })
  } })
  if (!st) return
  const now = syncGraphSets(st, g)
  GraphView.select('br-graph', S.br.doc || (S.br.node ? `topic:${S.br.node}` : null))
  // Zooming in on a client's topics brings its sources in (once per zoom; nothing for the practice's).
  if (!now.zoomWatch) {
    now.zoomWatch = true
    let t = null
    now.sigma.getCamera().on('updated', (c) => {
      clearTimeout(t)
      // Zoomed out again: a client closed by hand may come in by zooming once more.
      if (c.ratio > 0.35) { if (S.br.gnoauto) S.br.gnoauto.clear(); return }
      t = setTimeout(() => {
        const mid = now.sigma.viewportToGraph({ x: box.clientWidth / 2, y: box.clientHeight / 2 })
        let best = null, bd = Infinity
        now.graph.forEachNode((k, a) => { if (a.kind !== 'topic' || !a.layer || a.layer === 'shared') return; const d = (a.x - mid.x) ** 2 + (a.y - mid.y) ** 2; if (d < bd) { bd = d; best = a.layer } })
        if (best && !(S.br.gsets || []).some((s) => s.key === `client:${best}`) && !(S.br.gnoauto || new Set()).has(`client:${best}`)) openClientSources(best)
      }, 600)
    })
  }
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-gset-close],[data-gset-client]')
  if (!t) return
  if (t.dataset.gsetClient) { if (S.br.gnoauto) S.br.gnoauto.delete(`client:${t.dataset.gsetClient}`); return openClientSources(t.dataset.gsetClient) }
  // Closed by hand: zooming in where it is doesn't bring it back (until zoomed out).
  if (t.dataset.gsetClose.startsWith('client:')) (S.br.gnoauto || (S.br.gnoauto = new Set())).add(t.dataset.gsetClose)
  S.br.gsets = (S.br.gsets || []).filter((s) => s.key !== t.dataset.gsetClose)
  render()
})
/** The graph's search: typing lights matching topics and sources (by name); Enter asks the Brain (words and
 *  meaning), brings the sources it found onto the graph and lights them. */
async function graphSearch(q, ask) {
  S.br.gq = q
  if (!q.trim()) { S.br.gqRes = ''; GraphView.focus('br-graph', null); return }
  if (!ask) { const hits = GraphView.find('br-graph', q); const n = GraphView.focus('br-graph', hits); S.br.gqRes = n ? `${n} by name · Enter searches Recall` : 'no name matches · Enter searches Recall'; return }
  S.br.gqRes = 'searching Recall…'; render()
  try {
    const sc = brainQuery().replace('?', '&')
    const r = await get(`api/brain/browse/graph-search?q=${encodeURIComponent(q)}${sc}`)
    if (S.br.gq !== q) return
    if (r.error) throw new Error(r.error)
    const set = { key: 'search', label: `“${q}”`, nodes: r.nodes || [], edges: r.edges || [], total: r.total || 0 }
    S.br.gsets = (S.br.gsets || []).filter((s) => s.key !== 'search').concat([set])
    while (S.br.gsets.length > 1 && new Set(S.br.gsets.flatMap((s) => s.nodes.map((n) => n.id))).size > GRAPH_MAX) S.br.gsets = S.br.gsets.slice(1)
    render()
    const n = GraphView.focus('br-graph', new Set(set.nodes.map((x) => x.id)))
    S.br.gqRes = `${r.sections} section${r.sections === 1 ? '' : 's'} in ${n} source${n === 1 ? '' : 's'}`
  } catch (e) { S.br.gqRes = String(e.message || e) }
  render()
}

