// Where My notes' list is after a redraw (app.js, notesScroll). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/notes-scroll.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── notes scroll: ')
const end = src.indexOf('// end of the notes scroll block', start)
assert.ok(start >= 0 && end > start, 'the notes scroll block not found in app.js')
const { notesScroll } = new Function(`${src.slice(start, end)}\nreturn { notesScroll }`)()

const CALL = '2026-11-10_090000-acme-sync'
const tests = []
const test = (name, f) => tests.push([name, f])

// The report: while a call recorded, every live refresh drew the panel again and the list jumped to its top.
test('a list scrolled up from its end stays where the user scrolled it, after a redraw', () => {
  assert.equal(notesScroll({ call: CALL, top: 400, height: 881, client: 202 }, { call: CALL, height: 881 }), 400)
  assert.equal(notesScroll({ call: CALL, top: 400, height: 881, client: 202 }, { call: CALL, height: 960 }), 400, 'a live item added below')
})

test('a list at its end follows the end as new entries come', () => {
  assert.equal(notesScroll({ call: CALL, top: 679, height: 881, client: 202 }, { call: CALL, height: 960 }), 960)
  assert.equal(notesScroll({ call: CALL, top: 660, height: 881, client: 202 }, { call: CALL, height: 960 }), 960, 'a few pixels short of the end')
})

test('a list that did not scroll yet follows the end once it does', () => {
  assert.equal(notesScroll({ call: CALL, top: 0, height: 150, client: 202 }, { call: CALL, height: 260 }), 260)
})

test("another call's list opens at its top (nothing kept)", () => {
  assert.equal(notesScroll({ call: CALL, top: 400, height: 881, client: 202 }, { call: '2026-11-10_120000-globex-pilot', height: 500 }), null)
  assert.equal(notesScroll(null, { call: CALL, height: 500 }), null, 'no list before (the panel just opened)')
  assert.equal(notesScroll({ call: CALL, top: 400, height: 881, client: 202 }, { call: CALL.slice(0, 17) + '-acme-weekly-sync-renamed', height: 881 }), 400, 'the same call, retitled while it records')
})

let failed = 0
for (const [name, f] of tests) {
  try { await f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`notes scroll: ${tests.length} passed`)
