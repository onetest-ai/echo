// Knowledge in the window (knowledge.js): routes, the switcher, a call's Knowledge chip, the tree, the graph's filters
// and unsaved edits — the blocks as they ship. Run by scripts/check-mod.sh (`node crates/echoctl/ui/knowledge.test.mjs`).
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const src = readFileSync(new URL('./knowledge.js', import.meta.url), 'utf8')
const block = (name) => {
  const start = src.indexOf(`// ── ${name}`)
  const end = src.indexOf(`// end of the ${name} block`, start)
  assert.ok(start >= 0 && end > start, `the ${name} block not found in knowledge.js`)
  return src.slice(start, end)
}
const load = (names, exports) => new Function(`${names.map(block).join('\n')}\nreturn { ${exports.join(', ')} }`)()
const { knParseRoute, knRouteOf } = load(['kn-route'], ['knParseRoute', 'knRouteOf'])
const { knSwitcherEntries } = load(['kn-switcher'], ['knSwitcherEntries'])
const { knChipState } = load(['kn-chip'], ['knChipState'])
const { knBuildTree } = load(['kn-tree'], ['knBuildTree'])
const { knFilterGraph, knIsolatedCount, knGraphStart } = load(['kn-graph'], ['knFilterGraph', 'knIsolatedCount', 'knGraphStart'])
const { knRepel, knForceSim, knLabelPlan, knGraphHits, knShortLabel } = load(['kn-force'], ['knRepel', 'knForceSim', 'knLabelPlan', 'knGraphHits', 'knShortLabel'])
const { knPlainPrompt } = load(['kn-prompt'], ['knPlainPrompt'])
const { knKeepDraft, knRestoreDraft, knDropDraft } = load(['kn-draft'], ['knKeepDraft', 'knRestoreDraft', 'knDropDraft'])
const { knLineDiff } = load(['kn-diff'], ['knLineDiff'])

const tests = []
const test = (name, f) => tests.push([name, f])

test('routes: a vault, its tab, its page; Recall stays #brain', () => {
  assert.deepEqual(knParseRoute('kn/my-twin/pages/knowledge%2Fclients%2Facme.md'), { vault: 'my-twin', tab: 'pages', path: 'knowledge/clients/acme.md' })
  assert.deepEqual(knParseRoute('kn/my-twin/graph'), { vault: 'my-twin', tab: 'graph', path: '' })
  assert.deepEqual(knParseRoute('kn/my-twin'), { vault: 'my-twin', tab: 'pages', path: '' })
  assert.deepEqual(knParseRoute('kn'), { vault: '', tab: 'pages', path: '' })
  assert.equal(knParseRoute('kn/my-twin/nope'), null)
  assert.equal(knParseRoute('brain/sources'), null)
  assert.equal(knParseRoute('kn/../x/pages'), null)
  assert.equal(knParseRoute('kn/my-twin/pages/50%'), null, 'a broken escape: no route, no error')
  assert.equal(knParseRoute('kn/%E0%A4%A/pages'), null)
  assert.deepEqual(knParseRoute(knRouteOf({ vault: 'my-twin', path: 'knowledge/50% plan.md' })).path, 'knowledge/50% plan.md')
  assert.equal(knRouteOf({ vault: 'my-twin', tab: 'pages', path: 'knowledge/a b.md' }), 'kn/my-twin/pages/knowledge%2Fa%20b.md')
  assert.equal(knRouteOf({ vault: 'my-twin', tab: 'settings' }), 'kn/my-twin/settings')
})

test('the switcher: vaults first, then Recall when on, then plugins\' tabs', () => {
  const vaults = [{ id: 'my-twin', name: 'My twin', pages: 186, minutes: { mode: 'auto' } }, { id: 'team', name: 'Team wiki', pages: 42, minutes: { mode: 'ask' } }]
  const e = knSwitcherEntries(vaults, true, [{ tab: 'obsidian:notes', label: 'Notes', plugin: 'obsidian' }])
  assert.deepEqual(e.map((x) => x.kind), ['head', 'vault', 'vault', 'connect', 'new', 'sep', 'head', 'recall', 'sep', 'head', 'plugin'])
  assert.equal(e[1].star, true, 'the vault that takes minutes by itself')
  assert.equal(e[2].star, false)
  const none = knSwitcherEntries([], false, [])
  assert.deepEqual(none.map((x) => x.kind), ['head', 'connect', 'new'])
})

test('a call\'s chip: none, will save, saved, kept, failed, removed, update', () => {
  assert.equal(knChipState({ targets: [], vaults: [], record: { saved: [] } }).state, 'none')
  const will = knChipState({ targets: [{ id: 'my-twin', name: 'My twin' }], vaults: [{ id: 'my-twin', name: 'My twin' }], record: { saved: [] } })
  assert.deepEqual([will.state, will.label], ['will', 'Minutes → My twin'])
  const two = knChipState({ targets: [{ id: 'a', name: 'A' }, { id: 'b', name: 'B' }], vaults: [], record: { saved: [] } })
  assert.equal(two.label, 'Minutes → A, B')
  const saved = knChipState({ targets: [], vaults: [{ id: 'my-twin', name: 'My twin' }], record: { saved: [{ vault: 'my-twin', path: 'knowledge/notes/x.md', state: 'saved' }] } })
  assert.deepEqual([saved.state, saved.label, saved.path], ['saved', 'In My twin', 'knowledge/notes/x.md'])
  const kept = knChipState({ targets: [], vaults: [{ id: 'my-twin', name: 'My twin' }], record: { saved: [{ vault: 'my-twin', path: 'p', state: 'kept' }] } })
  assert.deepEqual([kept.state, kept.label], ['kept', 'My twin · not updated'])
  const failed = knChipState({ targets: [{ id: 'my-twin', name: 'My twin' }], vaults: [], record: { saved: [] }, failure: { vault: 'my-twin', name: 'My twin', error: 'the vault\'s folder isn\'t there' } })
  assert.deepEqual([failed.state, failed.label], ['failed', 'Couldn\'t save to My twin'])
  const removed = knChipState({ targets: [], vaults: [], record: { saved: [{ vault: 'gone', path: 'p', state: 'saved' }] } })
  assert.equal(removed.state, 'removed')
  const stale = knChipState({ targets: [], vaults: [{ id: 'team', name: 'Team wiki' }], stale: ['team'], record: { saved: [{ vault: 'team', path: 'p', state: 'saved', how: 'manual' }] } })
  assert.deepEqual([stale.state, stale.label], ['stale', 'Update in Team wiki'])
  const off = knChipState({ targets: [], vaults: [{ id: 'my-twin', name: 'My twin' }], record: { saved: [], choice: 'none' } })
  assert.deepEqual([off.state, off.label], ['off', 'Not saved to Knowledge'])
})

const PAGES = [
  { path: 'knowledge/CATALOG.md', title: 'Knowledge Catalog', tags: [], mtime: 5, in_index: null },
  { path: 'knowledge/clients/acme.md', title: 'Acme Corp', tags: ['client'], mtime: 9, in_index: true },
  { path: 'knowledge/notes/2026-01-01-a.md', title: 'First sync', tags: ['meeting'], mtime: 3, in_index: true },
  { path: 'knowledge/notes/2026-01-03-c.md', title: 'Third', tags: ['meeting'], mtime: 7, in_index: false },
  { path: 'knowledge/topics/deep/x.md', title: 'X', tags: [], mtime: 1, in_index: false },
]

test('the tree: folders with counts, open ones show their pages', () => {
  const rows = knBuildTree(PAGES, { root: 'knowledge', filter: 'all', open: new Set(['knowledge/notes']), index: 'knowledge/CATALOG.md' })
  assert.deepEqual(rows.map((r) => [r.kind, r.name, r.depth, r.count || 0]), [
    ['folder', 'clients', 0, 1], ['folder', 'notes', 0, 2], ['page', '2026-01-01-a.md', 1, 0], ['page', '2026-01-03-c.md', 1, 0], ['folder', 'topics', 0, 1], ['page', 'CATALOG.md', 0, 0],
  ])
  assert.equal(rows.find((r) => r.name === 'CATALOG.md').index, true)
})

test('the tree marks the vault\'s index alone, not the other pages it doesn\'t list', () => {
  const pages = [{ path: 'knowledge/CATALOG.md', in_index: null }, { path: 'knowledge/TAG_TAXONOMY.md', in_index: null }, { path: 'knowledge/notes/README.md', in_index: null }]
  const rows = knBuildTree(pages, { root: 'knowledge', filter: 'all', open: new Set(['knowledge/notes']), index: 'knowledge/CATALOG.md' })
  assert.deepEqual(rows.filter((r) => r.index).map((r) => r.path), ['knowledge/CATALOG.md'])
})

test('the tree\'s filters: recent first, not in the index', () => {
  const recent = knBuildTree(PAGES, { root: 'knowledge', filter: 'recent', open: new Set() })
  assert.deepEqual(recent.map((r) => r.path), ['knowledge/clients/acme.md', 'knowledge/notes/2026-01-03-c.md', 'knowledge/CATALOG.md', 'knowledge/notes/2026-01-01-a.md', 'knowledge/topics/deep/x.md'])
  assert.ok(recent.every((r) => r.kind === 'page' && r.depth === 0 && r.sub !== undefined), 'flat, each with its folder')
  const missing = knBuildTree(PAGES, { root: 'knowledge', filter: 'unindexed', open: new Set() })
  assert.deepEqual(missing.map((r) => r.path), ['knowledge/notes/2026-01-03-c.md', 'knowledge/topics/deep/x.md'])
})

const G = {
  nodes: [{ id: 'CATALOG.md' }, { id: 'a.md', tags: ['x'] }, { id: 'b.md' }, { id: 'c.md' }, { id: 'lonely.md' }, { id: '#x', tag: true }],
  edges: [['CATALOG.md', 'a.md'], ['CATALOG.md', 'b.md'], ['CATALOG.md', 'c.md'], ['a.md', 'b.md'], ['b.md', 'c.md'], ['a.md', '#x']],
  hubs: ['CATALOG.md'],
}
const ids = (g) => g.nodes.map((n) => n.id).sort()

test('the graph: hide index pages, isolated pages, tags as nodes', () => {
  const g = knFilterGraph(G, { hideIndex: true, isolated: true, tags: false })
  assert.deepEqual(ids(g), ['a.md', 'b.md', 'c.md', 'lonely.md'])
  assert.equal(g.edges.length, 2)
  assert.deepEqual(ids(knFilterGraph(G, { hideIndex: true, isolated: false, tags: false })), ['a.md', 'b.md', 'c.md'])
  assert.ok(ids(knFilterGraph(G, { hideIndex: false, isolated: true, tags: true })).includes('#x'))
})

test('the graph: the isolated count is what the switch hides, never the hidden index', () => {
  assert.equal(knIsolatedCount(G, true), 1, 'lonely.md')
  assert.equal(knIsolatedCount(G, false), 1)
  const onlyIndexed = { ...G, nodes: [...G.nodes, { id: 'd.md' }], edges: [...G.edges, ['CATALOG.md', 'd.md']] }
  assert.equal(knIsolatedCount(onlyIndexed, true), 2, 'd.md is alone once the index is hidden')
  assert.equal(knIsolatedCount(onlyIndexed, false), 1)
})

test('the graph: a local graph to a depth', () => {
  assert.deepEqual(ids(knFilterGraph(G, { hideIndex: true, isolated: true, tags: false, focus: 'a.md', depth: 1 })), ['a.md', 'b.md'])
  assert.deepEqual(ids(knFilterGraph(G, { hideIndex: true, isolated: true, tags: false, focus: 'a.md', depth: 2 })), ['a.md', 'b.md', 'c.md'])
})

test('the graph: folders turned off in the legend are left out, with their links', () => {
  const F = { nodes: [{ id: 'notes/a.md', folder: 'notes' }, { id: 'notes/b.md', folder: 'notes' }, { id: 'clients/c.md', folder: 'clients' }, { id: 'top.md', folder: '' }], edges: [['notes/a.md', 'clients/c.md'], ['notes/a.md', 'notes/b.md'], ['top.md', 'notes/b.md']], hubs: [] }
  const g = knFilterGraph(F, { hidden: ['clients', '(top)'] })
  assert.deepEqual(ids(g), ['notes/a.md', 'notes/b.md'])
  assert.deepEqual(g.edges, [['notes/a.md', 'notes/b.md']])
  assert.equal(knFilterGraph(F, {}).nodes.length, 4, 'none hidden by default')
  const lone = { ...F, nodes: [...F.nodes, { id: 'clients/alone.md', folder: 'clients' }] }
  assert.equal(knIsolatedCount(lone, true), 1)
  assert.equal(knIsolatedCount(lone, true, ['clients']), 0, 'the isolated count is of the pages shown')
  const tagged = { ...lone, nodes: [...lone.nodes.map((n) => n.id === 'clients/alone.md' ? { ...n, tags: ['x'] } : n), { id: '#x', tag: true }], edges: [...lone.edges, ['clients/alone.md', '#x']] }
  assert.equal(knIsolatedCount(tagged, true), 1)
  assert.equal(knIsolatedCount(tagged, true, [], true), 0, 'with tags as nodes, a link to a tag is a link')
})

// ── the graph's live layout ──
const rand = (seed) => () => { seed = (seed + 0x6d2b79f5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296 }
const settle = (sim, max = 2000) => { let n = 0; while (!sim.settled() && n < max) { sim.tick(); n++ } return n }
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y)
const mean = (xs) => xs.reduce((s, x) => s + x, 0) / xs.length

test('graph layout: linked pages settle near each other, unlinked groups apart', () => {
  const sim = knForceSim()
  const grp = (p) => Array.from({ length: 6 }, (_, i) => ({ id: `${p}${i}` }))
  const a = grp('a'), b = grp('b')
  const clique = (g) => g.flatMap((x, i) => g.slice(i + 1).map((y) => [x.id, y.id]))
  sim.set([...a, ...b], [...clique(a), ...clique(b)])
  settle(sim)
  const P = (id) => sim.node(id)
  const inside = mean([...clique(a), ...clique(b)].map(([x, y]) => dist(P(x), P(y))))
  const across = mean(a.flatMap((x) => b.map((y) => dist(P(x.id), P(y.id)))))
  assert.ok(inside * 2 < across, `inside ${inside.toFixed(1)} vs across ${across.toFixed(1)}`)
  assert.ok(sim.nodes().every((n) => Number.isFinite(n.x) && Number.isFinite(n.y)))
})

test('graph layout: it cools and stops by itself; a drag keeps it warm until let go', () => {
  const sim = knForceSim()
  sim.set(Array.from({ length: 30 }, (_, i) => ({ id: `p${i}` })), Array.from({ length: 29 }, (_, i) => [`p${i}`, `p${i + 1}`]))
  const n = settle(sim)
  assert.ok(n > 50 && n < 600, `settled after ${n} ticks`)
  sim.hold(true)
  for (let i = 0; i < 400; i++) sim.tick()
  assert.equal(sim.settled(), false, 'held warm while a page is dragged')
  sim.hold(false)
  assert.ok(settle(sim) < 600)
})

test('graph layout: a dragged page stays under the pointer and pulls its neighbours along', () => {
  const sim = knForceSim()
  sim.set([{ id: 'a' }, { id: 'b' }, { id: 'c' }, { id: 'far' }], [['a', 'b'], ['b', 'c']])
  settle(sim)
  const a = sim.node('a'), b = sim.node('b'), far = sim.node('far')
  const to = { x: a.x + 400, y: a.y }, b0 = { x: b.x, y: b.y }, far0 = { x: far.x, y: far.y }
  sim.fix('a', to.x, to.y)
  sim.hold(true)
  for (let i = 0; i < 120; i++) sim.tick()
  assert.equal(a.x, to.x); assert.equal(a.y, to.y)
  assert.ok(dist(b, to) < dist(b0, to) * 0.6, 'its neighbour came along')
  assert.ok(dist(far, far0) < dist(b, b0), 'an unlinked page moved less')
  sim.free('a'); sim.hold(false)
  assert.equal(a.fx, null, 'let go, it is laid out again')
})

test('graph layout: new data keeps where pages were; one brought back returns, a new one starts by its links', () => {
  const sim = knForceSim()
  sim.set([{ id: 'a' }, { id: 'b' }, { id: 'c' }], [['a', 'b'], ['b', 'c']])
  settle(sim)
  const at = (id) => ({ x: sim.node(id).x, y: sim.node(id).y })
  const a0 = at('a'), c0 = at('c')
  sim.set([{ id: 'a' }, { id: 'b' }], [['a', 'b']])
  assert.deepEqual(at('a'), a0)
  assert.equal(sim.node('c'), null)
  sim.set([{ id: 'a' }, { id: 'b' }, { id: 'c' }, { id: 'd' }], [['a', 'b'], ['b', 'c'], ['a', 'd']])
  assert.deepEqual(at('c'), c0, 'filtered out and back: where it was')
  assert.ok(dist(sim.node('d'), sim.node('a')) < 20, 'a new page starts beside the page it links to')
  settle(sim)
  sim.set([{ id: 'a' }, { id: 'b' }, { id: 'c' }, { id: 'd' }], [['a', 'b'], ['b', 'c'], ['a', 'd']], 0.1)
  assert.ok(sim.alpha() <= 0.1 + 1e-9, 'a file changed on disk: only a nudge, not a new layout')
  sim.forget()
  sim.set([{ id: 'c' }], [])
  assert.notDeepEqual(at('c'), c0, 'forgotten (another page\'s local graph): laid out afresh')
  assert.equal(sim.alpha(), 1)
})

test('graph layout: Barnes–Hut repulsion is close to the exact sum', () => {
  const r = rand(7)
  const mk = () => Array.from({ length: 400 }, (_, i) => ({ id: i, x: r() * 800 - 400, y: r() * 600 - 300, vx: 0, vy: 0 }))
  const exact = mk(), approx = exact.map((n) => ({ ...n }))
  knRepel(exact, 30, 0)
  knRepel(approx, 30, 0.9)
  const err = exact.map((e, i) => Math.hypot(approx[i].vx - e.vx, approx[i].vy - e.vy) / Math.max(1e-9, Math.hypot(e.vx, e.vy))).sort((x, y) => x - y)
  assert.ok(err[Math.floor(err.length / 2)] < 0.05, `median error ${err[200]}`)
  assert.ok(err[Math.floor(err.length * 0.95)] < 0.25, `95th percentile ${err[380]}`)
})

test('graph layout: pages on the same spot are pushed apart, never to NaN', () => {
  const sim = knForceSim()
  sim.set(Array.from({ length: 12 }, (_, i) => ({ id: `p${i}` })), [['p0', 'p1']])
  for (const n of sim.nodes()) { n.x = 0; n.y = 0 }
  for (let i = 0; i < 80; i++) sim.tick()
  const ns = sim.nodes()
  assert.ok(ns.every((n) => Number.isFinite(n.x) && Number.isFinite(n.y)))
  assert.equal(new Set(ns.map((n) => `${n.x.toFixed(3)},${n.y.toFixed(3)}`)).size, ns.length)
  const big = knForceSim()
  big.set(Array.from({ length: 300 }, (_, i) => ({ id: `p${i}` })), [])
  for (const n of big.nodes()) { n.x = 5; n.y = 5 }
  for (let i = 0; i < 40; i++) big.tick()
  assert.ok(big.nodes().every((n) => Number.isFinite(n.x) && Number.isFinite(n.y)), 'the tree path too')
})

test('graph labels: the hovered one always, the rest only where they fit on the canvas', () => {
  const c = (id, x, y, p, r = 3) => ({ id, x, y, w: 60, h: 12, p, r })
  const plan = knLabelPlan([c('small', 100, 100, 1, 2), c('big', 110, 102, 1, 9), c('hover', 105, 101, 3), c('off', 900, 100, 1), c('other', 300, 300, 1)], 500, 400)
  assert.deepEqual(plan.map((l) => l.id), ['hover', 'other'], 'overlapping and off-canvas labels skipped; the hovered one first')
  assert.deepEqual(knLabelPlan([c('a', 100, 100, 3), c('b', 100, 100, 3)], 500, 400).map((l) => l.id), ['a', 'b'], 'must-show labels are never skipped')
  assert.equal(knLabelPlan(Array.from({ length: 50 }, (_, i) => c(`n${i}`, 40 + (i % 5) * 90, 20 + Math.floor(i / 5) * 30, 1)), 500, 400, 10).length, 10, 'at most max')
})

test('graph search lights pages whose title or path has every word', () => {
  const ns = [{ id: 'clients/acme.md', title: 'Acme Corp' }, { id: 'notes/2026-acme-sync.md', title: 'Weekly sync' }, { id: 'topics/pricing.md', title: 'Pricing for Acme' }]
  assert.deepEqual([...knGraphHits(ns, 'acme')].sort(), ['clients/acme.md', 'notes/2026-acme-sync.md', 'topics/pricing.md'])
  assert.deepEqual([...knGraphHits(ns, 'ACME  pricing')], ['topics/pricing.md'])
  assert.equal(knGraphHits(ns, '  '), null, 'nothing typed: nothing lit')
  assert.equal(knGraphHits(ns, 'zzz').size, 0)
})

test('graph labels: long titles are shortened, short ones kept', () => {
  assert.equal(knShortLabel('Acme'), 'Acme')
  const long = 'AI/Run Hyperfactory — the operating model and the Control Plane'
  assert.equal(knShortLabel(long), 'AI/Run Hyperfactory — the ope…')
  assert.equal([...knShortLabel(long)].length, 30)
  assert.equal(knShortLabel('Клиент «Север» — еженедельная встреча по поставке', 12), 'Клиент «Сев…')
})

test('unsaved text is kept for the same page and base, gone after a save', () => {
  const store = new Map()
  const s = { getItem: (k) => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k) }
  knKeepDraft(s, 'my-twin', 'knowledge/a.md', 'base1', '# draft')
  assert.equal(knRestoreDraft(s, 'my-twin', 'knowledge/a.md', 'base1'), '# draft')
  assert.equal(knRestoreDraft(s, 'my-twin', 'knowledge/a.md', 'base2'), null, 'the file changed since: not restored over it')
  knDropDraft(s, 'my-twin', 'knowledge/a.md')
  assert.equal(knRestoreDraft(s, 'my-twin', 'knowledge/a.md', 'base1'), null)
  const broken = { getItem: () => { throw new Error('off') }, setItem: () => { throw new Error('off') }, removeItem: () => {} }
  assert.equal(knRestoreDraft(broken, 'v', 'p', 'b'), null, 'storage off: nothing, no error')
  knKeepDraft(broken, 'v', 'p', 'b', 't')
})

test('Knowledge\'s buttons use attributes the app\'s own click handler leaves alone', () => {
  const app = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
  const sel = app.split('\n').find((l) => l.includes("e.target.closest('[data-me-edit]"))
  assert.ok(sel, 'the app\'s click handler selector not found in app.js')
  const owned = new Set([...sel.matchAll(/\[(data-[a-z-]+)\]/g)].map((m) => m[1]))
  const used = new Set([...src.matchAll(/\b(data-[a-z-]+)=/g)].map((m) => m[1]))
  const both = [...used].filter((a) => owned.has(a) && a !== 'data-copy') // data-copy: the app's Copy, on purpose
  assert.deepEqual(both, [], `a click on these would also run the app's handler: ${both.join(', ')}`)
})

test('compare: the lines only in yours, only on disk, and the same ones folded', () => {
  assert.deepEqual(knLineDiff('a\nb\nc', 'a\nc\nd'), [['=', 'a'], ['-', 'b'], ['=', 'c'], ['+', 'd']])
  assert.deepEqual(knLineDiff('same', 'same'), [['=', 'same']])
  const mine = Array.from({ length: 20 }, (_, i) => `l${i}`).join('\n')
  const theirs = mine + '\nadded outside'
  const rows = knLineDiff(mine, theirs, 2)
  assert.deepEqual(rows[0], ['…', '18 lines the same'])
  assert.deepEqual(rows.slice(1), [['=', 'l18'], ['=', 'l19'], ['+', 'added outside']])
  const big = Array.from({ length: 3000 }, (_, i) => `x${i}`).join('\n')
  assert.equal(knLineDiff(big, big.replace(/x/g, 'z')), null, 'too big to compare here: none')
  assert.equal(knLineDiff(big, big + '\ny').filter(([o]) => o === '+').length, 1, 'a long page with one change is fine')
})

test('no confirm(), alert() or prompt(): the window\'s webview shows none of them', () => {
  const code = src.split('\n').filter((l) => !/^\s*(\/\/|\/\*\*|\*)/.test(l))
  const calls = code.filter((l) => /(^|[^.\w])(confirm|alert|prompt)\(/.test(l))
  assert.deepEqual(calls, [], 'ask() instead, in the page')
})

test('no two functions share a name: the later one would silently win', () => {
  const names = [...src.matchAll(/^\s*(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\s*\(/gm)].map((m) => m[1])
  const twice = [...new Set(names.filter((n, i) => names.indexOf(n) !== i))]
  assert.deepEqual(twice, [])
})

test('a graph over 5,000 pages starts with the open page\'s neighbours', () => {
  assert.equal(knGraphStart(5000, 'knowledge/a.md', 'knowledge/CATALOG.md'), null, 'up to 5,000: the whole graph')
  assert.deepEqual(knGraphStart(5001, 'knowledge/a.md', 'knowledge/CATALOG.md'), { depth: 1, sel: 'knowledge/a.md' })
  assert.deepEqual(knGraphStart(9000, '', 'knowledge/CATALOG.md'), { depth: 1, sel: 'knowledge/CATALOG.md' }, 'no page open: the index')
})

test('text typed into a session has no control characters', () => {
  assert.equal(knPlainPrompt('/twin What about notes/a\rb.md?\n'), '/twin What about notes/a b.md?')
  assert.equal(knPlainPrompt('/run-skill\u001b[2J'), '/run-skill [2J')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`knowledge: ${tests.length} passed`)
