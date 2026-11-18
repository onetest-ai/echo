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
