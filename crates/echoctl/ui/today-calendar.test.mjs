// Your day as a calendar and a strip (app.js, dayLanes / dayWindow): overlapping meetings side by side in lanes, the
// hours shown. Run by scripts/check-mod.sh (`node crates/echoctl/ui/today-calendar.test.mjs`): the block is taken from
// app.js as it ships. Design: Workbench `WidgetsCalendar`.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── day lanes:')
const end = src.indexOf('// end of the day lanes block', start)
assert.ok(start >= 0 && end > start, 'the day lanes block not found in app.js')
const { dayLanes, dayWindow } = new Function(`${src.slice(start, end)}\nreturn { dayLanes, dayWindow }`)()

const at = (h, m = 0) => new Date(2026, 10, 12, h, m).toISOString()
const mt = (subject, h1, m1, h2, m2) => ({ subject, start: at(h1, m1), end: at(h2, m2) })
const lanes = (xs) => dayLanes(xs).map((x) => `${x.item.subject}:${x.lane}/${x.lanes}`).join(' ')
const tests = []
const test = (name, f) => tests.push([name, f])

test('meetings that do not overlap each take the whole width', () => {
  assert.equal(lanes([mt('A', 9, 0, 10, 0), mt('B', 10, 0, 11, 0)]), 'A:0/1 B:0/1')
})
test('two at once side by side; the next free lane is reused', () => {
  assert.equal(lanes([mt('A', 11, 0, 12, 0), mt('B', 11, 0, 11, 30), mt('C', 11, 30, 12, 0)]), 'A:0/2 B:1/2 C:1/2')
})
test('a cluster is as wide as its most at once; a meeting after it starts over', () => {
  assert.equal(lanes([mt('A', 14, 0, 16, 0), mt('B', 14, 0, 15, 0), mt('C', 14, 30, 15, 30), mt('D', 17, 0, 18, 0)]), 'A:0/3 B:1/3 C:2/3 D:0/1')
})
test('the hours shown: 9 to 18 at least, wider for an early or late meeting, whole hours', () => {
  assert.deepEqual(dayWindow([mt('A', 10, 0, 11, 0)]), [9, 18])
  assert.deepEqual(dayWindow([mt('A', 7, 30, 8, 0), mt('B', 18, 15, 19, 10)]), [7, 20])
  assert.deepEqual(dayWindow([]), [9, 18])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
