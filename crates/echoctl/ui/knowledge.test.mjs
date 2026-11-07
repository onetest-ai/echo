// Knowledge in the window (knowledge.js): routes, the switcher, a call's Knowledge chip, the tree, the graph's filters,
// unsaved edits, Ask (turns, tool rows, @ mentions, following the bottom), Health (findings, the trend, fixes) and Ingest (the queue, a run, the review, the Add a source sheet) — the blocks as they ship. Run by scripts/check-mod.sh (`node crates/echoctl/ui/knowledge.test.mjs`).
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
const { knTabCounts, knTabLabel } = load(['kn-tabs'], ['knTabCounts', 'knTabLabel'])
const { knCallsRule } = load(['kn-calls-rule'], ['knCallsRule'])
const { knSwitcherEntries } = load(['kn-switcher'], ['knSwitcherEntries'])
const { knChipState } = load(['kn-chip'], ['knChipState'])
const { knBuildTree, knPagesEmpty } = load(['kn-tree'], ['knBuildTree', 'knPagesEmpty'])
const { knFilterGraph, knIsolatedCount, knGraphStart } = load(['kn-graph'], ['knFilterGraph', 'knIsolatedCount', 'knGraphStart'])
const { knRepel, knForceSim, knLabelPlan, knGraphHits, knShortLabel } = load(['kn-force'], ['knRepel', 'knForceSim', 'knLabelPlan', 'knGraphHits', 'knShortLabel'])
const { knPlainPrompt } = load(['kn-prompt'], ['knPlainPrompt'])
const { knKeepDraft, knRestoreDraft, knDropDraft, knEditText } = load(['kn-draft'], ['knKeepDraft', 'knRestoreDraft', 'knDropDraft', 'knEditText'])
const { knLineDiff } = load(['kn-diff'], ['knLineDiff'])
const { knChatTurns, knChatReadOutside, knFollow, knAnswerInto, knAnswerMd, knWaitingCard, knChatKey, knSentLanded } = load(['kn-chat'], ['knChatTurns', 'knChatReadOutside', 'knFollow', 'knAnswerInto', 'knAnswerMd', 'knWaitingCard', 'knChatKey', 'knSentLanded'])
const { knChatOpen, knPollKeep, knChatTake, knSentWhere } = load(['kn-poll'], ['knChatOpen', 'knPollKeep', 'knChatTake', 'knSentWhere'])
const { knToolRows } = load(['kn-tools'], ['knToolRows'])
const { knMention, knMentionPick, knChatMessage } = load(['kn-mention'], ['knMention', 'knMentionPick', 'knChatMessage'])
const { knHealthList, knHealthSelect, knHealthTrend, knTrendPath, knFixAll, knFixSummary, knHealthLoad, knHealthState, knLintNotice, knLintState, knVersus, KN_HEALTH_KINDS } = load(['kn-health'], ['knHealthList', 'knHealthSelect', 'knHealthTrend', 'knTrendPath', 'knFixAll', 'knFixSummary', 'knHealthLoad', 'knHealthState', 'knLintNotice', 'knLintState', 'knVersus', 'KN_HEALTH_KINDS'])
const { knIngestQueue, knIngestWaiting, knRunLine, knRunView, knIngestReview, knWriteSummary, knAddState, knAddArgs, knFirstOpen, knSourceState, knTakesDrops, knDropNote } = load(['kn-ingest'], ['knIngestQueue', 'knIngestWaiting', 'knRunLine', 'knRunView', 'knIngestReview', 'knWriteSummary', 'knAddState', 'knAddArgs', 'knFirstOpen', 'knSourceState', 'knTakesDrops', 'knDropNote'])
const { knRunOpen, knRunMayLoad, knRunTake } = load(['kn-ingest-poll'], ['knRunOpen', 'knRunMayLoad', 'knRunTake'])
const { knWhen, knHomeCards, knHomeNews, knHomeLine, knHomeAsk, knHomeRecord } = load(['kn-health', 'kn-home'], ['knWhen', 'knHomeCards', 'knHomeNews', 'knHomeLine', 'knHomeAsk', 'knHomeRecord'])

const tests = []
const test = (name, f) => tests.push([name, f])

test('settings: the new-calls rule offers Off, Add to the inbox, Add and prepare changes, one checked', () => {
  const labels = knCallsRule('add').map((o) => o.label)
  assert.deepEqual(labels, ['Off', 'Add to the inbox', 'Add and prepare changes'])
  assert.deepEqual(knCallsRule('prepare').map((o) => o.on), [false, false, true])
  // The default, and a value Echo doesn't know: Off.
  assert.deepEqual(knCallsRule(undefined).map((o) => o.k + (o.on ? '*' : '')), ['off*', 'add', 'prepare'])
  assert.equal(knCallsRule('always').find((o) => o.on).k, 'off')
  // Prepare says its changes wait for review.
  assert.match(knCallsRule('off')[2].sub, /wait for your review/)
})

test('routes: a vault, its tab, its page; Recall stays #brain', () => {
  assert.deepEqual(knParseRoute('kn/my-twin/pages/knowledge%2Fclients%2Facme.md'), { vault: 'my-twin', tab: 'pages', path: 'knowledge/clients/acme.md' })
  assert.deepEqual(knParseRoute('kn/my-twin/graph'), { vault: 'my-twin', tab: 'graph', path: '' })
  // spec-2 K10-01: a vault opens on Home now (slice 1 opened it on Pages) — changed on purpose.
  assert.deepEqual(knParseRoute('kn/my-twin'), { vault: 'my-twin', tab: 'home', path: '' })
  assert.deepEqual(knParseRoute('kn'), { vault: '', tab: 'home', path: '' })
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
  assert.deepEqual(knParseRoute('kn/my-twin/home'), { vault: 'my-twin', tab: 'home', path: '' }, 'Home is a tab now that its view works')
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

test('the poll: a conversation whose events Echo let go polls on, and opened again shows them once (final review F5)', () => {
  const c = { busy: false }
  const g1 = knChatOpen(c, 'A')
  assert.ok(knChatTake(c, g1, 0, chunk(A, 3, { live: false }), 1000))
  // Echo let its events go: the page still showing them asks from 3, and nothing comes.
  const got = knChatTake(c, g1, 3, chunk([], 3, { live: false }), 2000)
  assert.deepEqual(got.events, [])
  assert.deepEqual(c.events.map((e) => e.n), [0, 1, 2], 'what it shows stays, once')
  // Opened again: read again from the transcript, numbered after the ones let go.
  const g2 = knChatOpen(c, 'A')
  assert.ok(knChatTake(c, g2, 0, chunk(A.map((e) => ({ ...e, n: e.n + 3 })), 6, { live: false }), 3000))
  assert.deepEqual(c.events.map((e) => e.n), [3, 4, 5])
  assert.equal(c.next, 6)
  assert.equal(knChatTurns(c.events).length, 1, 'one turn, not two')
  // The poll goes on from there; a message follows.
  assert.deepEqual(knChatTake(c, g2, 6, chunk([], 6), 4000).events, [])
  assert.ok(knChatTake(c, g2, 6, chunk(A2.map((e) => ({ ...e, n: e.n + 3 })), 9), 5000))
  assert.deepEqual(c.events.map((e) => e.n), [3, 4, 5, 6, 7, 8])
  assert.deepEqual(knChatTurns(c.events).map((t) => t.user.text), ['What did Lena Hartmann decide?', 'And the gateway?'])
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

// ── Health (spec K16) ──
test('routes: Health, and a finding named by its key', () => {
  assert.deepEqual(knParseRoute('kn/my-twin/health'), { vault: 'my-twin', tab: 'health', path: '' })
  assert.deepEqual(knParseRoute('kn/my-twin/health/broken%3Aa'), { vault: 'my-twin', tab: 'health', path: 'broken:a' })
  const key = 'broken:knowledge/notes/2026-10-29-northwind-checkout-retro.md:[[Northwind Retail|NR]]'
  assert.equal(knRouteOf({ vault: 'my-twin', tab: 'health', path: key }), `kn/my-twin/health/${encodeURIComponent(key)}`)
  assert.equal(knParseRoute(knRouteOf({ vault: 'my-twin', tab: 'health', path: key })).path, key, 'encoded once, decoded once')
  const pct = 'broken:knowledge/a.md:(50%20plan.md)'
  assert.equal(knParseRoute(knRouteOf({ vault: 'my-twin', tab: 'health', path: pct })).path, pct, 'a key holding %20 stays as it is')
})

// Made-up findings, as `/api/kn/<vault>/health` gives them (Echo's order: by kind, then key).
const F = (key, group, by, title, o = {}) => ({ key, kind: key.split(':')[0], group, by, pages: o.pages || [[key.split(':')[1], o.line ?? null]], title, why: 'What it means.', fixable: !!o.fixable, fix_all: !!o.fix_all })
const KB1 = 'broken:knowledge/notes/2026-10-29-northwind-checkout-retro.md:[[Northwind Retail]]'
const KB2 = 'broken:knowledge/activities/platform/delivery-platform.md:(../../notes/2026-8-27-lena-pricing-1-1.md)'
const KN1 = 'not_in_index:knowledge/clients/helios.md'
const KN2 = 'not_in_index:knowledge/clients/aurora.md'
const FINDINGS = [
  F('unknown_tag:knowledge/topics/ai-strategy.md:#tokenomics', 'needs', 'code', '#tokenomics isn\'t in the taxonomy'),
  F('unknown_tag:knowledge/topics/dark-factory.md:#dark-factory', 'needs', 'code', '#dark-factory isn\'t in the taxonomy'),
  F('duplicate_title:knowledge/activities/weekly-sync.md,knowledge/notes/weekly-sync.md', 'needs', 'code', '“Weekly sync” is the title of 2 pages', { pages: [['knowledge/activities/weekly-sync.md', null], ['knowledge/notes/weekly-sync.md', null]] }),
  F(KN1, 'should', 'code', 'Helios isn\'t in CATALOG.md', { fixable: true, fix_all: true }),
  F(KN2, 'should', 'code', 'Aurora isn\'t in CATALOG.md', { fixable: true, fix_all: true }),
  F(KB1, 'should', 'code', '[[Northwind Retail]] leads to no page', { line: 14, fixable: true, fix_all: true }),
  F(KB2, 'should', 'code', '../../notes/2026-8-27-lena-pricing-1-1.md leads to no page', { line: 31, fixable: true }),
  F('broken:knowledge/media/2026-09-ai-summit-talk.md:[[summit-slides]]', 'should', 'code', '[[summit-slides]] leads to no page', { line: 8 }),
  F('isolated:knowledge/notes/2026-01-03-c.md', 'tidy', 'code', 'C has no links'),
]
const AI = F('contradiction:knowledge/clients/northwind-retail.md,knowledge/notes/2026-11-05-northwind-weekly-delivery-sync.md', 'needs', 'ai', 'Northwind go-live: 12 Dec or 9 Jan')

test('K16-T8 findings are grouped Needs you · Should fix · Tidy, by kind with counts, and filtered', () => {
  const l = knHealthList(FINDINGS)
  assert.deepEqual(l.groups.map((g) => [g.id, g.label]), [['needs', 'Needs you'], ['should', 'Should fix'], ['tidy', 'Tidy']])
  assert.deepEqual(l.groups.map((g) => g.kinds.map((k) => [k.kind, k.n])), [[['unknown_tag', 2], ['duplicate_title', 1]], [['not_in_index', 2], ['broken', 3]], [['isolated', 1]]])
  assert.deepEqual([l.kinds, l.needs], [5, 3], 'five kinds; three findings need you — the tab\'s number')
  const broken = l.groups[1].kinds[1]
  assert.deepEqual([broken.name, broken.fixable, broken.fixAll, broken.meta], ['Broken links', 2, 1, '2 fixable by code'], 'a guess has a fix, but Fix all leaves it out')
  assert.deepEqual(broken.keys, [KB1, KB2, 'broken:knowledge/media/2026-09-ai-summit-talk.md:[[summit-slides]]'])
  assert.equal(l.groups[0].kinds[0].meta, '#tokenomics isn\'t in the taxonomy · 1 more', 'nothing to fix by code: what it is about')
  assert.deepEqual(knHealthList(FINDINGS, { filter: 'needs' }).groups.map((g) => g.id), ['needs'])
  const all = [...FINDINGS, AI]
  assert.deepEqual(knHealthList(all, { filter: 'ai' }).groups.flatMap((g) => g.kinds.map((k) => k.kind)), ['contradiction'])
  assert.deepEqual(knHealthList(all, { filter: 'code' }).groups.flatMap((g) => g.kinds.filter((k) => k.by !== 'code')), [])
  assert.deepEqual(knHealthList(all, { filter: 'ai' }).needs, 4, 'the counts are the whole list\'s, whatever the filter')
  assert.deepEqual(knHealthList(FINDINGS, { filter: 'ai' }).groups, [], 'no AI check yet: nothing under AI')
  // Ignored findings are neither listed nor counted; Show ignored lists them apart.
  const ignored = [F('isolated:knowledge/notes/2026-01-04-d.md', 'tidy', 'code', 'D has no links'), F('unknown_tag:knowledge/topics/x.md:#later', 'needs', 'code', '#later isn\'t in the taxonomy')]
  assert.deepEqual(knHealthList(FINDINGS, { ignored }).groups.map((g) => g.id), ['needs', 'should', 'tidy'])
  const shown = knHealthList(FINDINGS, { ignored, showIgnored: true })
  assert.deepEqual(shown.groups.map((g) => g.id), ['needs', 'should', 'tidy', 'ignored'])
  assert.deepEqual(shown.groups[3].kinds.map((k) => [k.kind, k.n, k.ignored]), [['isolated', 1, true], ['unknown_tag', 1, true]])
  assert.deepEqual([shown.kinds, shown.needs], [5, 3], 'ignored findings don\'t count')
  assert.deepEqual(knHealthList([]), { groups: [], kinds: 0, needs: 0 })
})

test('K16-T8 the finding shown: the route\'s, else the first of its kind (one just fixed or ignored), else the first', () => {
  const l = knHealthList(FINDINGS)
  const summit = 'broken:knowledge/media/2026-09-ai-summit-talk.md:[[summit-slides]]'
  assert.deepEqual([knHealthSelect(l, summit).key, knHealthSelect(l, summit).kind.kind], [summit, 'broken'])
  assert.equal(knHealthSelect(l, 'broken:knowledge/notes/gone.md:[[Gone]]').key, KB1, 'gone: the first of its kind')
  assert.equal(knHealthSelect(l, '').key, FINDINGS[0].key)
  assert.equal(knHealthSelect(l, 'stale:knowledge/a.md').key, FINDINGS[0].key, 'a kind no longer listed: the first')
  assert.equal(knHealthSelect(knHealthList(FINDINGS, { filter: 'needs' }), KB1).kind.kind, 'unknown_tag', 'filtered out: the first shown')
  assert.equal(knHealthSelect(knHealthList([]), KB1), null)
})

test('K16-T8 the trend: thirty days of the history, a day without a score drawn as a gap', () => {
  const history = [['2026-09-20', 60], ['2026-10-09', 74], ['2026-10-10', 75], ['2026-10-11', 75], ['2026-10-20', 80], ['2026-11-05', 81], ['2026-11-06', 82], ['2026-11-07', 82]].map(([day, score]) => ({ day, score, categories: [] }))
  const t = knHealthTrend(history, '2026-11-07')
  assert.equal(t.points.length, 30)
  assert.deepEqual([t.points[0].day, t.points[29].day], ['2026-10-09', '2026-11-07'])
  assert.deepEqual(t.points.slice(0, 4).map((p) => p.score), [74, 75, 75, null])
  assert.equal(t.points.filter((p) => p.score != null).length, 7, 'older than thirty days: not in the trend')
  assert.deepEqual([t.first.day, t.last.day, t.delta], ['2026-10-09', '2026-11-07', 8])
  const p = knTrendPath(t.points, { w: 300, h: 44 })
  assert.equal((p.line.match(/M/g) || []).length, 2, 'two runs of days with a score: two lines')
  assert.match(p.line, /^M3,[\d.]+L13\.1,[\d.]+L23\.3,[\d.]+M276\.7,[\d.]+L286\.9,[\d.]+L297,[\d.]+$/, 'no line across a gap')
  assert.equal((p.area.match(/Z/g) || []).length, 2)
  assert.deepEqual(p.dots.map(([x]) => x), [114.5], 'a day alone between gaps is a dot (20 Oct)')
  assert.equal(p.end[0], 297, 'the last point: today')
  // Across a change of the clocks the days are still one apart.
  assert.deepEqual(knHealthTrend([], '2026-10-27', 3).points.map((x) => x.day), ['2026-10-25', '2026-10-26', '2026-10-27'])
  const none = knHealthTrend([], '2026-11-07')
  assert.equal(none.delta, null)
  assert.deepEqual(knTrendPath(none.points), { line: '', area: '', dots: [], end: null })
  const one = knHealthTrend([{ day: '2026-11-07', score: 100, categories: [] }], '2026-11-07')
  assert.deepEqual([one.delta, knTrendPath(one.points).line, knTrendPath(one.points).dots.length], [null, '', 1], 'the first check: a dot, no change to tell')
})

test('Fix all sends each ticked fix by its key with the base and sha of the diff shown; a guess is never in it', () => {
  const hunk = [['-', 'Agreed with [[Northwind Retail]] to add a 3-D Secure fallback.'], ['+', 'Agreed with [[clients/northwind-retail|Northwind Retail]] to add a 3-D Secure fallback.']]
  const fixes = [
    { key: KB1, choice: 'to', path: 'knowledge/notes/2026-10-29-northwind-checkout-retro.md', base: 'b1', sha: 's1', guess: false, hunks: [hunk], plus: 1, minus: 1 },
    { key: KB2, choice: 'to', path: 'knowledge/activities/platform/delivery-platform.md', base: 'b2', sha: 's2', guess: true, hunks: [hunk], plus: 1, minus: 1 },
    { key: KN1, choice: 'row', path: 'knowledge/CATALOG.md', base: 'b3', sha: 's3', guess: false, hunks: [[['+', '| [Helios](clients/helios.md) | Helios | 2026-11-07 |']]], plus: 1, minus: 0 },
    { key: KN2, choice: 'row', path: 'knowledge/CATALOG.md', base: 'b3', sha: 's4', guess: false, hunks: [[['+', '| [Aurora](clients/aurora.md) | Aurora | 2026-11-07 |']]], plus: 1, minus: 0 },
    { key: 'broken:knowledge/notes/x.md:[[Gone]]', error: 'the link isn\'t there any more' },
  ]
  const s = knFixAll(fixes, FINDINGS)
  assert.deepEqual(s.rows.map((r) => r.key), [KB1, KN1, KN2, 'broken:knowledge/notes/x.md:[[Gone]]'], 'a guess is fixed on its own finding only')
  assert.deepEqual(s.items, [{ key: KB1, choice: 'to', base: 'b1', sha: 's1' }, { key: KN1, choice: 'row', base: 'b3', sha: 's3' }, { key: KN2, choice: 'row', base: 'b3', sha: 's4' }])
  assert.equal(s.files, 2, 'two rows in one index: one file')
  assert.deepEqual([s.rows[0].line, s.rows[0].title, s.rows[0].on], [14, '[[Northwind Retail]] leads to no page', true])
  assert.deepEqual([s.rows[3].on, s.rows[3].error], [false, 'the link isn\'t there any more'], 'one Echo couldn\'t compute: why, unticked')
  const off = knFixAll(fixes, FINDINGS, new Set([KN1, KN2]))
  assert.deepEqual([off.items.map((i) => i.key), off.files], [[KB1], 1], 'unticked: left out')
  const bare = knFixAll([{ key: KB1, choice: 'to', path: 'p.md', base: 'b1', guess: false, hunks: [hunk] }], FINDINGS)
  assert.deepEqual([bare.items, bare.rows[0].on], [[], false], 'without the sha of what was shown, never sent')
})

test('what a fix did, in Echo\'s words: what was written, what wasn\'t and why', () => {
  assert.deepEqual(knFixSummary({ written: ['knowledge/notes/a.md'], changed_since: [], skipped: [], commit: 'a1b2c3d' }), { text: 'Fixed: knowledge/notes/a.md. Committed (a1b2c3d).', wait: false })
  assert.deepEqual(knFixSummary({ written: ['knowledge/CATALOG.md', 'knowledge/log.md'], changed_since: [], skipped: [], commit: 'off' }), { text: 'Fixed in 2 files.', wait: false })
  // The commit takes only files without changes of yours (X-G2): `committed` names them.
  assert.deepEqual(knFixSummary({ written: ['knowledge/a.md', 'knowledge/b.md'], committed: ['knowledge/a.md', 'knowledge/b.md'], changed_since: [], skipped: [], commit: 'a1b2c3d' }), { text: 'Fixed in 2 files. Committed (a1b2c3d).', wait: false })
  assert.deepEqual(knFixSummary({ written: ['knowledge/a.md', 'knowledge/b.md'], committed: ['knowledge/a.md'], changed_since: [], skipped: [], commit: 'a1b2c3d' }), { text: 'Fixed in 2 files. Committed (a1b2c3d), but not knowledge/b.md: it had changes of yours.', wait: true })
  assert.equal(knFixSummary({ written: ['knowledge/a.md', 'knowledge/b.md', 'knowledge/c.md'], committed: ['knowledge/a.md'], changed_since: [], skipped: [], commit: 'a1b2c3d' }).text, 'Fixed in 3 files. Committed (a1b2c3d), but not knowledge/b.md, knowledge/c.md: they had changes of yours.')
  const r = knFixSummary({ written: ['knowledge/a.md', 'knowledge/b.md'], changed_since: ['knowledge/c.md'], skipped: [{ key: KB2, why: 'Echo\'s page for it is a guess: fix it on its own' }], commit: 'skipped: the file had changes of yours that weren\'t committed' }, FINDINGS)
  assert.equal(r.text, 'Fixed in 2 files. Not committed: the file had changes of yours that weren\'t committed. knowledge/c.md changed since Echo showed its fix, so it was left as it is. Not fixed: ../../notes/2026-8-27-lena-pricing-1-1.md leads to no page — Echo\'s page for it is a guess: fix it on its own.')
  assert.equal(r.wait, true, 'something wasn\'t done: said until OK')
  assert.equal(knFixSummary({ written: [], changed_since: [], skipped: [{ key: 'gone:x', why: 'that finding isn\'t there any more' }], commit: 'off' }).text, 'Not fixed: a finding — that finding isn\'t there any more.')
  assert.deepEqual(knFixSummary({ written: [], changed_since: [], skipped: [], commit: 'off' }), { text: 'Nothing was changed.', wait: true })
})

// ── AI checks (K16-05…07) ──
const AIQ = (path, line, date, text) => ({ path, line, date, text })
const CLIENT_P = 'knowledge/clients/northwind-retail.md', NOTE_P = 'knowledge/notes/2026-11-05-northwind-weekly-delivery-sync.md'
const AI_FINDINGS = [
  F(`contradiction:${CLIENT_P},${NOTE_P}:#1a2b3c4d`, 'needs', 'ai', 'Northwind go-live: 12 Dec or 9 Jan', { pages: [[CLIENT_P, 18], [NOTE_P, 22]], fixable: true }),
  F('possible_duplicate:knowledge/topics/pricing.md,knowledge/topics/per-stream-pricing.md:#5e6f7a8b', 'needs', 'ai', 'Two pricing pages', { pages: [['knowledge/topics/pricing.md', 3], ['knowledge/topics/per-stream-pricing.md', 7]] }),
  F('stale:knowledge/topics/ai-strategy.md:#9c0d1e2f', 'tidy', 'ai', 'The factory guild meets weekly', { line: 44 }),
  F('stale:knowledge/topics/delivery.md:#3a4b5c6d', 'tidy', 'ai', 'Releases go out on Fridays', { line: 9 }),
  F('missing_summary:knowledge/clients/helios.md:#7e8f9a0b', 'tidy', 'ai', 'Helios has no summary', { line: 1, fixable: true }),
]

test('K16-07 AI findings: grouped with the code ones, by kind, under the AI filter only theirs', () => {
  const all = [...AI_FINDINGS, ...FINDINGS]
  const ai = knHealthList(all, { filter: 'ai' })
  assert.deepEqual(ai.groups.map((g) => [g.id, g.kinds.map((k) => [k.kind, k.n, k.by])]), [['needs', [['contradiction', 1, 'ai'], ['possible_duplicate', 1, 'ai']]], ['tidy', [['stale', 2, 'ai'], ['missing_summary', 1, 'ai']]]])
  const [contra] = ai.groups[0].kinds, [stale] = ai.groups[1].kinds
  assert.deepEqual([contra.name, contra.meta], ['Two pages disagree', 'Northwind go-live: 12 Dec or 9 Jan'], 'an AI kind\'s line names the finding, never "fixable by code"')
  assert.deepEqual([stale.name, stale.meta], ['Statements a newer page replaced', 'The factory guild meets weekly · 1 more'])
  assert.deepEqual(knHealthList(all).groups[0].kinds.map((k) => k.kind), ['contradiction', 'possible_duplicate', 'unknown_tag', 'duplicate_title'], 'All: the AI kinds first in their group, as Health lists them')
  assert.deepEqual(knHealthList(all, { filter: 'code' }).groups.flatMap((g) => g.kinds).filter((k) => k.by === 'ai'), [], 'By code: none of them')
  assert.deepEqual(knHealthList(all, { filter: 'needs' }).groups.flatMap((g) => g.kinds.map((k) => k.kind)), ['contradiction', 'possible_duplicate', 'unknown_tag', 'duplicate_title'])
  assert.equal(knHealthList(all).needs, 5, 'Needs you counts AI findings too (the tab\'s number)')
  for (const k of ['contradiction', 'possible_duplicate', 'stale', 'link_to_add', 'missing_summary']) {
    const [name, head, fixAll] = KN_HEALTH_KINDS[k]
    assert.ok(name && head(1) && head(3) !== head(1), k)
    assert.equal(fixAll, undefined, `${k}: AI fixes are never in Fix all`)
  }
  assert.equal(KN_HEALTH_KINDS.contradiction[1](2), '2 contradictions between pages')
})

test('K16-06 what an AI check kept and dropped, in Echo\'s words', () => {
  assert.equal(knLintNotice({ kept: 6, dropped: 2 }), 'Kept 6 findings · 2 dropped: their quotes aren\'t in the pages.')
  assert.equal(knLintNotice({ kept: 1, dropped: 1 }), 'Kept 1 finding · 1 dropped: its quotes aren\'t in the pages.')
  assert.equal(knLintNotice({ kept: 3, dropped: 0 }), 'Kept 3 findings.')
  assert.equal(knLintNotice({ kept: 0, dropped: 4 }), 'Kept no finding · 4 dropped: their quotes aren\'t in the pages.')
  assert.equal(knLintNotice({ kept: 0, dropped: 0 }), 'Found nothing to fix.')
  assert.equal(knLintNotice({ kept: 200, dropped: 3, left: 40 }), 'Kept 200 findings · 3 dropped: their quotes aren\'t in the pages · 40 more not read: Echo keeps 200 a check.', 'over the cap: said, not kept')
  assert.equal(knLintNotice({ kept: 200, dropped: 0, left: 1 }), 'Kept 200 findings · 1 more not read: Echo keeps 200 a check.')
  assert.equal(knLintNotice(null), '')
})

test('K16-07 a contradiction\'s two quotes side by side: the older first, then the newer', () => {
  const older = AIQ(CLIENT_P, 18, '2026-10-02', 'Phase 2 go-live: 12 December, before the holiday freeze')
  const newer = AIQ(NOTE_P, 22, '2026-11-05', 'Phase 2 go-live moves to 9 January')
  assert.deepEqual(knVersus([newer, older]).map((q) => [q.path, q.side]), [[CLIENT_P, 'Older'], [NOTE_P, 'Newer']], 'as the check gave them, newer first: turned')
  assert.deepEqual(knVersus([older, newer]).map((q) => [q.path, q.side]), [[CLIENT_P, 'Older'], [NOTE_P, 'Newer']])
  assert.deepEqual(knVersus([newer, { ...older, date: '2026-11-05' }]).map((q) => [q.path, q.side]), [[NOTE_P, null], [CLIENT_P, null]], 'the same day: as given, no side named')
  assert.deepEqual(knVersus([newer, { ...older, date: null }]).map((q) => q.side), [null, null], 'a date unknown: as given')
  assert.equal(knVersus([older]), null, 'not two quotes: no side by side')
  assert.equal(knVersus([older, newer, older]), null)
})

test('K16-05 the AI row: a check going, the last one\'s failure said until the next check, else the last that ended', () => {
  const ai = { at: '2026-11-07T10:00:00+01:00', run: '2026-11-07_095500', cost: 0.31, kept: 1, dropped: 2 }
  assert.deepEqual(knLintState({ running: '2026-11-07_120000', last: { id: '2026-11-07_120000', state: 'running', waiting: 'Waiting for a call\'s transcription to finish' } }, ai), { state: 'running', run: '2026-11-07_120000', waiting: 'Waiting for a call\'s transcription to finish', reason: null, cost: null })
  assert.deepEqual(knLintState({ running: null, last: { id: '2026-11-07_120000', state: 'failed', reason: 'It wrote no findings', cost: 0.04 } }, ai), { state: 'failed', run: null, waiting: null, reason: 'It wrote no findings', cost: 0.04 }, 'what the failed check cost, said with it (K16-01) — the last check that ended stays `ai`')
  assert.equal(knLintState({ running: null, last: { id: '2026-11-07_120000', state: 'stopped', reason: 'Stopped for a sign-in' } }, ai).cost, null, 'no result, no cost')
  assert.equal(knLintState({ running: null, last: { id: '2026-11-07_120000', state: 'stopped', reason: 'Stopped for a sign-in' } }, ai).state, 'stopped')
  assert.equal(knLintState({ running: null, last: { id: ai.run, state: 'done' } }, ai).state, 'idle')
  assert.equal(knLintState(null, null).state, 'idle', 'never run')
})

// ── Health, fix round 1 ──
test('Health is asked again whenever its tab is entered (a settings change isn\'t in the pages\' rev); picking a finding asks for that one', () => {
  const h = { vault: 'my-twin', data: { findings: [] } }
  assert.equal(knHealthLoad(h, { vault: 'my-twin', tab: 'health' }), 'all', 'entering Health with its findings loaded: asked again')
  assert.equal(knHealthLoad(h, { vault: 'my-twin', tab: 'health', picked: true }), 'finding', 'another finding of the list shown: that one alone')
  assert.equal(knHealthLoad(null, { vault: 'my-twin', tab: 'health', picked: true }), 'all')
  assert.equal(knHealthLoad({ vault: 'team-wiki', data: { findings: [] } }, { vault: 'my-twin', tab: 'health', picked: true }), 'all', 'another vault\'s findings')
  assert.equal(knHealthLoad(h, { vault: 'my-twin', tab: 'pages' }), null, 'elsewhere, with the tab\'s number there: nothing')
  assert.equal(knHealthLoad({ vault: 'my-twin', data: null }, { vault: 'my-twin', tab: 'settings' }), 'count', 'after a settings change emptied it: the number again')
})

test('a settings change empties Health but keeps how you look at it; an answer still coming for the old state is dropped', () => {
  const fresh = knHealthState('my-twin')
  assert.deepEqual([fresh.vault, fresh.data, fresh.filter, fresh.showIgnored, fresh.choice], ['my-twin', null, 'all', false, {}])
  const old = { ...fresh, data: { findings: [] }, rev: 'r1', at: 5, detail: { key: KB1 }, want: KB1, choice: { [KB1]: 'remove' }, filter: 'needs', showIgnored: true, error: 'x' }
  const s = knHealthState('my-twin', old)
  assert.deepEqual([s.data, s.rev, s.detail, s.choice, s.error, s.filter, s.showIgnored], [null, null, null, {}, null, 'needs', true])
  assert.notEqual(s, old, 'a new state: a load that started for the old one doesn\'t land in it')
})

test('Fix all shows the first fixes and folds the rest, which stay ticked and are sent', () => {
  const nine = Array.from({ length: 9 }, (_, i) => ({ key: `broken:knowledge/notes/n${i}.md:[[Northwind Retail]]`, choice: 'to', path: `knowledge/notes/n${i}.md`, base: `b${i}`, sha: `s${i}`, guess: false, hunks: [], plus: 1, minus: 1 }))
  const s = knFixAll(nine, [])
  assert.deepEqual([s.shown.map((r) => r.key), s.more], [nine.slice(0, 4).map((x) => x.key), { n: 5, ticked: 5 }])
  assert.equal(s.items.length, 9, 'the folded fixes are sent too')
  assert.deepEqual(knFixAll(nine, [], new Set([nine[6].key])).more, { n: 5, ticked: 4 })
  const open = knFixAll(nine, [], new Set(), { open: true })
  assert.deepEqual([open.shown.length, open.more], [9, null], 'shown, all of them')
  const five = knFixAll(nine.slice(0, 5), [])
  assert.deepEqual([five.shown.length, five.more], [5, null], 'one more than four isn\'t folded')
})

test('a tab with a number says it in its name: "Health, 3 need you"', () => {
  assert.equal(knTabLabel('Health', 3), 'Health, 3 need you')
  assert.equal(knTabLabel('Health', 1), 'Health, 1 needs you')
  assert.equal(knTabLabel('Health', null), 'Health')
})

// ── Ingest (spec K14, K15) ──
test('routes: Ingest, and a run named by its id', () => {
  assert.deepEqual(knParseRoute('kn/my-twin/ingest'), { vault: 'my-twin', tab: 'ingest', path: '' })
  assert.deepEqual(knParseRoute('kn/my-twin/ingest/2026-11-06_111700'), { vault: 'my-twin', tab: 'ingest', path: '2026-11-06_111700' })
  assert.equal(knRouteOf({ vault: 'my-twin', tab: 'ingest', path: '2026-11-06_111700-2' }), 'kn/my-twin/ingest/2026-11-06_111700-2')
  assert.equal(knParseRoute(knRouteOf({ vault: 'my-twin', tab: 'ingest', path: '2026-11-06_111700-2' })).path, '2026-11-06_111700-2')
})

// Made-up changes, as `/api/kn/<vault>/ingest/<run>/changes` gives them (Rust has merged the answers and D28's drops).
const CH = (path, kind, state, o = {}) => ({ path, kind, adds_only: !!o.adds, plus: o.plus ?? 1, minus: o.minus ?? 0, diff: o.diff || [[['+', `a line of ${path}`]]], why: o.why ?? null, quotes: o.quotes || [], invented: o.invented || 0, quote_note: o.qnote ?? null, dropped: o.dropped || [], note: o.note ?? null, state, edited: !!o.edited })
const TOPIC = 'knowledge/topics/per-stream-pricing.md'
const ROW = '| [per-stream-pricing.md](topics/per-stream-pricing.md) | Per-stream pricing | 2026-11-06 |'
const CHANGES = [
  CH('knowledge/clients/northwind-retail.md', 'edit', 'approved', { plus: 4, minus: 1, why: 'the go-live moves to 9 January', quotes: [{ source: 'raw_inbox/mail/2026-11-04-phase-2-and-the-holiday-freeze.md', text: 'I\'d like to move go-live to 9 January' }] }),
  CH('knowledge/notes/2026-11-06-pricing-thoughts.md', 'new', 'undecided', { plus: 31, quotes: [{ source: 'raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md', text: 'count streams the same way at every client' }], invented: 1 }),
  CH(TOPIC, 'new', 'rejected', { plus: 22, qnote: 'Echo can\'t check quotes in this kind of file' }),
  CH('knowledge/CATALOG.md', 'index', 'undecided', { adds: true, qnote: 'no quote found', dropped: [ROW], note: `1 line left out: it links to ${TOPIC}, which you rejected` }),
  CH('log.md', 'log', 'undecided', { adds: true, plus: 2, qnote: 'no quote found', dropped: [`- 2026-11-06 · added · ${TOPIC}`, `- 2026-11-06 · processed · raw_inbox/web/2026-11-06-per-stream-pricing.md → ${TOPIC}`], note: `2 lines left out: they link to ${TOPIC}, which you rejected` }),
  CH('knowledge/aux.md', 'other', 'blocked', { note: 'can\'t be written: “aux.md” names a device on Windows' }),
]

test('K15-T8 the review: counts from the changes and their answers, Approve the rest only with one undecided, Write N only with one approved', () => {
  const r = knIngestReview(CHANGES)
  assert.deepEqual(r.counts, { all: 6, undecided: 3, approved: 1, rejected: 1, written: 0, changed: 0, blocked: 1 })
  assert.equal(r.rest, true, 'three changes are undecided')
  assert.deepEqual(r.write, { n: 1, can: true })
  assert.deepEqual(r.cards.map((c) => [c.n, c.kind, c.tag, c.state]), [[1, 'Edit', '', 'approved'], [2, 'New page', 'ok', 'undecided'], [3, 'New page', 'ok', 'rejected'], [4, 'Index', 'acc', 'undecided'], [5, 'Log', 'acc', 'undecided'], [6, 'Other', '', 'blocked']])
  assert.deepEqual(r.cards.map((c) => c.open), [false, true, false, true, true, false], 'only an undecided change takes Approve · Reject · Edit')
  const all = knIngestReview(CHANGES.map((c) => ({ ...c, state: c.state === 'undecided' ? 'rejected' : c.state })))
  assert.equal(all.rest, false, 'nothing undecided: no Approve the rest')
  assert.deepEqual(all.write, { n: 1, can: true })
  const none = knIngestReview(CHANGES.map((c) => ({ ...c, state: c.state === 'approved' ? 'rejected' : c.state })))
  assert.deepEqual(none.write, { n: 0, can: false }, 'nothing approved: no Write')
  assert.deepEqual(knIngestReview([]).write, { n: 0, can: false })
})

test('K15-T8 D28: a rejected new page\'s index row and log lines are left out of those changes, each saying so', () => {
  const r = knIngestReview(CHANGES)
  assert.deepEqual(r.cards[3].left, { note: `1 line left out: it links to ${TOPIC}, which you rejected`, lines: [ROW] })
  assert.equal(r.cards[4].left.lines.length, 2)
  assert.match(r.cards[4].left.note, /^2 lines left out/)
  assert.equal(r.cards[0].left, null, 'a change with nothing left out says nothing')
  assert.equal(r.cards[3].adds, true, 'adds only')
})

test('K15-T8 a blocked change says why and takes no answer; Approve the rest leaves it out', () => {
  const r = knIngestReview(CHANGES)
  assert.equal(r.cards[5].blocked, 'can\'t be written: “aux.md” names a device on Windows')
  assert.equal(r.cards[5].left, null, 'its note is why it is blocked, not D28')
  const only = knIngestReview([CHANGES[5], CHANGES[0]])
  assert.equal(only.rest, false, 'a blocked change is never one of the rest')
  assert.equal(r.cards[0].blocked, null)
})

test('K15-T8 the quotes: found ones shown, invented ones counted, a PDF\'s change says Echo can\'t check it', () => {
  const r = knIngestReview(CHANGES)
  assert.equal(r.cards[1].quotes.length, 1)
  assert.equal(r.cards[1].invented, '1 more quote the run gave isn\'t in the sources — not shown.')
  assert.equal(knIngestReview([CH('a.md', 'new', 'undecided', { invented: 2, qnote: 'no quote found' })]).cards[0].invented, '2 more quotes the run gave aren\'t in the sources — not shown.')
  assert.equal(r.cards[2].fromNote, 'Echo can\'t check quotes in this kind of file, so none are shown.')
  assert.equal(r.cards[3].fromNote, 'Written by the skill from the changes above — no source words to show.')
  assert.equal(knIngestReview([CH('knowledge/x.md', 'edit', 'undecided', { qnote: 'no quote found' })]).cards[0].fromNote, 'No quote found in the sources.')
  assert.equal(r.cards[0].fromNote, '', 'quotes shown: nothing more to say')
})

test('K15-T8 an answer on its way: that change waits, and so do Approve the rest and Write', () => {
  const r = knIngestReview(CHANGES, { 'knowledge/notes/2026-11-06-pricing-thoughts.md': 'approve' })
  assert.equal(r.cards[1].busy, true)
  assert.equal(r.cards[1].open, false, 'answered once')
  assert.equal(r.rest, false)
  assert.deepEqual(r.write, { n: 1, can: false })
})

test('nothing left to decide (written, rejected, can\'t be written): Finish the run instead of Write', () => {
  const left = [CH('knowledge/a.md', 'new', 'written'), CH('log.md', 'log', 'rejected'), CH('knowledge/aux.md', 'other', 'blocked', { note: 'can\'t be written: x' })]
  assert.deepEqual([knIngestReview(left).finish, knIngestReview(left).write], [true, { n: 0, can: false }])
  assert.equal(knIngestReview(CHANGES).finish, false, 'something is undecided or approved')
  assert.equal(knIngestReview([...left, CH('knowledge/b.md', 'edit', 'changed')]).finish, false, 'a change made in the vault since the run waits for an answer')
  assert.equal(knIngestReview(left, { 'log.md': 'approve' }).finish, false, 'an answer on its way')
  assert.equal(knIngestReview([]).finish, true, 'fix round 1, M1: a run that changed nothing is finished too')
})

test('a change made in the vault since the run waits for Compare, then Take the run\'s; written ones are done', () => {
  const r = knIngestReview([CH('knowledge/clients/helios.md', 'edit', 'changed'), CH('knowledge/notes/a.md', 'new', 'written')])
  assert.deepEqual(r.counts, { all: 2, undecided: 0, approved: 0, rejected: 0, written: 1, changed: 1, blocked: 0 })
  assert.deepEqual([r.cards[0].open, r.cards[0].changed, r.cards[1].open], [false, true, false])
  assert.equal(r.rest, false)
})

// Made-up sources, as `/api/kn/<vault>/ingest` gives them (newest first), and its runs.
const SRC = (path, kind, title, state, meta = {}) => ({ path, kind, title, state, meta: { size: 1200, markdown: /\.md$/.test(path), ...meta } })
const RUNS = [
  { id: 'r3', at: '2026-11-06T11:17:00+01:00', sources: ['raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md', 'raw_inbox/mail/2026-11-04-phase-2.md', 'raw_inbox/web/2026-11-06-per-stream-pricing.md'], state: 'running' },
  { id: 'r2', at: '2026-11-06T08:06:00+01:00', sources: ['raw_inbox/Aurora RFP.pdf'], state: 'review', changes: 1, kept: 0, to_answer: 1, cost: 0.05 },
  { id: 'r1', at: '2026-11-05T17:12:00+01:00', sources: ['raw_inbox/pricing-workshop-notes.md'], state: 'done', changes: 3, kept: 3, cost: 0.07 },
  { id: 'r0', at: '2026-11-05T10:03:00+01:00', sources: ['raw_inbox/teams/2026-11-05-northwind-delivery.md'], state: 'failed', reason: 'It stopped when Echo did' },
]
const SOURCES = [
  SRC('raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md', 'echo', 'Memo · Pricing thoughts', 'waiting', { run: 'r3' }),
  SRC('raw_inbox/web/2026-11-06-per-stream-pricing.md', 'web', 'Per-stream pricing for AI delivery', 'waiting', { run: 'r3' }),
  SRC('raw_inbox/Aurora RFP.pdf', 'file', 'Aurora RFP.pdf', 'waiting', { run: 'r2' }),
  SRC('raw_inbox/teams/2026-11-05-northwind-delivery.md', 'teams', 'Northwind delivery', 'waiting', { run: 'r0' }),
  SRC('raw_inbox/mail/2026-11-04-phase-2.md', 'mail', 'Phase 2 and the holiday freeze', 'waiting', { run: 'r3' }),
  SRC('raw_inbox/echo/2026-11-05-northwind-sync.md', 'echo', 'Northwind · Weekly delivery sync', 'new'),
  SRC('raw_inbox/tokenomics-draft.docx', 'file', 'tokenomics-draft.docx', 'skipped', { note: 'A Word file Claude can\'t read' }),
  SRC('raw_inbox/pricing-workshop-notes.md', 'file', 'Pricing workshop notes', 'done', { run: 'r1', kept: 3 }),
  SRC('raw_inbox/old-notes.md', 'file', 'Old notes', 'done', { kept: 0, run: 'r9' }),
  SRC('raw_inbox/2025-plan.md', 'file', '2025 plan', 'done'),
]

test('K14-08 the queue: grouped From Echo · Mail & Teams · Web · Files, newest first, each with its state', () => {
  const q = knIngestQueue(SOURCES, RUNS)
  assert.deepEqual(q.groups.map((g) => [g.id, g.label, g.n]), [['echo', 'From Echo', 2], ['mail', 'Mail & Teams', 2], ['web', 'Web', 1], ['file', 'Files', 5]])
  assert.deepEqual(q.groups[0].rows.map((r) => r.title), ['Memo · Pricing thoughts', 'Northwind · Weekly delivery sync'], 'in the order Echo gives: newest first')
  assert.deepEqual(q.groups[1].rows.map((r) => r.kind), ['teams', 'mail'], 'a Teams chat is with mail')
  const open = knIngestQueue(SOURCES, RUNS, { showDone: { echo: true, mail: true, web: true, file: true } })
  const by = new Map(open.groups.flatMap((g) => g.rows).map((r) => [r.path, r]))
  assert.deepEqual(by.get('raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md').chip, ['run', 'Preparing'], 'its run is going')
  assert.deepEqual(by.get('raw_inbox/Aurora RFP.pdf').chip, ['wait', 'In review'])
  assert.deepEqual(by.get('raw_inbox/teams/2026-11-05-northwind-delivery.md').chip, ['fail', 'Run stopped'], 'its run ended with its Echo: Run again, not Prepare changes')
  assert.equal(by.get('raw_inbox/teams/2026-11-05-northwind-delivery.md').state, 'failed')
  assert.deepEqual(by.get('raw_inbox/echo/2026-11-05-northwind-sync.md').chip, ['new', 'New'])
  assert.deepEqual(by.get('raw_inbox/tokenomics-draft.docx').chip, ['skip', 'Skipped'])
  assert.equal(by.get('raw_inbox/tokenomics-draft.docx').note, 'A Word file Claude can\'t read')
  assert.deepEqual([...by.values()].filter((r) => r.selectable).map((r) => r.path), ['raw_inbox/echo/2026-11-05-northwind-sync.md'], 'only a New source is picked for Prepare changes')
  assert.deepEqual(q.counts, { all: 10, new: 1, waiting: 5, done: 4 })
})

test('K14-08 the queue\'s filters, and done sources folded under All until shown', () => {
  const all = knIngestQueue(SOURCES, RUNS)
  const files = all.groups.find((g) => g.id === 'file')
  assert.deepEqual([files.rows.map((r) => r.path), files.folded], [['raw_inbox/Aurora RFP.pdf'], 4], 'Done and Skipped fold into one line')
  const open = knIngestQueue(SOURCES, RUNS, { showDone: { file: true } }).groups.find((g) => g.id === 'file')
  assert.deepEqual([open.rows.length, open.folded], [5, 0])
  const done = knIngestQueue(SOURCES, RUNS, { filter: 'done' })
  assert.deepEqual(done.groups.map((g) => [g.id, g.rows.length, g.folded]), [['file', 4, 0]], 'Done shows them all, Skipped too')
  const chips = done.groups[0].rows.map((r) => r.chip[1])
  assert.deepEqual(chips, ['Skipped', 'Done · kept 3', 'Done — nothing kept', 'Done'])
  assert.deepEqual(knIngestQueue(SOURCES, RUNS, { filter: 'new' }).groups.map((g) => [g.id, g.rows.length]), [['echo', 1]])
  assert.deepEqual(knIngestQueue(SOURCES, RUNS, { filter: 'waiting' }).groups.map((g) => [g.id, g.rows.length]), [['echo', 1], ['mail', 2], ['web', 1], ['file', 1]])
  assert.deepEqual(knIngestQueue(SOURCES, RUNS, { filter: 'waiting' }).counts, all.counts, 'the counts never change with the filter')
  assert.deepEqual(knIngestQueue([], []), { groups: [], counts: { all: 0, new: 0, waiting: 0, done: 0 } })
})

test('K10-02 Ingest\'s number: the changes waiting for review across the vault\'s runs', () => {
  assert.equal(knIngestWaiting(RUNS), 1)
  assert.equal(knIngestWaiting([...RUNS, { id: 'r4', state: 'review', changes: 5, kept: 0, to_answer: 5 }, { id: 'r5', state: 'review', changes: 4, kept: 4, to_answer: 0 }]), 6)
  // Fix round 1, I3: answered changes (rejected, approved and not written yet) wait for no review.
  assert.equal(knIngestWaiting([{ id: 'r6', state: 'review', changes: 5, kept: 0, to_answer: 0 }]), 0, 'all 5 rejected: nothing waits')
  assert.equal(knIngestWaiting([{ id: 'r7', state: 'review', changes: 5, kept: 1, to_answer: 2 }]), 2, 'what Echo counted: undecided and changed in the vault')
  assert.equal(knIngestWaiting([]), 0)
  assert.equal(knIngestWaiting(undefined), 0)
})

test('K15-11 the last runs: when, sources, changes kept, cost', () => {
  const t = (p) => (SOURCES.find((s) => s.path === p) || { title: p }).title
  assert.equal(knRunLine(RUNS[0], t).text, '3 sources · preparing')
  assert.equal(knRunLine(RUNS[1], t).text, 'Aurora RFP.pdf · 1 to review · $0.05')
  assert.equal(knRunLine(RUNS[2], t).text, 'Pricing workshop notes · kept 3 of 3 · $0.07')
  assert.equal(knRunLine(RUNS[3], t).text, 'Northwind delivery · failed')
  assert.equal(knRunLine({ ...RUNS[0], waiting: 'Waiting for a call\'s transcription to finish' }, t).text, '3 sources · waiting for a call')
  assert.equal(knRunLine({ id: 'x', sources: ['a.md', 'b.md'], state: 'discarded' }).text, '2 sources · discarded — nothing kept')
  assert.equal(knRunLine({ id: 'x', sources: ['raw_inbox/a.md'], state: 'stopped', cost: 0.004 }).text, 'a.md · stopped · <$0.01')
  assert.equal(knRunLine({ id: 'x', sources: ['raw_inbox/a.md'], state: 'review', changes: 5, kept: 0, to_answer: 0 }).text, 'a.md · all answered — write or finish', 'fix round 1, I3')
  assert.equal(knRunLine({ id: 'x', sources: ['raw_inbox/a.md'], state: 'review', changes: 5, kept: 2, to_answer: 1 }).text, 'a.md · 1 to review')
  assert.equal(knRunLine({ id: 'x', sources: ['raw_inbox/a.md'], state: 'review', changes: 0, kept: 0, to_answer: 0 }).text, 'a.md · nothing changed — finish', 'fix round 1, M1')
  assert.equal(knRunLine({ id: 'x', sources: ['raw_inbox/a.md'], state: 'done', changes: 0, kept: 0 }).text, 'a.md · nothing changed')
})

// A made-up run's events, as `/api/kn/<vault>/ingest/<run>?from=` gives them.
const STEPS = (s) => JSON.stringify(s.map(([text, state]) => ({ text, state })))
const RUN_EVENTS = [
  { n: 1, type: 'notice', kind: 'copy', text: 'Not in the copy: .git/, media/talk.mp4 (30 MB), 42 other sources in raw_inbox/' },
  { n: 2, type: 'notice', kind: 'waiting', text: 'Waiting for a call\'s transcription to finish' },
  { n: 3, type: 'tool', id: 't1', name: 'Skill', verb: 'Used skill', target: 'process-inbox', detail: '', readonly: true },
  { n: 4, type: 'tool_result', id: 't1', ok: true, summary: '' },
  { n: 5, type: 'tool', id: 't2', name: 'Write', verb: 'Wrote', target: '.echo-ingest/steps.json', detail: '', path: '.echo-ingest/steps.json', readonly: false, input: { file_path: '.echo-ingest/steps.json', content: STEPS([['Find what is new', 'done'], ['Read the memo', 'active'], ['Index rows and log lines', 'todo']]) } },
  { n: 6, type: 'todo', items: [{ text: 'Find what is new', state: 'done' }, { text: 'Read the memo', state: 'active' }, { text: 'Index rows and log lines', state: 'todo' }] },
  { n: 7, type: 'tool_result', id: 't2', ok: true, summary: '' },
  { n: 8, type: 'tool', id: 't3', name: 'Read', verb: 'Read', target: 'raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md', detail: '', path: 'raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md', readonly: true },
]

test('K15-06 a run as it goes: the copy\'s line, the steps as they tick, its tool rows, the wait for a call', () => {
  const w = knRunView(RUN_EVENTS.slice(0, 2))
  assert.equal(w.copy, 'Not in the copy: .git/, media/talk.mp4 (30 MB), 42 other sources in raw_inbox/')
  assert.equal(w.waiting, 'Waiting for a call\'s transcription to finish', 'X-S2: recording has priority, said')
  const r = knRunView(RUN_EVENTS)
  assert.equal(r.waiting, null, 'Claude Code started: the wait is over')
  assert.deepEqual(r.steps.map((s) => s.state), ['done', 'active', 'todo'])
  assert.deepEqual(r.tools.map((t) => [t.verb, t.target, t.detail]), [['Used skill', 'process-inbox', ''], ['Updated the steps', '', '1 of 3'], ['Read', 'raw_inbox/echo/2026-11-06-memo-pricing-thoughts.md', '']])
  assert.equal(r.tools[1].result.ok, true, 'each row with its result')
  assert.equal(r.tools[2].result, null, 'still reading')
  assert.equal(r.ended, null)
  const end = knRunView([...RUN_EVENTS, { n: 9, type: 'result', tokens_in: 41200, tokens_out: 3400, cost: 0.18, seconds: 160 }, { n: 10, type: 'notice', kind: 'ended', state: 'review', text: '5 changes to review' }])
  assert.deepEqual(end.ended, { state: 'review', text: '5 changes to review' })
  assert.equal(end.result.cost, 0.18)
  const failed = knRunView([{ n: 1, type: 'error', text: 'No credits left' }, { n: 2, type: 'notice', kind: 'ended', state: 'failed', text: 'No credits left' }])
  assert.deepEqual([failed.errors, failed.ended.state], [['No credits left'], 'failed'])
})

test('K14-02 the Add a source sheet: what each kind can add, and what it sends — never a file path', () => {
  const big = { name: 'workshop-recording.mp4', size: 212 << 20, too_big: true }
  const files = { tab: 'files', pick: { seq: 3, files: [{ name: 'Aurora RFP.pdf', size: 2200000 }, { name: 'pricing-workshop-notes.md', size: 14000 }, big] } }
  assert.deepEqual(knAddState(files), { can: true, label: 'Add 2 files', count: 2 })
  assert.deepEqual(knAddArgs(files), { kind: 'files', pick: 3 }, 'files by the number of their pick')
  assert.deepEqual(knAddState({ tab: 'files', pick: { seq: 4, files: [big] } }), { can: false, label: 'Add files', count: 0 })
  assert.deepEqual(knAddState({ tab: 'files', pick: null }), { can: false, label: 'Add files', count: 0 })
  assert.equal(knAddState({ ...files, busy: true }).can, false, 'once')
  const call = { tab: 'call', call: { name: '2026-11-05-090000-northwind-weekly-sync', title: 'Northwind · Weekly delivery sync' }, include: { transcript: true, chat: false } }
  assert.deepEqual(knAddState(call), { can: true, label: 'Add 1 source', count: 1 })
  assert.deepEqual(knAddArgs(call), { kind: 'call', call: '2026-11-05-090000-northwind-weekly-sync', transcript: true, chat: false, anyway: false })
  const other = { ...call, mismatch: { name: call.call.name, call: 'system', vault: 'work', call_name: 'Claude Code (as configured)', vault_name: 'Claude · Work' } }
  assert.equal(knAddState(other).label, 'Add anyway', 'on that call only')
  assert.equal(knAddArgs(other).anyway, true)
  const moved = { ...other, call: { name: '2026-11-04-140000-helios-design-review' } }
  assert.deepEqual([knAddState(moved).label, knAddArgs(moved).anyway], ['Add 1 source', false], 'another call picked: no anyway')
  assert.deepEqual(knAddState({ tab: 'call', call: null }), { can: false, label: 'Add a source', count: 0 })
  assert.deepEqual(knAddState({ tab: 'mail', hit: null }, { m365: false }), { can: false, signin: true, label: 'Add a source', count: 0 })
  const mail = { tab: 'mail', hit: { kind: 'mail', id: 'AAMkMadeUp1', subject: 'Phase 2 and the holiday freeze' } }
  assert.deepEqual([knAddState(mail).can, knAddArgs(mail)], [true, { kind: 'mail', message: 'AAMkMadeUp1' }])
  const chat = { tab: 'mail', hit: { kind: 'chats', id: 'm1', chat: '19:made-up@thread.v2' }, days: 7 }
  assert.deepEqual(knAddArgs(chat, '2026-11-07'), { kind: 'chat', chat: '19:made-up@thread.v2', since: '2026-11-01', until: '2026-11-07' }, 'the last 7 days')
  assert.deepEqual(knAddArgs({ ...chat, days: 30 }, '2026-03-01'), { kind: 'chat', chat: '19:made-up@thread.v2', since: '2026-01-31', until: '2026-03-01' })
  assert.deepEqual(knAddState({ tab: 'web', url: 'http://example.com/a' }), { can: false, label: 'Add the page', count: 0, note: 'Only an https address: Echo fetches nothing else.' })
  assert.deepEqual(knAddState({ tab: 'web', url: '' }), { can: false, label: 'Add the page', count: 0, note: '' })
  const web = { tab: 'web', url: '  https://www.example.com/reports/pricing  ' }
  assert.deepEqual([knAddState(web).can, knAddArgs(web)], [true, { kind: 'web', url: 'https://www.example.com/reports/pricing' }])
  for (const x of [files, call, other, mail, chat, web]) {
    const a = knAddArgs(x, '2026-11-07')
    assert.ok(!Object.keys(a).some((k) => /path|file|dir/.test(k)) && !Object.values(a).some((v) => typeof v === 'string' && /^(\/|~|[A-Za-z]:\\)/.test(v)), `no path in what is sent: ${JSON.stringify(a)}`)
  }
})

test('K14-09 the first open\'s question stands until it is answered, even after an add (R40)', () => {
  assert.deepEqual(knFirstOpen({ offer: { count: 41, folder: 'raw_inbox' }, sources: [] }), { count: 41, folder: 'raw_inbox', text: '41 files are already in raw_inbox/. Were they processed?' })
  const added = { offer: { count: 41, folder: 'raw_inbox' }, sources: [SRC('raw_inbox/echo/2026-11-05-northwind-sync.md', 'echo', 'Northwind · Weekly delivery sync', 'new')] }
  assert.equal(knFirstOpen(added).count, 41, 'an add first doesn\'t take it away')
  assert.equal(knFirstOpen({ offer: { count: 1, folder: 'inbox' } }).text, '1 file is already in inbox/. Was it processed?')
  assert.equal(knFirstOpen({ offer: { count: 1204, folder: 'raw_inbox' } }).text, '1,204 files are already in raw_inbox/. Were they processed?')
  assert.equal(knFirstOpen({ offer: null, sources: [] }), null, 'answered: never asked again')
  assert.equal(knFirstOpen(null), null)
})

test('K15-09 what Write did, in Echo\'s words: written, changed in the vault since the run, the commit', () => {
  assert.deepEqual(knWriteSummary({ written: ['knowledge/clients/a.md', 'knowledge/CATALOG.md', 'log.md', 'knowledge/notes/b.md'], changed_since: [], commit: '3f9c2d1', kept: 4, of: 5, finished: true }, 'My twin'),
    { text: 'Wrote 4 files into My twin. Committed (3f9c2d1). The run is done: 4 of its 5 changes kept.', wait: false })
  assert.deepEqual(knWriteSummary({ written: ['log.md'], changed_since: [], commit: 'off', kept: 1, of: 3, finished: false }, 'My twin'), { text: 'Wrote log.md into My twin.', wait: false })
  const stale = knWriteSummary({ written: ['log.md'], changed_since: ['knowledge/clients/a.md'], commit: 'skipped: the file had changes of yours that weren\'t committed', kept: 1, of: 2, finished: false }, 'My twin')
  assert.equal(stale.text, 'Wrote log.md into My twin. Not committed: the file had changes of yours that weren\'t committed. knowledge/clients/a.md changed in the vault since the run, so it wasn\'t written: compare it below.')
  assert.equal(stale.wait, true, 'something wasn\'t written: said until OK')
  assert.deepEqual(knWriteSummary({ written: [], changed_since: [], commit: 'off', kept: 0, of: 1, finished: false }, 'My twin'), { text: 'Nothing was written.', wait: true })
  // X-G2: a file with changes of yours before the write is written, not committed — said, and waits for OK.
  const left = knWriteSummary({ written: ['knowledge/topics/a.md', 'log.md'], committed: ['log.md'], changed_since: [], commit: '9a37fc7', kept: 2, of: 3, finished: false }, 'My twin')
  assert.deepEqual(left, { text: 'Wrote 2 files into My twin. Committed (9a37fc7), but not knowledge/topics/a.md: it had changes of yours.', wait: true })
})

// ── Ingest, fix round 1 ──
test('I1 a run\'s poll never takes the view from the run opened since; one load of a run at a time; an answer merged once', () => {
  const g = { run: null, review: null, open: {} }
  const a = knRunOpen(g, 'A')
  a.data = { state: 'running' }
  assert.equal(knRunOpen(g, 'A'), a, 'the same run stays open, its events kept')
  assert.equal(knRunMayLoad(g, a), true)
  a.loading = true
  assert.equal(knRunMayLoad(g, a), false, 'a load of A is on its way: no second one from the same point (Stop\'s loop and the poll)')
  a.loading = false
  g.review = { run: 'A' }; g.open = { t1: true }
  const b = knRunOpen(g, 'B')
  assert.deepEqual([g.run === b, g.review, g.open], [true, null, {}], 'B opens afresh')
  assert.equal(knRunMayLoad(g, a), false, 'A\'s poll waking after the switch may not load')
  assert.equal(knRunTake(g, a, 0, { events: [{ n: 1, type: 'todo', items: [] }], next: 1, state: 'running' }, 5), null, 'an answer for A lands nowhere')
  assert.equal(g.run, b, 'B stays the run shown')
  const got = knRunTake(g, b, 0, { events: [{ n: 1, type: 'notice', kind: 'copy', text: 'Not in the copy: .git/' }], next: 1, state: 'review' }, 5)
  assert.deepEqual([got === b, b.next, b.events.length, b.data.state, 'events' in b.data, b.loaded], [true, 1, 1, 'review', false, true])
  assert.equal(knRunTake(g, b, 0, { events: [{ n: 1, type: 'notice' }], next: 1, state: 'review' }, 6), null, 'asked from the same point twice: merged once')
  assert.equal(b.events.length, 1)
  knRunTake(g, b, 1, { events: [{ n: 2, type: 'todo', items: [] }], next: 2, state: 'review' }, 7)
  assert.deepEqual([b.events[1].at, b.events[0].at], [7, undefined], 'what comes while it is watched carries when it came')
})

test('I2 the page takes dropped files only while Ingest shows or the Add a source sheet is open', () => {
  const on = { view: 'brain', active: true, tab: 'ingest', vault: true, sheet: null }
  assert.equal(knTakesDrops(on), true)
  assert.equal(knTakesDrops({ ...on, tab: 'pages' }), false, 'another tab of the vault')
  assert.equal(knTakesDrops({ ...on, view: 'projects' }), false, 'the threads, whose logo tiles take dropped images')
  assert.equal(knTakesDrops({ ...on, active: false }), false, 'Recall or a plugin\'s tab')
  assert.equal(knTakesDrops({ ...on, vault: false }), false)
  assert.equal(knTakesDrops({ ...on, view: 'calls', tab: 'pages', sheet: 'source' }), true, 'the sheet open, wherever')
  assert.equal(knTakesDrops({ ...on, tab: 'pages', sheet: 'newpage' }), false, 'another sheet')
})

test('M4 folders dropped are said, never swallowed', () => {
  assert.equal(knDropNote({ seq: null, files: [], left_out: 2 }), 'Folders aren\'t added: drop the files in them.')
  assert.equal(knDropNote({ seq: 4, files: [{ name: 'a.md' }], left_out: 1 }), '1 folder left out: folders aren\'t added.')
  assert.equal(knDropNote({ seq: 4, files: [{ name: 'a.md' }], left_out: 3 }), '3 folders left out: folders aren\'t added.')
  assert.equal(knDropNote({ seq: 4, files: [{ name: 'a.md' }] }), '')
})

test('M2 a source Done by a write while its run still waits for review is In review (Prepare again would be refused)', () => {
  const runs = [{ id: 'r1', state: 'review' }, { id: 'r0', state: 'done' }]
  assert.equal(knSourceState({ path: 'raw_inbox/a.md', state: 'done', meta: { run: 'r1', kept: 1 } }, runs), 'review')
  assert.equal(knSourceState({ path: 'raw_inbox/b.md', state: 'done', meta: { run: 'r0', kept: 1 } }, runs), 'done')
  assert.equal(knSourceState({ path: 'raw_inbox/c.md', state: 'done', meta: {} }, runs), 'done')
  const q = knIngestQueue([{ path: 'raw_inbox/a.md', kind: 'file', title: 'a', state: 'done', meta: { run: 'r1', kept: 1 } }], runs)
  assert.deepEqual([q.groups[0].rows[0].chip, q.counts.waiting, q.counts.done], [['wait', 'In review'], 1, 0])
})

// ── Home (spec-2 K10, K11) ──
test('K10-T1 routes: a vault opens on Home; Ask, Ingest and Health name what they show; slice 1\'s as before', () => {
  assert.deepEqual(knParseRoute('kn/my-twin'), { vault: 'my-twin', tab: 'home', path: '' })
  assert.deepEqual(knParseRoute('kn/my-twin/home'), { vault: 'my-twin', tab: 'home', path: '' })
  assert.deepEqual(knParseRoute('kn/my-twin/ask/6f2c9e1a-1b2c-4d5e-8f90-123456789abc'), { vault: 'my-twin', tab: 'ask', path: '6f2c9e1a-1b2c-4d5e-8f90-123456789abc' })
  assert.deepEqual(knParseRoute('kn/my-twin/ingest/20261106-0951'), { vault: 'my-twin', tab: 'ingest', path: '20261106-0951' })
  assert.deepEqual(knParseRoute('kn/my-twin/health/broken%3Aa'), { vault: 'my-twin', tab: 'health', path: 'broken:a' })
  assert.deepEqual(knParseRoute('kn/my-twin/pages/knowledge%2Fclients%2Facme.md'), { vault: 'my-twin', tab: 'pages', path: 'knowledge/clients/acme.md' })
  assert.deepEqual(knParseRoute('kn/my-twin/graph'), { vault: 'my-twin', tab: 'graph', path: '' })
  assert.deepEqual(knParseRoute('kn/my-twin/settings'), { vault: 'my-twin', tab: 'settings', path: '' })
  assert.equal(knParseRoute('kn/my-twin/nope'), null)
  assert.equal(knRouteOf({ vault: 'my-twin' }), 'kn/my-twin/home', 'a vault\'s own route is its Home')
  assert.equal(knRouteOf({ vault: 'my-twin', tab: 'pages' }), 'kn/my-twin/pages')
})

// Saturday 7 November 2026, 11:00 here.
const NOW = new Date(2026, 10, 7, 11, 0)
const DAY = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
const HOME = (o = {}) => ({
  health: {
    score: 82, code_only: true, history: [], ai: null, needs: 3,
    top: [
      { key: 'contradiction:knowledge/clients/northwind-retail.md,knowledge/notes/2026-11-05-northwind-weekly-delivery-sync.md#1a2b3c4d', title: 'Northwind go-live: 12 Dec or 9 Jan', kind: 'contradiction', by: 'ai', pages: [] },
      { key: 'unknown_tag:knowledge/media/tokenomics.md:#tokenomics', title: '#tokenomics isn\'t in the taxonomy', kind: 'unknown_tag', by: 'code', pages: [] },
      { key: 'duplicate_title:knowledge/activities/weekly-sync.md,knowledge/notes/weekly-sync.md', title: '“Weekly sync” is the title of 2 pages', kind: 'duplicate_title', by: 'code', pages: [] },
    ],
    fixable: 26, fixable_kinds: [['broken', 12], ['not_in_index', 7], ['no_props', 7]],
  },
  coming_in: { waiting: 0, runs: [], new: 0, kinds: [], running: { id: '20261107-1010', step: 'Read the sources', waiting: null, sources: ['pricing-workshop-notes.md'] }, offer: null },
  ...o,
})

test('K11-T4 Home\'s cards: code checks only, three Needs you lines, and no "Nothing waiting" while a run goes', () => {
  const c = knHomeCards(HOME(), NOW)
  assert.equal(c.health.sub, 'code checks only · AI checks when you ask')
  assert.deepEqual(c.health.lines.map((l) => [l.title, l.sub]), [
    ['Northwind go-live: 12 Dec or 9 Jan', 'two pages disagree · AI'],
    ['#tokenomics isn\'t in the taxonomy', 'tags not in the taxonomy'],
    ['“Weekly sync” is the title of 2 pages', 'pages that share a title'],
  ])
  assert.equal(c.health.lines[0].key, HOME().health.top[0].key, 'a line opens its finding')
  assert.equal(c.health.fixable, '26 fixable by code — 12 broken links · 7 not in the index · 7 without properties')
  assert.deepEqual(c.coming.running, { run: '20261107-1010', step: 'Read the sources' })
  assert.equal(c.coming.nothing, false, 'a run goes: something is coming in')
  // Once an AI check has run, it is said when.
  const ai = knHomeCards(HOME({ health: { ...HOME().health, code_only: false, ai: new Date(2026, 10, 7, 9, 20).toISOString() } }), NOW)
  assert.equal(ai.health.sub, 'AI checks Today 09:20')
  // A run waiting for X-S2 says what it waits for.
  const wait = knHomeCards(HOME({ coming_in: { ...HOME().coming_in, running: { id: 'r3', step: null, waiting: 'Waiting for the call being recorded', sources: [] } } }), NOW)
  assert.equal(wait.coming.running.step, 'Waiting for the call being recorded')
  // Nothing at all.
  const idle = knHomeCards(HOME({ coming_in: { waiting: 0, runs: [], new: 0, kinds: [], running: null } }), NOW)
  assert.deepEqual([idle.coming.nothing, idle.coming.waiting, idle.coming.fresh, idle.coming.running], [true, null, null, null])
})

test('Coming in: changes waiting with their sources, sources not processed by kind; Review opens the one run waiting', () => {
  const two = knHomeCards(HOME({ coming_in: { waiting: 6, runs: [{ id: 'r1', to_answer: 5, sources: ['Northwind · Weekly delivery sync'] }, { id: 'r2', to_answer: 1, sources: ['Aurora RFP.pdf'] }], new: 3, kinds: [['echo', 1], ['mail', 1], ['web', 1]], running: null } }), NOW)
  assert.deepEqual(two.coming.waiting, { n: 6, sources: 'Northwind · Weekly delivery sync (5) · Aurora RFP.pdf (1)', run: '' })
  assert.deepEqual(two.coming.fresh, { n: 3, kinds: [['echo', 1], ['mail', 1], ['web', 1]] })
  assert.equal(two.coming.nothing, false)
  const one = knHomeCards(HOME({ coming_in: { waiting: 2, runs: [{ id: 'r1', to_answer: 2, sources: ['a.md', 'b.pdf'] }], new: 0, kinds: [], running: null } }), NOW)
  assert.deepEqual(one.coming.waiting, { n: 2, sources: 'a.md, b.pdf (2)', run: 'r1' })
  assert.equal(one.coming.fresh, null)
  assert.equal(knHomeCards(HOME({ coming_in: { error: 'ingest.json: unreadable' } }), NOW).coming.error, 'ingest.json: unreadable')
})

test('the Health card of a tidy vault: nothing to fix, no lines', () => {
  const c = knHomeCards(HOME({ health: { score: 100, code_only: true, history: [], ai: null, needs: 0, top: [], fixable: 0, fixable_kinds: [] } }), NOW)
  assert.deepEqual([c.health.score, c.health.nothing, c.health.lines, c.health.fixable], [100, true, [], ''])
})

test('when, as Home says it: today and yesterday with the time, the weekday this week, else the date', () => {
  const mon = new Date(2026, 10, 2, 11, 5)
  assert.equal(knWhen(new Date(2026, 10, 7, 10, 1).toISOString(), NOW), 'Today 10:01')
  assert.equal(knWhen(new Date(2026, 10, 6, 17, 20).toISOString(), NOW), 'Yesterday 17:20')
  assert.equal(knWhen(mon.toISOString(), NOW), `${DAY[mon.getDay()]} 11:05`)
  assert.equal(knWhen(new Date(2026, 9, 28, 9, 0).toISOString(), NOW), '28 Oct')
  assert.equal(knWhen(new Date(2025, 9, 28, 9, 0).toISOString(), NOW), '28 Oct 2025')
  assert.equal(knWhen(new Date(2026, 10, 7, 10, 1).toISOString(), NOW, true), '10:01', 'short: the time alone today')
  assert.equal(knWhen(new Date(2026, 10, 6, 17, 20).toISOString(), NOW, true), 'yesterday')
  assert.equal(knWhen(mon.toISOString(), NOW, true), DAY[mon.getDay()])
  assert.equal(knWhen(null, NOW), '')
  assert.equal(knWhen('not a time', NOW), '')
})

const NEWS = (path, kind, by, h, o = {}) => ({ path, title: path.split('/').pop(), kind, plus: 1, minus: 0, at: new Date(2026, 10, 7, h, 0).toISOString(), by, who: by, detail: '', ...o })
test('What\'s new: All · New pages · Notes, twelve at most, in Echo\'s order', () => {
  const rows = [
    NEWS('knowledge/activities/platform/delivery-platform.md', 'edit', 'ask', 10),
    NEWS('knowledge/notes/2026-11-05-northwind-weekly-delivery-sync.md', 'new', 'minutes', 9),
    NEWS('knowledge/topics/ai-strategy.md', 'edit', 'ingest', 8),
    NEWS('', 'new', 'starter', 7, { title: 'The vault' }),
    ...Array.from({ length: 14 }, (_, i) => NEWS(`knowledge/notes/2026-10-${String(10 + i)}-sync.md`, 'edit', 'outside', 6, { plus: null, minus: null })),
  ]
  assert.equal(knHomeNews(rows, 'all', 'knowledge/notes').length, 12)
  assert.deepEqual(knHomeNews(rows, 'all', 'knowledge/notes').slice(0, 3).map((r) => r.by), ['ask', 'minutes', 'ingest'])
  assert.deepEqual(knHomeNews(rows, 'new', 'knowledge/notes').map((r) => r.path), ['knowledge/notes/2026-11-05-northwind-weekly-delivery-sync.md'], 'new pages: the vault\'s own row is no page')
  const notes = knHomeNews(rows, 'notes', 'knowledge/notes')
  assert.equal(notes.length, 12)
  assert.ok(notes.every((r) => r.path.startsWith('knowledge/notes/')))
  assert.deepEqual(knHomeNews(rows, 'notes', ''), [], 'a vault without a notes folder has no notes')
  assert.deepEqual(knHomeNews(null, 'all', ''), [])
})

test('Continue: the mode Ask ran it in, when, and the terminal panel when it has it', () => {
  const mon = new Date(2026, 10, 2, 16, 0)
  assert.equal(knHomeLine({ mode: 'curate', at: new Date(2026, 10, 7, 10, 1).toISOString(), terminal: false }, NOW), 'Curate · 10:01')
  assert.equal(knHomeLine({ mode: 'research', at: mon.toISOString(), terminal: true }, NOW), `Research · ${DAY[mon.getDay()]} · in the terminal panel`)
  assert.equal(knHomeLine({ mode: null, at: new Date(2026, 10, 6, 9, 0).toISOString(), terminal: true }, NOW), 'yesterday · in the terminal panel', 'a panel session has no mode of Ask\'s')
  assert.equal(knHomeLine({ mode: 'ask', at: null }, NOW), 'Ask')
})

test('the Ask box: at most four suggestions, only those Echo gave; with none, its placeholder alone', () => {
  const s = (kind, text) => ({ kind, text, why: 'because', pages: [] })
  const a = knHomeAsk({ fresh: false, suggestions: [s('meeting', 'Brief me for Helios · Sprint review at 14:00'), s('note', 'What did we decide in Helios · Design review prep?'), { kind: 'x', text: '' }, s('actions', 'Which action items are still open?'), s('contradiction', 'Northwind go-live: 12 Dec or 9 Jan — which is right?'), s('more', 'A fifth')] }, 'My twin')
  assert.deepEqual(a.suggestions.map((x) => x.kind), ['meeting', 'note', 'actions', 'contradiction'], 'one without words is no suggestion; four at most')
  assert.equal(a.placeholder, 'Ask My twin — @ to mention a page')
  const none = knHomeAsk({ fresh: true, suggestions: [] }, 'My twin')
  assert.deepEqual(none, { placeholder: 'Ask My twin — it knows only its starter pages for now', suggestions: [] })
  assert.deepEqual(knHomeAsk({}, 'Team wiki').suggestions, [])
})

test('K11-09 a new vault\'s second step says where its minutes land, from the vault\'s minutes rule', () => {
  const paths = { notes: 'knowledge/notes', index: 'knowledge/CATALOG.md' }
  assert.deepEqual(knHomeRecord({ mode: 'auto' }, paths), { word: 'automatically', text: 'Its minutes are saved here by themselves: a note in notes/ and a row in CATALOG.md.', code: ['notes/', 'CATALOG.md'] })
  assert.deepEqual(knHomeRecord({ mode: 'ask' }, paths), { word: 'when you choose', text: 'After a call, its page offers to save its minutes here: a note in notes/ and a row in CATALOG.md.', code: ['notes/', 'CATALOG.md'] })
  assert.deepEqual(knHomeRecord({ mode: 'off' }, paths), { word: 'off', text: 'Minutes aren\'t saved here by themselves. Save one from its call\'s page when you want it.', code: [] })
  assert.equal(knHomeRecord({}, {}).word, 'off')
})

// ── Home, fix round 1 ──
test('I3 a send\'s answer opens on the composer it was sent from; the address names it only while Ask shows that one', () => {
  const ask = { id: '' }
  assert.deepEqual(knSentWhere(ask, { conversation: 'c1' }, { tab: 'ask', ask }), { open: true, route: true })
  assert.deepEqual(knSentWhere(ask, { conversation: 'c1' }, { tab: 'home', ask }), { open: true, route: false }, 'Home opened meanwhile: Ask\'s composer gets it all the same, the address stays Home\'s')
  assert.deepEqual(knSentWhere(ask, { conversation: 'c1' }, { tab: 'ask', ask: { id: '' } }), { open: true, route: false }, 'Ask shows another composer now')
  assert.deepEqual(knSentWhere({ id: 'c0' }, { conversation: 'c0' }, { tab: 'ask', ask }), { open: false, route: false }, 'a conversation going on: nothing to open')
  assert.deepEqual(knSentWhere(ask, null, { tab: 'ask', ask }), { open: false, route: false }, 'no answer')
  assert.deepEqual(knSentWhere(ask, { mismatch: {} }, { tab: 'ask', ask }), { open: false, route: false }, 'a call on another profile: not sent')
})

test('C2 Pages waits for the vault\'s pages before it says there is no index page', () => {
  assert.equal(knPagesEmpty(null, 'team-twin'), 'loading')
  assert.equal(knPagesEmpty({ vault: 'my-twin', index: 'knowledge/CATALOG.md' }, 'team-twin'), 'loading', 'another vault\'s pages')
  assert.equal(knPagesEmpty({ vault: 'team-twin', index: 'knowledge/CATALOG.md' }, 'team-twin'), 'pick')
  assert.equal(knPagesEmpty({ vault: 'team-twin', index: null }, 'team-twin'), 'no-index')
})

test('F2 Pages says when the vault\'s pages couldn\'t be listed — not a skeleton forever', () => {
  assert.equal(knPagesEmpty(null, 'team-twin', { vault: 'team-twin', text: '500 api/kn/team-twin/tree' }), 'failed')
  assert.equal(knPagesEmpty({ vault: 'my-twin', index: 'knowledge/CATALOG.md' }, 'team-twin', { vault: 'team-twin', text: 'the folder isn\'t there' }), 'failed', 'another vault\'s pages are no answer')
  assert.equal(knPagesEmpty(null, 'team-twin', { vault: 'my-twin', text: 'x' }), 'loading', 'another vault\'s failure')
  assert.equal(knPagesEmpty({ vault: 'team-twin', index: 'knowledge/CATALOG.md' }, 'team-twin', { vault: 'team-twin', text: 'x' }), 'pick', 'listed since')
  assert.equal(knPagesEmpty(null, 'team-twin', null), 'loading')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`knowledge: ${tests.length} passed`)
