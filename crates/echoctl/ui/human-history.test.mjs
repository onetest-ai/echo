// History's line for the user's own ledger edits (app.js, humanHistory). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/human-history.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── human history: what the user did to an item, in words')
const end = src.indexOf('// end of the human history block', start)
assert.ok(start >= 0 && end > start, 'the human history block not found in app.js')
const { humanHistory } = new Function(`${src.slice(start, end)}\nreturn { humanHistory }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('ticking and unticking a criterion are said in words, not "ac_set" (#153)', () => {
  assert.equal(humanHistory({ op: 'ac_set', ac: 1, met: true }), 'you ticked criterion 1')
  assert.equal(humanHistory({ op: 'ac_set', ac: 2, met: false, ac_text: 'SAML works' }), 'you unticked criterion 2 (SAML works)')
  assert.equal(humanHistory({ op: 'ac_set', ac: 1, text: 'SAML works for Berlin users' }), 'you reworded criterion 1: SAML works for Berlin users')
  assert.equal(humanHistory({ op: 'ac_set', ac: 3 }), 'you dropped criterion 3')
})

test('every other human op has its words', () => {
  assert.equal(humanHistory({ op: 'ac_add', text: 'Help desk trained' }), 'you added a criterion: Help desk trained')
  assert.equal(humanHistory({ op: 'link', depends_on: 'T002' }), 'you made it wait for T002')
  assert.equal(humanHistory({ op: 'unlink', depends_on: 'T002' }), 'you took away its wait for T002')
  assert.equal(humanHistory({ op: 'merge', into: 'T004' }), 'you merged it into T004')
  assert.equal(humanHistory({ op: 'delete' }), 'you deleted it')
  assert.equal(humanHistory({ op: 'create' }), 'you created it')
})

test('an edit names what was set; a description by the item\'s own word', () => {
  assert.equal(humanHistory({ op: 'edit', set: { state: 'done', due_date: null } }), 'you set state, due date')
  assert.equal(humanHistory({ op: 'edit', set: { description: 'x' } }, 'the answer'), 'you set the answer')
})

test('no op is shown raw, whatever comes', () => {
  for (const op of ['ac_add', 'ac_set', 'link', 'unlink', 'merge', 'delete', 'create', 'edit', 'something_new']) {
    const line = humanHistory({ op, ac: 1, set: {} })
    assert.ok(!line.includes(`you: ${op}`) && !/\b[a-z]+_[a-z]+\b/.test(line), `${op}: ${line}`)
    assert.ok(line.startsWith('you '), line)
  }
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`human history: ${tests.length} passed`)
