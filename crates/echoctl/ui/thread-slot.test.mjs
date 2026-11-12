// The plugins' rows of a thread's Profile (app.js, threadSlotRows): each drawn for the thread, one that throws left out
// and logged while the others draw. Run by scripts/check-mod.sh (`node crates/echoctl/ui/thread-slot.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── thread slot:'), b = src.indexOf('// end of the thread slot block', a)
assert.ok(a >= 0 && b > a, 'the thread slot block not found in app.js')
const rows = new Function(`${src.slice(a, b)}\nreturn threadSlotRows`)()
assert.ok(src.includes("'thread.profile']"), 'thread.profile is one of SLOTS')

const thread = { id: 'alpha', name: 'Alpha', client: 'acme' }
const logged = []
const items = [
  { plugin: 'gh', id: 'repos', label: 'Repositories', render: (t) => `<b>${t.id}</b> of ${t.client}` },
  { plugin: 'boom', id: 'x', label: 'Broken', render: () => { throw new Error('nope') } },
  { plugin: 'jira', id: 'project', label: 'Jira', render: () => undefined },
]
const got = rows(items, thread, (m) => logged.push(m))
assert.deepEqual(got.map((r) => [r.key, r.label, r.html]), [['gh:repos', 'Repositories', '<b>alpha</b> of acme'], ['jira:project', 'Jira', '']])
assert.deepEqual(logged, ['boom: Broken: nope'], 'the one that threw is logged, the rest drawn')
assert.deepEqual(rows(undefined, thread, () => {}), [], 'no plugin rows: none')
console.log('thread-slot: ok')
