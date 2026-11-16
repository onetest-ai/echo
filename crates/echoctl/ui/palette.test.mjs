// The ⌘K palette's ranking (app.js, Palette). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/palette.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── ⌘K palette: ranking what was typed')
const end = src.indexOf('// end of the palette block', start)
assert.ok(start >= 0 && end > start, 'the palette block not found in app.js')
const { Palette } = new Function(`${src.slice(start, end)}\nreturn { Palette }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const labels = (xs) => xs.map((x) => x.label)
const ITEMS = [
  { label: 'Write minutes again' },
  { label: 'Ask Claude about this call', keywords: 'question terminal' },
  { label: 'Open Transcript' },
  { label: 'Record the next call', keywords: 'start arm' },
  { label: 'Settings: Transcription' },
  { label: 'Bruno - Acme follow-up', keywords: 'Pat Novak Ivo Brandt Today' },
  { label: 'Delete this call…', disabled: null },
  { label: 'Open minutes in browser', disabled: 'no minutes yet' },
]

test('no query: the items in their own order', () => {
  assert.deepEqual(labels(Palette.rank(ITEMS, '')), labels(ITEMS))
  assert.equal(Palette.rank(ITEMS, '  ', 3).length, 3)
})

test('every word must appear, in any order, in the label or the keywords', () => {
  assert.deepEqual(labels(Palette.rank(ITEMS, 'call claude')), ['Ask Claude about this call'])
  assert.deepEqual(labels(Palette.rank(ITEMS, 'novak')), ['Bruno - Acme follow-up'])
  assert.deepEqual(labels(Palette.rank(ITEMS, 'zzz')), [])
})

test('a word start ranks before the middle of a word, an earlier match before a later one', () => {
  assert.deepEqual(labels(Palette.rank(ITEMS, 'tran')), ['Open Transcript', 'Settings: Transcription'])
  assert.equal(labels(Palette.rank(ITEMS, 'min'))[0], 'Write minutes again')
})

test('unavailable items come after available ones', () => {
  assert.deepEqual(labels(Palette.rank(ITEMS, 'minutes')), ['Write minutes again', 'Open minutes in browser'])
})

test('keywords match only at a word start; labels anywhere', () => {
  assert.deepEqual(labels(Palette.rank(ITEMS, 'min')).includes('Ask Claude about this call'), false, '"min" in "terminal"')
  assert.deepEqual(labels(Palette.rank(ITEMS, 'term')), ['Ask Claude about this call'])
})

test('each group stays together, groups in the order of their best item', () => {
  const xs = [{ label: 'b1', group: 'B' }, { label: 'a1', group: 'A' }, { label: 'b2', group: 'B' }]
  assert.deepEqual(labels(Palette.grouped(xs)), ['b1', 'b2', 'a1'])
})

test('accents and case do not matter', () => {
  const xs = [{ label: 'Inés Peña Gómez' }]
  assert.deepEqual(labels(Palette.rank(xs, 'ines pena')), ['Inés Peña Gómez'])
  assert.deepEqual(labels(Palette.rank(xs, 'INÉS')), ['Inés Peña Gómez'])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`palette: ${tests.length} passed`)
