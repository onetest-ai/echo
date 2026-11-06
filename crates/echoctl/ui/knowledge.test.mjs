// Knowledge in the window (knowledge.js): routes, the switcher, a call's Knowledge chip, the tree, the graph's filters,
// unsaved edits and Ask (turns, tool rows, @ mentions, following the bottom) — the blocks as they ship. Run by scripts/check-mod.sh (`node crates/echoctl/ui/knowledge.test.mjs`).
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
const { knTabCounts } = load(['kn-tabs'], ['knTabCounts'])
const { knSwitcherEntries } = load(['kn-switcher'], ['knSwitcherEntries'])
const { knChipState } = load(['kn-chip'], ['knChipState'])
const { knBuildTree } = load(['kn-tree'], ['knBuildTree'])
const { knFilterGraph, knIsolatedCount, knGraphStart } = load(['kn-graph'], ['knFilterGraph', 'knIsolatedCount', 'knGraphStart'])
const { knRepel, knForceSim, knLabelPlan, knGraphHits, knShortLabel } = load(['kn-force'], ['knRepel', 'knForceSim', 'knLabelPlan', 'knGraphHits', 'knShortLabel'])
const { knPlainPrompt } = load(['kn-prompt'], ['knPlainPrompt'])
const { knKeepDraft, knRestoreDraft, knDropDraft, knEditText } = load(['kn-draft'], ['knKeepDraft', 'knRestoreDraft', 'knDropDraft', 'knEditText'])
const { knLineDiff } = load(['kn-diff'], ['knLineDiff'])
const { knChatTurns, knChatReadOutside, knFollow, knAnswerInto, knAnswerMd, knWaitingCard, knChatKey, knSentLanded } = load(['kn-chat'], ['knChatTurns', 'knChatReadOutside', 'knFollow', 'knAnswerInto', 'knAnswerMd', 'knWaitingCard', 'knChatKey', 'knSentLanded'])
const { knChatOpen, knPollKeep, knChatTake } = load(['kn-poll'], ['knChatOpen', 'knPollKeep', 'knChatTake'])
const { knToolRows } = load(['kn-tools'], ['knToolRows'])
const { knMention, knMentionPick, knChatMessage } = load(['kn-mention'], ['knMention', 'knMentionPick', 'knChatMessage'])

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

test('tab_counts_only_where_something_waits', () => {
  assert.deepEqual(knTabCounts({ waiting: 0, needs: 0 }), { ingest: null, health: null }, 'no number at 0')
  assert.deepEqual(knTabCounts({ waiting: 6, needs: 3 }), { ingest: 6, health: 3 })
  assert.deepEqual(knTabCounts({ waiting: 6, needs: 0 }), { ingest: 6, health: null })
  assert.deepEqual(knTabCounts({}), { ingest: null, health: null }, 'missing counts: none')
  assert.deepEqual(knTabCounts(), { ingest: null, health: null })
  const c = knTabCounts({ waiting: 6, needs: 3 })
  for (const tab of ['pages', 'graph', 'settings', 'ask', 'home']) assert.equal(c[tab], undefined, `${tab} never shows a number`)
})

// ── Ask (spec K13) ──
test('routes: Ask, a new conversation or one of them', () => {
  assert.deepEqual(knParseRoute('kn/my-twin/ask'), { vault: 'my-twin', tab: 'ask', path: '' })
  const id = '6f2c9e1a-1b2c-4d5e-8f90-123456789abc'
  assert.deepEqual(knParseRoute(`kn/my-twin/ask/${id}`), { vault: 'my-twin', tab: 'ask', path: id })
  assert.equal(knRouteOf({ vault: 'my-twin', tab: 'ask', path: id }), `kn/my-twin/ask/${id}`)
  assert.equal(knParseRoute('kn/my-twin/ingest'), null, 'a tab appears only when its view works')
})

const Q = 'What did Lena Hartmann and I settle on Northwind Retail\'s pricing?'
const TURN = [
  { n: 0, type: 'notice', kind: 'history', text: 'Its history can\'t be shown here; continuing works.' },
  { n: 1, type: 'user', text: Q, context: [{ page: 'knowledge/clients/northwind-retail.md' }] },
  { n: 2, type: 'thinking', block: 'b1', done: false, superseded: true },
  { n: 3, type: 'thinking', block: 'b1', text: 'A decision question — start from the index.', done: true, seconds: 3 },
  { n: 4, type: 'tool', id: 'toolu_01', name: 'Read', verb: 'Read', target: 'knowledge/CATALOG.md', detail: 'lines 1–180', path: 'knowledge/CATALOG.md', readonly: true },
  { n: 5, type: 'tool_result', id: 'toolu_01', ok: true, summary: '180 lines', preview: '| Page | Summary |', lines: [1, 180] },
  { n: 6, type: 'text', block: 'b2', html: '<p>From your</p>', done: false },
  { n: 7, type: 'text', block: 'b2', html: '<p>From your knowledge base: two things.</p>', done: true },
  { n: 8, type: 'permission', request: 'req_7', tool: 'Edit', path: 'knowledge/clients/northwind-retail.md', hunks: [[['=', '## Decisions'], ['+', '- 2026-11-05 — Fix forward, no rollback.']]], plus: 1, minus: 0 },
  { n: 9, type: 'answered', request: 'req_7', allow: true, always: false },
  { n: 10, type: 'result', tokens_in: 18200, tokens_out: 1100, cache_read: 0.61, cost: 0.21, seconds: 38, stop: 'end_turn' },
  { n: 11, type: 'suggestion', text: 'Which of these decisions has an owner?' },
]

test('K13-T1 a conversation folds into turns: the message, thinking, tools, text, a card, the result', () => {
  const turns = knChatTurns(TURN)
  assert.equal(turns.length, 2, 'what came before any message (the history notice), then one turn')
  assert.equal(turns[0].user, null)
  assert.deepEqual(turns[0].blocks.map((b) => [b.kind, b.text]), [['notice', 'Its history can\'t be shown here; continuing works.']])
  const t = turns[1]
  assert.equal(t.user.text, Q)
  assert.deepEqual(t.user.context, [{ page: 'knowledge/clients/northwind-retail.md' }])
  assert.deepEqual(t.blocks.map((b) => b.kind), ['thinking', 'tools', 'text', 'permission'], 'in order; superseded renders skipped')
  assert.deepEqual([t.blocks[0].text, t.blocks[0].done, t.blocks[0].seconds], ['A decision question — start from the index.', true, 3])
  assert.equal(t.blocks[1].items.length, 1)
  assert.equal(t.blocks[1].items[0].result.summary, '180 lines', 'a result joins its call')
  assert.deepEqual([t.blocks[2].html, t.blocks[2].done], ['<p>From your knowledge base: two things.</p>', true], 'the later render of a block replaces it')
  assert.deepEqual(t.blocks[3].answer && [t.blocks[3].answer.allow, t.blocks[3].answer.always], [true, false], 'the card knows its answer')
  assert.deepEqual([t.result.tokens_in, t.result.cost], [18200, 0.21])
  assert.equal(t.suggestion, 'Which of these decisions has an owner?')
})

test('K13-T1 a streaming block is one block; a finished one is never replaced by a later process\'s block of that name', () => {
  const live = knChatTurns([TURN[1], { n: 2, type: 'text', block: 'b1', html: '<p>Two</p>', done: false }, { n: 3, type: 'text', block: 'b1', html: '<p>Two things</p>', done: false }])
  assert.deepEqual(live[0].blocks.map((b) => [b.kind, b.html, b.done]), [['text', '<p>Two things</p>', false]])
  assert.equal(live[0].result, null, 'still answering')
  // A mode change restarts the process: its blocks are named b1… again.
  const again = knChatTurns([...TURN, { n: 12, type: 'user', text: 'And the gateway?', context: [] }, { n: 13, type: 'text', block: 'b2', html: '<p>Still open.</p>', done: true }])
  assert.equal(again.length, 3)
  assert.equal(again[1].blocks[2].html, '<p>From your knowledge base: two things.</p>', 'the first answer stays')
  assert.deepEqual(again[2].blocks.map((b) => b.html), ['<p>Still open.</p>'])
  const sub = knChatTurns([TURN[1], { n: 2, type: 'text', block: 'b1', html: '<p>Sub-agent words</p>', done: true, parent: 'toolu_09' }])
  assert.deepEqual(sub[0].blocks, [], 'a sub-agent\'s own text is not the answer')
  const history = knChatTurns([{ n: 0, type: 'user', text: 'Hi', context: [] }, { n: 1, type: 'text', block: 'h1', html: '<p>Hello</p>', done: true }, { n: 2, type: 'user', text: 'Again', context: [] }, { n: 3, type: 'text', block: 'b1', html: '<p>Hello again</p>', done: false }])
  assert.deepEqual(history.map((x) => x.blocks.map((b) => b.block)), [['h1'], ['b1']])
})

test('K13-T1 notices and errors stay in their turn; D41 knows when a conversation read the web or others\' words', () => {
  const t = knChatTurns([TURN[1], { n: 2, type: 'error', text: 'This Claude Code doesn\'t hand permission prompts to Echo', action: 'terminal' }, { n: 3, type: 'notice', kind: 'committed', text: 'Committed 1 file · 3f9c2d1' }])
  assert.deepEqual(t[0].blocks.map((b) => [b.kind, b.action || b.text]), [['error', 'terminal'], ['notice', 'Committed 1 file · 3f9c2d1']])
  assert.equal(knChatReadOutside(TURN), false, 'pages only')
  assert.equal(knChatReadOutside([{ n: 0, type: 'user', text: 'Brief me', context: [{ call: '2026-11-05-0900-northwind-weekly', title: 'Northwind weekly', date: '2026-11-05' }] }]), true, 'an attached call')
  assert.equal(knChatReadOutside([{ n: 0, type: 'tool', id: 't', name: 'WebSearch', verb: 'Searched the web', target: '"pricing"', readonly: true }]), true, 'Research searched')
  assert.equal(knChatReadOutside([{ n: 0, type: 'tool', id: 't', name: 'WebFetch', verb: 'Opened', target: 'example.com/x', readonly: true }]), true)
})

const T = (id, name, verb, target, detail, result) => ({ type: 'tool', id, name, verb, target, detail, readonly: !['Edit', 'Write', 'Agent'].includes(name), result: result === undefined ? { ok: true, summary: 'done' } : result })
const ok = (summary) => ({ ok: true, summary })
const line = (item, o) => { const r = knToolRows([item], o); assert.equal(r.length, 1); return r[0] }

test('K13-T2 a tool call is one line: verb, target and result', () => {
  assert.equal(line(T('1', 'Read', 'Read', 'knowledge/CATALOG.md', 'lines 1–180', ok('180 lines'))).text, 'Read knowledge/CATALOG.md · lines 1–180')
  assert.equal(line(T('1', 'Read', 'Read', 'knowledge/notes/2026-10-14-architecture-review.md', '', ok('96 lines'))).text, 'Read knowledge/notes/2026-10-14-architecture-review.md · 96 lines', 'a whole file: how many lines')
  assert.equal(line(T('2', 'Grep', 'Searched', '"northwind"', 'in knowledge/notes/', ok('6 files'))).text, 'Searched "northwind" in knowledge/notes/ · 6 files')
  assert.equal(line(T('3', 'Glob', 'Searched', 'knowledge/**/*.md', '', ok('12 files'))).text, 'Searched knowledge/**/*.md · 12 files')
  assert.equal(line(T('4', 'Skill', 'Used skill', 'twin', '', ok('Launching skill: twin'))).text, 'Used skill twin')
  assert.equal(line(T('5', 'Edit', 'Edited', 'knowledge/clients/northwind-retail.md', '+4 −1', ok('The file has been updated.'))).text, 'Edited knowledge/clients/northwind-retail.md +4 −1')
  assert.equal(line(T('6', 'Write', 'Created', 'knowledge/topics/pricing.md', '12 lines', ok('File created'))).text, 'Created knowledge/topics/pricing.md · 12 lines')
  assert.equal(line(T('7', 'WebSearch', 'Searched the web', '"per-stream pricing delivery platform"', '', ok('Web search results for query'))).text, 'Searched the web "per-stream pricing delivery platform"', 'no result count in the events')
  assert.equal(line(T('8', 'WebFetch', 'Opened', 'example.com/reports/pricing', '', ok('# Pricing'))).text, 'Opened example.com/reports/pricing')
  assert.equal(line(T('9', 'Agent', 'Delegated', 'Find the pricing notes', '', ok('Found two notes'))).text, 'Delegated Find the pricing notes')
  const r = line(T('1', 'Read', 'Read', 'knowledge/clients/gone.md', '', { ok: false, summary: 'File does not exist.' }))
  assert.deepEqual([r.text, r.error], ['Read knowledge/clients/gone.md · File does not exist.', 'File does not exist.'], 'a failed call says why')
  assert.equal(line(T('1', 'Read', 'Read', 'knowledge/CATALOG.md', '', null)).running, true, 'no result yet: running')
  assert.equal(line(T('1', 'Read', 'Read', 'knowledge/CATALOG.md', '', null), { ended: true }).running, false, 'the turn ended without one')
  assert.equal(line(T('1', 'Read', 'Read', 'knowledge/CATALOG.md', '', ok('3 lines'))).code, true, 'a path is shown as code')
})

test('K13-T2 three or more read-only calls in a row fold into one line', () => {
  const read = (i) => T(`r${i}`, 'Read', 'Read', `knowledge/notes/${i}.md`, '', ok('9 lines'))
  const three = knToolRows([read(1), read(2), read(3)])
  assert.deepEqual(three.map((r) => [r.kind, r.text]), [['fold', 'Read 3 files']])
  assert.equal(three[0].rows.length, 3, 'it opens to its calls')
  const grep = (i) => T(`g${i}`, 'Grep', 'Searched', '"pricing"', '', ok('2 files'))
  assert.equal(knToolRows([read(1), read(2), grep(1), read(3), grep(2)])[0].text, 'Read 3 files, searched 2 times')
  const edit = T('e', 'Edit', 'Edited', 'knowledge/a.md', '+1 −0')
  assert.deepEqual(knToolRows([read(1), read(2), edit]).map((r) => r.kind), ['row', 'row', 'row'], 'two reads then an edit don\'t fold')
  assert.deepEqual(knToolRows([read(1), edit, read(2), read(3), read(4)]).map((r) => r.kind), ['row', 'row', 'fold'])
  const running = knToolRows([read(1), read(2), { ...read(3), result: null }])
  assert.equal(running[0].running, true, 'a fold with a call still running spins')
})

test('K13-T3 @ opens the page picker; picking puts the page in a chip; the message carries its path', () => {
  assert.deepEqual(knMention('What about @cli', 15), { query: 'cli', start: 11 })
  assert.deepEqual(knMention('@', 1), { query: '', start: 0 }, 'just typed: the picker with nothing found yet')
  assert.equal(knMention('Mail lena@example.com', 21), null, 'an address is no mention')
  assert.equal(knMention('@cli and more', 13), null, 'a space ends it')
  assert.equal(knMention('No mention here', 15), null)
  const text = 'Risks with @north today'
  const m = knMention(text, 17)
  assert.deepEqual(m, { query: 'north', start: 11 })
  const p = knMentionPick(text, m, 17, 'Northwind Retail')
  assert.deepEqual(p, { text: 'Risks with Northwind Retail today', caret: 27 })
  const msg = knChatMessage(p.text, [{ page: 'knowledge/clients/northwind-retail.md', title: 'Northwind Retail' }])
  assert.deepEqual(msg, { text: 'Risks with Northwind Retail today', context: [{ page: 'knowledge/clients/northwind-retail.md' }] })
  assert.ok(!msg.text.includes('@'), 'the text without the mention mark')
  assert.deepEqual(knChatMessage('  Brief me\n', [{ call: '2026-11-05-0900-northwind-weekly', title: 'Northwind weekly' }, { call: '2026-11-04-1400-helios-review', anyway: true }]), {
    text: 'Brief me', context: [{ call: '2026-11-05-0900-northwind-weekly' }, { call: '2026-11-04-1400-helios-review', anyway: true }],
  }, 'calls by their names, never a path; *Attach anyway* only on its call')
})

test('K13-T4 the view follows the bottom only while you are there', () => {
  const bottom = { top: 800, height: 1400, view: 600 }
  assert.deepEqual(knFollow(bottom, { n: 9 }), { scroll: true, jump: false })
  const up = { top: 600, height: 1400, view: 600 }
  assert.deepEqual(knFollow(up, { n: 9 }), { scroll: false, jump: true }, 'scrolled up 200 px: stays, "Jump to the latest" shows')
  assert.deepEqual(knFollow({ ...up, jump: true }, null), { scroll: false, jump: true }, 'scrolling while up keeps it')
  assert.deepEqual(knFollow({ ...bottom, jump: true }, null), { scroll: false, jump: false }, 'back at the bottom: gone')
  assert.deepEqual(knFollow({ top: 0, height: 300, view: 600 }, { n: 1 }), { scroll: true, jump: false }, 'nothing to scroll yet')
})

test('an answer saved as a page carries its source; added to a page, it goes under a dated heading', () => {
  const page = '---\ntitle: Pricing\ndate: 2026-11-06\ntags: []\nsource: \nsummary: ""\n---\n\n# Pricing\n'
  assert.equal(knAnswerInto(page, '- A platform fee.', { source: 'echo-ask 6f2c9e1a' }), '---\ntitle: Pricing\ndate: 2026-11-06\ntags: []\nsource: echo-ask 6f2c9e1a\nsummary: ""\n---\n\n# Pricing\n\n- A platform fee.\n')
  assert.equal(knAnswerInto('# Pricing\n', 'Text.', { source: 'echo-ask x' }), '---\nsource: echo-ask x\n---\n\n# Pricing\n\nText.\n', 'no properties yet')
  assert.equal(knAnswerInto('# Northwind Retail\n\nA client.\n\n', 'Two things.', { heading: '2026-11-06 — from Ask' }), '# Northwind Retail\n\nA client.\n\n## 2026-11-06 — from Ask\n\nTwo things.\n')
})

// ── Ask, fix round 1 ──
const A = [
  { n: 0, type: 'user', text: 'What did Lena Hartmann decide?', context: [] },
  { n: 1, type: 'text', block: 'b1', html: '<p>Two things.</p>', md: 'Two things.', done: true },
  { n: 2, type: 'result', tokens_in: 10, tokens_out: 5, seconds: 2 },
]
const A2 = [
  { n: 3, type: 'user', text: 'And the gateway?', context: [] },
  { n: 4, type: 'text', block: 'b1', html: '<p>Still open.</p>', md: 'Still open.', done: true },
  { n: 5, type: 'result', tokens_in: 12, tokens_out: 4, seconds: 1 },
]
const chunk = (events, next, o = {}) => ({ events: events.map((e) => ({ ...e })), next, live: true, busy: false, waiting: false, ...o })

test('the poll: A → New → A within 25 s loads A once, the old request is dropped', () => {
  const c = { busy: false }
  const g1 = knChatOpen(c, 'A') // its loop asks from 0 …
  const g2 = knChatOpen(c, '')
  assert.equal(c.polling, null, 'opening another one lets a new loop start')
  const g3 = knChatOpen(c, 'A')
  assert.ok(g1 !== g2 && g2 !== g3)
  assert.equal(knPollKeep(c, g1), false, 'the first loop stops')
  assert.equal(knPollKeep(c, g3), true)
  assert.ok(knChatTake(c, g3, 0, chunk([...A, ...A2], 6), 1000))
  assert.equal(knChatTake(c, g1, 0, chunk(A, 3), 2000), null, '… and its answer, coming back late, is dropped')
  assert.deepEqual(c.events.map((e) => e.n), [0, 1, 2, 3, 4, 5])
  assert.equal(knChatTurns(c.events).length, 2, 'A\'s earlier turn is there')
})

test('the poll: A → B → A within 25 s polls A once; a chunk is merged once', () => {
  const c = { busy: false }
  const g1 = knChatOpen(c, 'A')
  assert.ok(knChatTake(c, g1, 0, chunk(A, 3), 1000))
  knChatOpen(c, 'B')
  const g3 = knChatOpen(c, 'A')
  assert.deepEqual(c.events, [], 'A again starts afresh')
  assert.equal(knChatTake(c, g1, 3, chunk(A2, 6), 2000), null, 'the first loop\'s request (from 3) is stale')
  assert.ok(knChatTake(c, g3, 0, chunk([...A, ...A2], 6), 3000))
  assert.equal(knChatTake(c, g3, 0, chunk([...A, ...A2], 6), 3000), null, 'asked from 0, but the view is at 6 now: not merged twice')
  assert.deepEqual(c.events.map((e) => e.n), [0, 1, 2, 3, 4, 5], 'no duplicates, no "Echo started again" reset')
  assert.deepEqual(knChatTurns(c.events).map((t) => t.user.text), ['What did Lena Hartmann decide?', 'And the gateway?'])
  // Echo started again: asked from 6, it answers from its first event — the list is rebuilt from it.
  assert.ok(knChatTake(c, g3, 6, chunk([{ n: 0, type: 'notice', kind: 'history', text: 'Its history can\'t be shown here; continuing works.' }], 1), 4000))
  assert.deepEqual(c.events.map((e) => e.type), ['notice'])
})

test('the poll: a new message starts the blocks\' names afresh (a cut answer keeps its text)', () => {
  const c = { busy: false }
  const g = knChatOpen(c, 'A')
  knChatTake(c, g, 0, chunk([A[0], { n: 1, type: 'text', block: 'b1', html: '<p>Half an ans</p>', done: false }, { n: 2, type: 'notice', kind: 'stopped', text: 'Stopped' }], 3), 0)
  knChatTake(c, g, 3, chunk([A2[0], { n: 4, type: 'text', block: 'b1', html: '<p>New</p>', done: false }], 5, { busy: true }), 0)
  const t = knChatTurns(c.events)
  assert.deepEqual(t.map((x) => x.blocks.filter((b) => b.kind === 'text').map((b) => b.html)), [['<p>Half an ans</p>'], ['<p>New</p>']])
  // Within a turn a block's older render drops its copy.
  knChatTake(c, g, 5, chunk([{ n: 5, type: 'text', block: 'b1', html: '<p>New text</p>', done: false }], 6, { busy: true }), 0)
  assert.equal(c.events[4].html, undefined)
  assert.equal(c.events[4].superseded, true)
})

test('⌘↵ in a composer with text never answers a card; Esc denies, then stops; nothing while composing', () => {
  const k = (o) => knChatKey({ key: 'Enter', mod: true, ...o })
  assert.equal(k({ card: true, inComposer: true, typed: true }), 'blocked', 'the "send" keystroke doesn\'t approve an edit')
  assert.equal(k({ card: true, inComposer: true, typed: false }), 'allow', 'an empty composer answers the card')
  assert.equal(k({ card: true, inComposer: false }), 'allow')
  assert.equal(k({ card: false, inComposer: true, typed: true }), 'send')
  assert.equal(k({ card: false, inComposer: false }), null)
  assert.equal(knChatKey({ key: 'Enter', mod: false, inComposer: true, typed: true }), null, 'Enter alone is a new line')
  assert.equal(knChatKey({ key: 'Escape', card: true, busy: true }), 'deny')
  assert.equal(knChatKey({ key: 'Escape', busy: true }), 'stop')
  assert.equal(knChatKey({ key: 'Escape' }), null)
  assert.equal(k({ card: true, composing: true }), null, 'an input method composing')
  assert.equal(knChatKey({ key: 'Escape', busy: true, composing: true }), null)
})

test('the card waiting: answered by Echo or from this page, it waits no more', () => {
  const ev = [A[0], { n: 1, type: 'permission', request: 'req_1', tool: 'Edit', path: 'knowledge/a.md', hunks: [], plus: 1, minus: 0 }, { n: 2, type: 'permission', request: 'req_2', tool: 'WebFetch', url: 'https://example.com/a', domain: 'example.com' }]
  const turns = knChatTurns(ev)
  assert.equal(knWaitingCard(turns, true).request, 'req_1')
  assert.equal(knWaitingCard(turns, true, { req_1: { allow: true } }).request, 'req_2', 'a second ⌘↵ goes to the next card, not the same one again')
  assert.equal(knWaitingCard(turns, true, { req_1: { allow: true }, req_2: { allow: false } }), null)
  assert.equal(knWaitingCard(turns, false), null, 'no process: nothing waits')
  assert.equal(knWaitingCard(knChatTurns([...ev, { n: 3, type: 'answered', request: 'req_1', allow: true, always: false }]), true).request, 'req_2')
})

test('Copy and Save use the answer\'s Markdown as Claude wrote it (md), not its HTML', () => {
  const md = '1. Deployment\n   - hosted\n\n   ```\n   fix forward\n   ```\n2. Pricing'
  const t = knChatTurns([A[0], { n: 1, type: 'text', block: 'b1', html: '<ol><li>flattened</li></ol>', md, done: true }, { n: 2, type: 'tool', id: 't', name: 'Read', verb: 'Read', target: 'a.md', readonly: true }, { n: 3, type: 'text', block: 'b3', html: '<p>And more.</p>', md: 'And more.', done: true }])
  assert.equal(knAnswerMd(t[0]), `${md}\n\nAnd more.`)
  assert.equal(knAnswerMd(knChatTurns([A[0], { n: 1, type: 'text', block: 'b1', html: '<p>Half</p>', done: false }])[0]), '', 'a block still streaming has none')
})

test('Add to a page starts from the page\'s kept unsaved text, as Edit does (it never writes over it)', () => {
  const store = new Map()
  const s = { getItem: (k) => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)), removeItem: (k) => store.delete(k) }
  knKeepDraft(s, 'my-twin', 'knowledge/clients/northwind-retail.md', 'base1', '# Northwind Retail\n\nMy unsaved line.\n')
  assert.equal(knEditText(s, 'my-twin', 'knowledge/clients/northwind-retail.md', 'base1', '# Northwind Retail\n'), '# Northwind Retail\n\nMy unsaved line.\n')
  assert.equal(knEditText(s, 'my-twin', 'knowledge/clients/northwind-retail.md', 'base2', '# Northwind Retail\n'), '# Northwind Retail\n', 'changed on disk since: the file')
  assert.equal(knEditText(null, 'my-twin', 'knowledge/a.md', 'b', '# A\n'), '# A\n', 'storage off')
})

test('a message sent stays in the composer until it lands; a sign-in error keeps it', () => {
  assert.equal(knSentLanded(null, [A[0]]), null, 'nothing sent from here')
  assert.equal(knSentLanded({ text: 'What did Lena Hartmann decide?' }, [A[0], A[1]]), 'landed')
  assert.equal(knSentLanded({ text: 'Hi' }, [{ n: 0, type: 'error', text: 'Claude · Work needs you to sign in again', action: 'signin' }]), 'failed')
  assert.equal(knSentLanded({ text: 'Hi' }, []), null, 'not yet')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`knowledge: ${tests.length} passed`)
