// My notes' timeline with live notes (app.js, LiveTimeline). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/live-timeline.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── live timeline: the user\'s notes and the live notes in one list')
const end = src.indexOf('// end of the live timeline block', start)
assert.ok(start >= 0 && end > start, 'the live timeline block not found in app.js')
const { LiveTimeline } = new Function(`${src.slice(start, end)}\nreturn { LiveTimeline }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
// The merged list as short labels: a user's entry by its index, a live item by its text.
const order = (mine, live) => LiveTimeline.merge(mine, live).map((e) => (e.live ? e.live.text : `#${e.mine.i}`))

test('items: decisions as decision, action items as todo with their owner, questions as question', () => {
  const n = { now: 'x', decisions: [{ text: 'Go', t: 250 }], actions: [{ owner: 'Rita Gomez', task: 'Dry run', t: 45 }], questions: [{ text: 'Who?', t: 151 }] }
  assert.deepEqual(LiveTimeline.items(n), [
    { kind: 'decision', text: 'Go', t: 250 },
    { kind: 'todo', text: 'Dry run', owner: 'Rita Gomez', t: 45 },
    { kind: 'question', text: 'Who?', t: 151 },
  ])
})

test('items: no draft, or a draft with only Now, has none; a time that is not a number is none', () => {
  assert.deepEqual(LiveTimeline.items(null), [])
  assert.deepEqual(LiveTimeline.items({ now: 'only now' }), [])
  assert.equal(LiveTimeline.items({ decisions: [{ text: 'a', t: '12' }] })[0].t, null)
  assert.equal(LiveTimeline.items({ decisions: [{ text: 'a', t: NaN }] })[0].t, null)
})

test('merge: live items go between the user\'s by time', () => {
  const mine = [{ i: 0, t: 12 }, { i: 1, t: 31 }, { i: 2, t: 90 }]
  const live = [{ text: 'L250', t: 250 }, { text: 'L45', t: 45 }, { text: 'L5', t: 5 }]
  assert.deepEqual(order(mine, live), ['L5', '#0', '#1', 'L45', '#2', 'L250'])
})

test('merge: the same second puts the user\'s note first', () => {
  assert.deepEqual(order([{ i: 0, t: 45 }], [{ text: 'L45', t: 45 }]), ['#0', 'L45'])
})

test('merge: the user\'s entries never move, even out of time order or with no time', () => {
  // a heading (no time), a note at 1:30, one at 0:10 written after it, an untimed note
  const mine = [{ i: 0, t: null }, { i: 1, t: 90 }, { i: 2, t: 10 }, { i: 3, t: null }]
  const got = order(mine, [{ text: 'L60', t: 60 }, { text: 'L200', t: 200 }])
  assert.deepEqual(got.filter((x) => x.startsWith('#')), ['#0', '#1', '#2', '#3'])
  assert.deepEqual(got, ['#0', 'L60', '#1', '#2', '#3', 'L200'])
})

test('merge: live items with no time come last, in the draft\'s order', () => {
  const got = order([{ i: 0, t: 30 }], [{ text: 'A', t: null }, { text: 'L10', t: 10 }, { text: 'B', t: null }])
  assert.deepEqual(got, ['L10', '#0', 'A', 'B'])
})

test('merge: live items of the same time keep the draft\'s order; nothing of the user\'s: the live list alone', () => {
  assert.deepEqual(order([], [{ text: 'b', t: 20 }, { text: 'a', t: 20 }, { text: 'c', t: 5 }]), ['c', 'b', 'a'])
  assert.deepEqual(order([{ i: 0, t: 1 }, { i: 1, t: null }], []), ['#0', '#1'])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`live timeline: ${tests.length} passed`)
