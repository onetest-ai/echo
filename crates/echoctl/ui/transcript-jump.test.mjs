// Which transcript turn a moment opens on (app.js, jumpTurn): a minutes time chip or a note's time.
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/transcript-jump.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── transcript jump')
const end = src.indexOf('// end of the transcript jump block', start)
assert.ok(start >= 0 && end > start, 'the transcript jump block not found in app.js')
const { jumpTurn } = new Function(`${src.slice(start, end)}\nreturn { jumpTurn }`)()

// The turns of the made-up Acme weekly sync around the chips of #101.
const turns = [
  { start: 5, end: 12 }, { start: 13.2, end: 20 }, { start: 21, end: 28 }, { start: 29, end: 35.4 }, { start: 37.3, end: 44 }, { start: 50.666, end: 58 },
]
const cases = [
  ['a chip at a turn that starts later in the same second opens that turn, not the one before', 37, 4],
  ['a chip at 00:13 opens the turn starting at 0:13.2', 13, 1],
  ['a moment inside a turn opens that turn', 31, 3],
  ['a moment in the silence after a turn opens the next turn', 36, 4],
  ['a note saved at a turn\'s start rounded up opens that turn', Math.ceil(50.666), 5],
  ['a moment before the first turn opens the first', 1, 0],
  ['a moment after the last turn opens the last', 99, 5],
]
let failed = 0
for (const [name, t, want] of cases) {
  try { assert.equal(jumpTurn(turns, t), want); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
