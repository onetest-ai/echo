// What opening a thread loads (app.js, loadProject). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/thread-load.test.mjs`): the block is taken from app.js as it ships, the app's
// requests answered here.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── thread load: what opening a thread asks the app for')
const end = src.indexOf('// end of the thread load block', start)
assert.ok(start >= 0 && end > start, 'the thread load block not found in app.js')

/** loadProject on one of a thread's tabs: the state it leaves and the paths it asked for. */
async function open(tab, work = null) {
  const asked = []
  const S = { pj: { sel: 'acme-rollout', tab, work, detail: null } }
  const get = async (url) => {
    asked.push(url)
    if (url === 'api/project/acme-rollout') return { id: 'acme-rollout', call_count: 1 }
    if (url === 'api/project/acme-rollout/work') return { nodes: { T001: { kind: 'task', state: 'open' } } }
    return {}
  }
  const noop = async () => {}
  const { loadProject } = new Function('S', 'get', 'loadFound', 'loadPeople',
    `${src.slice(start, end)}\nreturn { loadProject }`)(S, get, noop, noop)
  await loadProject()
  return { S, asked }
}

const tests = []
const test = (name, f) => tests.push([name, f])

// The tab row counts the thread's open work from its work tree: the tree comes with the thread on every
// tab, as the call count does, not only once the Work tab has been opened (#110).
test('the work tree comes with the thread on every tab (#110)', async () => {
  for (const tab of ['brief', 'work', 'calls', 'people', 'profile']) {
    const { S, asked } = await open(tab)
    assert.ok(asked.includes('api/project/acme-rollout/work'), `${tab}: asked ${asked.join(', ')}`)
    assert.ok(S.pj.work && S.pj.work.nodes.T001, tab)
  }
})

test('once the tree is there, only the Work tab reads it again (it is costly on a big ledger)', async () => {
  const had = { nodes: {} }
  for (const tab of ['brief', 'calls', 'people', 'profile']) {
    const { S, asked } = await open(tab, had)
    assert.ok(!asked.includes('api/project/acme-rollout/work'), `${tab}: asked ${asked.join(', ')}`)
    assert.equal(S.pj.work, had, tab)
  }
  const { asked } = await open('work', had)
  assert.ok(asked.includes('api/project/acme-rollout/work'), 'the Work tab reads it again')
})

// A view change (layout, filters) is saved over IPC; a reload asked before it, or just after it, may answer with the
// board as it was. Its answer keeps the board as the user changed it, or the next change is saved on the old one (#321).
async function reload(boardAt, served) {
  const S = { pj: { sel: 'acme-rollout', tab: 'brief', work: { nodes: {} }, boardAt, detail: { id: 'acme-rollout', board: { views: [{ id: 'board', layout: 'tree' }] } } } }
  const get = async (url) => url === 'api/project/acme-rollout' ? { id: 'acme-rollout', call_count: 2, board: served } : {}
  const noop = async () => {}
  const { loadProject } = new Function('S', 'get', 'loadFound', 'loadPeople', `${src.slice(start, end)}\nreturn { loadProject }`)(S, get, noop, noop)
  await loadProject()
  return S.pj.detail
}
const OLD = { views: [{ id: 'board', layout: 'board' }] }

test('a reload asked around a board change keeps the board as changed, and the rest of its answer (#321)', async () => {
  for (const ago of [0, 500, 1500]) {
    const d = await reload(Date.now() - ago, OLD)
    assert.equal(d.board.views[0].layout, 'tree', `changed ${ago} ms before the reload was asked`)
    assert.equal(d.call_count, 2, 'the rest is the reload\'s')
  }
  // Changed while the reload was in flight: after it was asked.
  const d = await reload(Date.now() + 50, OLD)
  assert.equal(d.board.views[0].layout, 'tree')
})

test('a reload asked well after the last board change takes the board it is given (an outside change shows)', async () => {
  for (const boardAt of [undefined, Date.now() - 10_000]) {
    const d = await reload(boardAt, OLD)
    assert.equal(d.board.views[0].layout, 'board', String(boardAt))
  }
})

test('a thread just opened (no detail yet) takes its own board, whatever was changed on the last one', async () => {
  const S = { pj: { sel: 'acme-rollout', tab: 'brief', work: { nodes: {} }, boardAt: Date.now(), detail: null } }
  const get = async (url) => url === 'api/project/acme-rollout' ? { id: 'acme-rollout', board: OLD } : {}
  const noop = async () => {}
  const { loadProject } = new Function('S', 'get', 'loadFound', 'loadPeople', `${src.slice(start, end)}\nreturn { loadProject }`)(S, get, noop, noop)
  await loadProject()
  assert.equal(S.pj.detail.board, OLD)
})

let failed = 0
for (const [name, f] of tests) {
  try { await f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`thread load: ${tests.length} passed`)
