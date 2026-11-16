// A client page's tabs and their counts (app.js, clientTabs). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/client-tabs.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── client tabs:')
const end = src.indexOf('// end of the client tabs block', start)
assert.ok(start >= 0 && end > start, 'the client tabs block not found in app.js')
const { clientTabs } = new Function(`${src.slice(start, end)}\nreturn { clientTabs }`)()

const label = (tabs, id) => tabs.find(([t]) => t === id)[1]
// A count shows when there is something to count; a zero shows no number, on every tab alike (#173).
assert.equal(label(clientTabs([], 0, 0), 'calls'), 'Calls')
assert.equal(label(clientTabs([], 0, 0), 'tasks'), 'Tasks')
assert.equal(label(clientTabs([], 0, 0), 'people'), 'People')
// Not loaded yet: no number either.
assert.equal(label(clientTabs(null, 0, 0), 'calls'), 'Calls')
assert.equal(label(clientTabs([{}, {}], 3, 1), 'calls'), 'Calls<span class="n">2</span>')
assert.equal(label(clientTabs([{}, {}], 3, 1), 'tasks'), 'Tasks<span class="n">3</span>')
assert.equal(label(clientTabs([{}, {}], 3, 1), 'people'), 'People<span class="n">1</span>')
assert.deepEqual(clientTabs([], 0, 0).map(([t]) => t), ['overview', 'calls', 'tasks', 'people', 'profile'])
console.log('client-tabs: ok')
