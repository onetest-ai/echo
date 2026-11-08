// NotesSync (app.js): how the Echo window saves the user's notes. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/notes-sync.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('const NotesSync = (() => {')
const end = src.indexOf('\n})()\n', start)
assert.ok(start >= 0 && end > start, 'NotesSync not found in app.js')
const make = () => {
  const NotesSync = new Function('send', `${src.slice(start, end + 5)}\nreturn NotesSync`)(() => {})
  const sent = []
  NotesSync.setPost((m) => sent.push(m))
  return { N: NotesSync, sent }
}
const CALL = '2026-10-23_090000-standup'
const tests = []
const test = (name, f) => tests.push([name, f])

test('saves what was typed, with what the file held as its base', () => {
  const { N, sent } = make()
  N.seen(CALL, 'a\n')
  N.edit(CALL, 'a\nb\n')
  N.save(CALL)
  assert.deepEqual(sent, [{ cmd: 'save_my_notes', call: CALL, id: 1, text: 'a\nb\n', base: 'a\n' }])
  N.saved({ call: CALL, id: 1, stored: 'a\nb\n' })
  assert.deepEqual(N.view(CALL), { draft: 'a\nb\n', saving: false, dirty: false, error: null, kept: null })
})

test('an unchanged text is never sent (it would move the minutes clock)', () => {
  const { N, sent } = make()
  N.seen(CALL, 'same')
  N.edit(CALL, 'same')
  N.save(CALL)
  assert.equal(sent.length, 0)
})

test('typing while a save is in flight: saved next, on top of what was stored', () => {
  const { N, sent } = make()
  N.seen(CALL, '')
  N.edit(CALL, 'one')
  N.save(CALL)
  N.edit(CALL, 'one two')
  N.save(CALL) // one in flight per call: nothing sent yet
  assert.equal(sent.length, 1)
  N.saved({ call: CALL, id: 1, stored: 'one' })
  assert.deepEqual(sent[1], { cmd: 'save_my_notes', call: CALL, id: 2, text: 'one two', base: 'one' })
  assert.equal(N.view(CALL).draft, 'one two')
})

test('a tray note appended meanwhile lands at the end of the draft', () => {
  const { N, sent } = make()
  N.seen(CALL, '')
  N.edit(CALL, 'mine')
  N.save(CALL)
  N.saved({ call: CALL, id: 1, stored: 'mine- [03:10] tray\n' })
  assert.equal(N.view(CALL).draft, 'mine- [03:10] tray\n')
  // …also when typing went on during that save.
  N.edit(CALL, 'mine- [03:10] tray\nmore')
  N.save(CALL)
  N.edit(CALL, 'mine- [03:10] tray\nmore and more')
  N.saved({ call: CALL, id: 2, stored: 'mine- [03:10] tray\nmore- [04:00] again\n' })
  assert.equal(N.view(CALL).draft, 'mine- [03:10] tray\nmore and more- [04:00] again\n')
  assert.equal(sent.at(-1).base, 'mine- [03:10] tray\nmore- [04:00] again\n')
})

test('the file changing while there are unsaved edits keeps the draft and its base', () => {
  const { N, sent } = make()
  N.seen(CALL, 'a')
  N.edit(CALL, 'a mine')
  N.seen(CALL, 'a- tray\n') // the 2 s refresh brings the tray's note
  assert.equal(N.view(CALL).draft, 'a mine')
  N.save(CALL)
  assert.equal(sent[0].base, 'a', 'the app keeps what was appended after this base')
  // With nothing unsaved, a refresh is taken as it is.
  N.saved({ call: CALL, id: 1, stored: 'a mine- tray\n' })
  N.seen(CALL, 'a mine- tray\n- later\n')
  assert.equal(N.view(CALL).draft, 'a mine- tray\n- later\n')
})

test('a failed save keeps the draft, says why, and stops what waited for it', async () => {
  const { N } = make()
  N.seen(CALL, '')
  N.edit(CALL, 'x')
  const flushed = N.flush(CALL)
  N.saved({ call: CALL, id: 1, error: 'disk full' })
  await assert.rejects(flushed, /disk full/)
  assert.deepEqual(N.view(CALL), { draft: 'x', saving: false, dirty: true, error: 'disk full', kept: null })
})

test('flush resolves only once the file holds the draft (a delayed answer, a second round)', async () => {
  const { N, sent } = make()
  N.seen(CALL, '')
  N.edit(CALL, 'one')
  N.save(CALL)
  N.edit(CALL, 'one two') // typed after the first save went out
  let done = false
  const flushed = N.flush(CALL).then(() => { done = true })
  await Promise.resolve()
  assert.equal(done, false)
  N.saved({ call: CALL, id: 1, stored: 'one' })
  await Promise.resolve()
  assert.equal(done, false, 'the newer text is not saved yet')
  assert.equal(sent[1].text, 'one two')
  N.saved({ call: CALL, id: 2, stored: 'one two' })
  await flushed
  assert.equal(done, true)
  await N.flush(CALL) // nothing left: at once
})

test('closing waits for every call, and a failing one stops it', async () => {
  const { N } = make()
  const OTHER = '2026-10-23_110000-review'
  N.seen(CALL, ''); N.edit(CALL, 'a')
  N.seen(OTHER, ''); N.edit(OTHER, 'b')
  const all = N.flushAll()
  N.saved({ call: CALL, id: 1, stored: 'a' })
  N.saved({ call: OTHER, id: 2, error: 'no such call' })
  await assert.rejects(all, /no such call/)
  assert.equal(N.view(CALL).dirty, false)
  assert.equal(N.view(OTHER).draft, 'b')
})

test('a retitled call keeps its draft (by stamp), and an old answer is ignored', () => {
  const { N, sent } = make()
  N.seen(CALL, '')
  N.edit(CALL, 'x')
  N.save(CALL)
  N.saved({ call: CALL, id: 99, stored: 'nope' })
  assert.equal(N.view(CALL).saving, true)
  const renamed = '2026-10-23_090000-weekly-sync'
  N.saved({ call: renamed, id: 1, stored: 'x' })
  assert.equal(N.view(renamed).dirty, false)
  N.edit(renamed, 'xy')
  N.save(renamed)
  assert.equal(sent.at(-1).call, renamed, 'saved under the folder as it is named now')
})

test('a copy kept of an outside edit is reported once', () => {
  const { N } = make()
  N.seen(CALL, 'old')
  N.edit(CALL, 'new')
  N.save(CALL)
  N.saved({ call: CALL, id: 1, stored: 'new', kept: 'my-notes.101500.md' })
  assert.equal(N.view(CALL).kept, 'my-notes.101500.md')
  N.clearKept(CALL)
  assert.equal(N.view(CALL).kept, null)
})

test('a fetch that began before a save was answered never rolls the editor back', () => {
  const { N } = make()
  N.seen(CALL, 'a')
  N.edit(CALL, 'a b')
  N.save(CALL)
  const before = N.epoch(CALL) // a poll starts while the save is in flight…
  N.saved({ call: CALL, id: 1, stored: 'a b' })
  N.seen(CALL, 'a', before) // …and answers with the file from before it
  assert.deepEqual([N.view(CALL).draft, N.view(CALL).dirty], ['a b', false])
  N.seen(CALL, 'a b- tray\n', N.epoch(CALL)) // a fetch begun after the answer is taken
  assert.equal(N.view(CALL).draft, 'a b- tray\n')
})

test('an outside change while a note is being typed waits: the note goes on the notes shown, the other version is kept beside (#130)', () => {
  const { N, sent } = make()
  N.seen(CALL, '- [00:20] Kim\n')
  // The box holds a note (`typing`): the 2 s re-read sees another editor's version and leaves the notes as shown.
  N.seen(CALL, '- Rewritten\n', N.epoch(CALL), true)
  assert.equal(N.view(CALL).draft, '- [00:20] Kim\n')
  // Enter adds the note to them; the save's base is what was shown, so the app keeps the other version beside.
  N.edit(CALL, '- [00:20] Kim\n- Ask Kim\n')
  N.save(CALL)
  assert.deepEqual(sent[0], { cmd: 'save_my_notes', call: CALL, id: 1, text: '- [00:20] Kim\n- Ask Kim\n', base: '- [00:20] Kim\n' })
  N.saved({ call: CALL, id: 1, stored: '- [00:20] Kim\n- Ask Kim\n', kept: 'my-notes.101500.md' })
  assert.equal(N.view(CALL).kept, 'my-notes.101500.md')
  // With nothing typed, the file's version is taken again.
  N.seen(CALL, '- Rewritten again\n', N.epoch(CALL))
  assert.equal(N.view(CALL).draft, '- Rewritten again\n')
})

let failed = 0
for (const [name, f] of tests) {
  try { await f(); console.log(`ok   ${name}`) } catch (e) { failed++; console.log(`FAIL ${name}\n     ${e.message}`) }
}
console.log(`NotesSync: ${tests.length - failed} pass, ${failed} fail`)
process.exit(failed ? 1 : 0)
