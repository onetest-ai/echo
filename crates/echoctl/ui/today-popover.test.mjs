// A meeting of Your day opened as Calendar's popover (app.js, the today popover block): which meeting has its card
// open, and beside which of its forms. Run by scripts/check-mod.sh (`node crates/echoctl/ui/today-popover.test.mjs`):
// the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── today popover:')
const end = src.indexOf('// end of the today popover block', start)
assert.ok(start >= 0 && end > start, 'the today popover block not found in app.js')
const { tdFormOf, tdPick } = new Function(`${src.slice(start, end)}\nreturn { tdFormOf, tdPick }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('the form is what was clicked: the agenda line, a calendar or strip block, an invite line, a name at once', () => {
  assert.deepEqual([['ag-row', 'past'], ['dc-ev', 'yes'], ['ds-ev', 'maybe'], ['ch-row'], ['ch-m']].map(tdFormOf), ['ag-row', 'dc-ev', 'ds-ev', 'ch-row', 'ch-m'])
  assert.equal(tdFormOf(['ev', 'online']), null)
})

test('a click opens the card beside what was clicked; another meeting, or the same in another form, moves it there', () => {
  const a = tdPick(null, 'ev1', 'ag-row')
  assert.deepEqual(a, { id: 'ev1', at: 'ag-row' })
  assert.deepEqual(tdPick(a, 'ev2', 'ag-row'), { id: 'ev2', at: 'ag-row' })
  assert.deepEqual(tdPick(a, 'ev1', 'ch-row'), { id: 'ev1', at: 'ch-row' })
})

test('the same clicked again closes it, as a block clicked again in Calendar', () => {
  assert.equal(tdPick({ id: 'ev1', at: 'dc-ev' }, 'ev1', 'dc-ev'), null)
})

test("the menu's Show details opens it, even on the one open", () => {
  assert.deepEqual(tdPick({ id: 'ev1', at: 'ag-row' }, 'ev1', 'ag-row', false), { id: 'ev1', at: 'ag-row' })
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
