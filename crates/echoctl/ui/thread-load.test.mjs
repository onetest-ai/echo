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
async function open(tab) {
  const asked = []
  const S = { pj: { sel: 'acme-rollout', tab, work: null, detail: null, tbrain: { thread: 'acme-rollout' } } }
  const get = async (url) => {
    asked.push(url)
    if (url === 'api/project/acme-rollout') return { id: 'acme-rollout', call_count: 1 }
    if (url === 'api/project/acme-rollout/work') return { nodes: { T001: { kind: 'task', state: 'open' } } }
    return {}
  }
  const noop = async () => {}
  const { loadProject } = new Function('S', 'get', 'loadFound', 'loadPeople', 'loadThreadBrain',
    `${src.slice(start, end)}\nreturn { loadProject }`)(S, get, noop, noop, noop)
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

let failed = 0
for (const [name, f] of tests) {
  try { await f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`thread load: ${tests.length} passed`)
