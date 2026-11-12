// Echo's suggestions for a text box (app.js, the suggest block): which rows show for what is typed.
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/suggest.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── suggest\n')
const end = src.indexOf('// end of the suggest block', start)
assert.ok(start >= 0 && end > start, 'the suggest block not found in app.js')
const { suggestRows } = new Function(`${src.slice(start, end)}\nreturn { suggestRows }`)()

const opts = [
  { value: 'Me', sub: 'you' },
  { value: 'Dana Lee', sub: 'sponsor' },
  { value: 'Sam Rivera', sub: 'initech.example' },
  { value: 'Adam Lee', sub: '' },
  { value: '', label: 'Nobody named' },
]
const names = (rows) => rows.map((r) => r.label)
assert.deepEqual(names(suggestRows(opts, '')), ['Me', 'Dana Lee', 'Sam Rivera', 'Adam Lee', 'Nobody named'], 'nothing typed: all, nobody last')
assert.deepEqual(names(suggestRows(opts, 'lee')), ['Dana Lee', 'Adam Lee', 'Nobody named'], 'by a part of the name, any case')
assert.deepEqual(names(suggestRows(opts, 'a')), ['Adam Lee', 'Me', 'Dana Lee', 'Sam Rivera', 'Nobody named'].filter((n) => n !== 'Me'), 'the ones starting with it first')
assert.deepEqual(names(suggestRows(opts, 'initech')), ['Sam Rivera', 'Nobody named'], 'by its line too')
assert.deepEqual(names(suggestRows(opts, 'zz')), ['Nobody named'], 'nothing else: only the clearing row')
assert.equal(suggestRows(opts, '').at(-1).empty, true)
assert.deepEqual(names(suggestRows([{ value: 'Ops' }, { value: 'Sales' }], 'x')), [], 'no clearing row: none')
console.log('suggest: ok')
