// Whether a note keeps its call time (app.js, recordsCall and noteTime). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/note-time.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── note time: ')
const end = src.indexOf('// end of the note time block', start)
assert.ok(start >= 0 && end > start, 'the note time block not found in app.js')
const { recordsCall, noteTime, askRecording, within } = new Function(`${src.slice(start, end)}\nreturn { recordsCall, noteTime, askRecording, within }`)()

const CALL = '2026-11-10_090000-acme-sync'
const tests = []
const test = (name, f) => tests.push([name, f])

test('the app says whether it records the call: by its stamp (a retitle keeps it), on either kind of path', () => {
  assert.equal(recordsCall({ recording: `/Users/x/Echo/calls/${CALL}` }, CALL), true)
  assert.equal(recordsCall({ recording: 'C:\\Users\\x\\Echo\\calls\\2026-11-10_090000-weekly-sync' }, CALL), true, 'retitled, Windows path')
  assert.equal(recordsCall({ recording: '/Users/x/Echo/calls/2026-11-10_120000-memo' }, CALL), false, 'another call records')
  assert.equal(recordsCall({ recording: null }, CALL), false, 'nothing records')
  assert.equal(recordsCall(null, CALL), false, 'no state')
})

test('a note started while the call records keeps its time, exactly', () => {
  assert.equal(noteTime(754.4, true), 754.4)
})

// Typed in the seconds after hang-up, the window may still show the call recording: the app's answer decides (#358).
test('a note the app says came after the call (or with no answer in time) gets no call time (#358)', () => {
  assert.equal(noteTime(754.4, false), null, 'just ended')
  assert.equal(noteTime(754.4, undefined), null, 'unsure: no answer within the wait')
  assert.equal(noteTime(null, true), null, 'no time to keep')
})

// The app is asked by the state the window polls, read again; Add waits for the answer at most 500 ms.
test("the app's answer: true while it records the call, false after, unsure when the ask fails", async () => {
  const get = (state) => async () => state
  assert.equal((await askRecording(get({ recording: `/calls/${CALL}` }), CALL)).ok, true)
  assert.equal((await askRecording(get({ recording: null }), CALL)).ok, false)
  assert.equal((await askRecording(async () => { throw new Error('app restarting') }, CALL)).ok, undefined)
})

test('an answer later than the wait is no answer: unsure, so no call time', async () => {
  let fire
  const timer = (f) => { fire = f }
  const late = new Promise(() => {}) // never in time
  const waited = within(late, 500, timer)
  fire()
  assert.equal(await waited, undefined)
  assert.equal(noteTime(754.4, await waited), null)
  // One in time is taken.
  assert.equal(await within(Promise.resolve(true), 500, () => {}), true)
})

let failed = 0
for (const [name, f] of tests) {
  try { await f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`note time: ${tests.length} passed`)
