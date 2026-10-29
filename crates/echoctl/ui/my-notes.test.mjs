// My notes as moments (app.js, NoteDoc): reading and writing note lines of my-notes.md. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/my-notes.test.mjs`): the block is taken from app.js
// as it ships.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const src = readFileSync(new URL('./app.js', import.meta.url), 'utf8')
const start = src.indexOf('// ── my notes as moments')
const end = src.indexOf('// end of the notes block', start)
assert.ok(start >= 0 && end > start, 'the notes block not found in app.js')
const { NoteDoc } = new Function(`${src.slice(start, end)}\nreturn { NoteDoc }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const notes = (text) => NoteDoc.parse(text).filter((b) => b.note).map(({ kind, t, done, text, quote }) => ({ kind, t, done, text, quote }))

test('every kind of note line is read, and anything else is kept as it is', () => {
  const text = '# Pricing\nGoal: agree scope\n- [12:05] ? Is legal OK?\n  > "I can have SAML by Friday."\n- [12:21] Ravi wants the diagram\n- [ ] [12:22] send diagram\n- [x] [1:02:03] done thing\n- [12:34] ⚑\n- [12:39] ! Lena signs off\n- general point\n'
  assert.deepEqual(notes(text), [
    { kind: 'question', t: 725, done: false, text: 'Is legal OK?', quote: 'I can have SAML by Friday.' },
    { kind: 'note', t: 741, done: false, text: 'Ravi wants the diagram', quote: null },
    { kind: 'todo', t: 742, done: false, text: 'send diagram', quote: null },
    { kind: 'todo', t: 3723, done: true, text: 'done thing', quote: null },
    { kind: 'flag', t: 754, done: false, text: '', quote: null },
    { kind: 'decision', t: 759, done: false, text: 'Lena signs off', quote: null },
    { kind: 'note', t: null, done: false, text: 'general point', quote: null },
  ])
  // Reading and writing back changes nothing.
  assert.equal(NoteDoc.parse(text).map((b) => b.raw).join('\n'), text)
})

test("the tray's lines (`- [mm:ss] text`) are notes at their time", () => {
  assert.deepEqual(notes('- [03:07] from the tray\n'), [{ kind: 'note', t: 187, done: false, text: 'from the tray', quote: null }])
})

test('each kind is written so it reads back the same', () => {
  for (const n of [
    { kind: 'note', t: 65, text: 'a' }, { kind: 'question', t: 65, text: 'b' }, { kind: 'decision', t: 3725, text: 'c' },
    { kind: 'todo', t: 65, text: 'd', done: true }, { kind: 'flag', t: 65, text: '' }, { kind: 'note', t: null, text: 'e', quote: 'said "x"\nhere' },
  ]) {
    const back = notes(NoteDoc.line(n) + '\n')[0]
    assert.equal(back.kind, n.kind)
    assert.equal(back.t, n.t)
    assert.equal(back.text, n.text)
    if (n.quote) assert.equal(back.quote, 'said ”x” here')
  }
})

test('a note goes in time order: a comment on an earlier moment lands there', () => {
  let text = '# Notes\n- [10:00] first\n- [20:00] third\n'
  text = NoteDoc.add(text, { kind: 'note', t: 900, text: 'second' })
  text = NoteDoc.add(text, { kind: 'note', t: null, text: 'general' })
  text = NoteDoc.add(text, { kind: 'question', t: 1500, text: 'last?' })
  assert.equal(text, '# Notes\n- [10:00] first\n- [15:00] second\n- [20:00] third\n- general\n- [25:00] ? last?\n')
  assert.equal(NoteDoc.add('', { kind: 'flag', t: 5, text: '' }), '- [00:05] ⚑\n')
})

test('a note is changed or removed in place, quote line and all', () => {
  const text = 'top\n- [12:05] ? q\n  > "said"\n- [ ] [12:22] todo\n'
  const blocks = NoteDoc.parse(text)
  const qi = blocks.findIndex((b) => b.kind === 'question'), ti = blocks.findIndex((b) => b.kind === 'todo')
  assert.equal(NoteDoc.change(text, ti, { done: true }), 'top\n- [12:05] ? q\n  > "said"\n- [x] [12:22] todo\n')
  assert.equal(NoteDoc.change(text, qi, null), 'top\n- [ ] [12:22] todo\n')
  assert.equal(NoteDoc.change(text, 0, null), text, 'not a note: unchanged')
})

test('editing a note changes its text and kind; its time and quote stay', () => {
  const text = '- [16:20] ! figure out GPT plugins\n  > "GPT plugins are out of this phase."\n'
  assert.equal(NoteDoc.change(text, 0, { text: 'When are GPT plugins GA?', kind: 'question' }),
    '- [16:20] ? When are GPT plugins GA?\n  > "GPT plugins are out of this phase."\n')
  assert.equal(NoteDoc.change(text, 0, { kind: 'todo' }), '- [ ] [16:20] figure out GPT plugins\n  > "GPT plugins are out of this phase."\n')
})

test('a to-do at a time reads however its box and time are written', () => {
  for (const l of ['- [ ] [16:20] check GA', '- [] [16:20] check GA', '- [16:20] [] check GA', '- [16:20] [ ] check GA', '- [][16:20] check GA', '- [16:20][]check GA']) {
    assert.deepEqual(notes(l + '\n'), [{ kind: 'todo', t: 980, done: false, text: 'check GA', quote: null }], l)
  }
  assert.equal(notes('- [16:20] [x] done\n')[0].done, true)
  assert.deepEqual(notes('- [] general to-do\n')[0], { kind: 'todo', t: null, done: false, text: 'general to-do', quote: null })
})

test('a time typed in the note box sets the moment, before or after the kind', () => {
  assert.deepEqual(NoteDoc.typed('[16:20] [] review', 'note'), { kind: 'todo', text: 'review', t: 980 })
  assert.deepEqual(NoteDoc.typed('[] [16:20] review', 'note'), { kind: 'todo', text: 'review', t: 980 })
  assert.deepEqual(NoteDoc.typed('[1:02:03] ? why', 'note'), { kind: 'question', text: 'why', t: 3723 })
  assert.equal(NoteDoc.typed('[] review', 'note').t, undefined)
})

test('what is typed picks its kind', () => {
  assert.deepEqual(NoteDoc.typed('? legal OK', 'note'), { kind: 'question', text: 'legal OK' })
  assert.deepEqual(NoteDoc.typed('! go', 'note'), { kind: 'decision', text: 'go' })
  assert.deepEqual(NoteDoc.typed('[] send', 'note'), { kind: 'todo', text: 'send' })
  assert.deepEqual(NoteDoc.typed('[ ] send', 'question'), { kind: 'todo', text: 'send' })
  assert.deepEqual(NoteDoc.typed('plain', 'decision'), { kind: 'decision', text: 'plain' })
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`my notes: ${tests.length} passed`)
