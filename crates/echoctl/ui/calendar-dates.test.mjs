// The calendar's helpers (app.js): days, hours and side-by-side lanes by the local calendar and
// clock, across the days the clocks change. Run by scripts/check-mod.sh (`node crates/echoctl/ui/calendar-dates.test.mjs`):
// the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

process.env.TZ = 'Europe/Berlin' // clocks go back on 25 Oct 2026, forward on 29 Mar 2026
// A Node without time zone data silently uses UTC, where most of these would pass on the old code.
assert.equal(new Date(2026, 9, 26) - new Date(2026, 9, 25), 25 * 3600000, 'Europe/Berlin not in effect')

const src = appSource()
const start = src.indexOf('const ymd = (d) =>')
const end = src.indexOf('// end of the calendar\'s helpers', start)
assert.ok(start >= 0 && end > start, "the calendar's helpers not found in app.js")
const { addDays, clockHours, layoutDay, ymd } = new Function(`${src.slice(start, end)}\nreturn { addDays, clockHours, layoutDay, ymd }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const week = (from) => Array.from({ length: 7 }, (_, i) => ymd(addDays(from, i)))

test('a week across the clocks going back has seven different days', () => {
  assert.deepEqual(week(new Date(2026, 9, 24)), ['2026-10-24', '2026-10-25', '2026-10-26', '2026-10-27', '2026-10-28', '2026-10-29', '2026-10-30'])
})

test('a week across the clocks going forward has seven different days', () => {
  assert.deepEqual(week(new Date(2026, 2, 28)), ['2026-03-28', '2026-03-29', '2026-03-30', '2026-03-31', '2026-04-01', '2026-04-02', '2026-04-03'])
})

test('each day starts at midnight, also after either change', () => {
  for (const from of [new Date(2026, 9, 24), new Date(2026, 2, 28)]) {
    for (let i = 0; i < 7; i++) {
      const d = addDays(from, i)
      assert.equal(d.getHours() * 60 + d.getMinutes(), 0, ymd(d))
    }
  }
})

test('an event sits at its clock time, on the change day and after it', () => {
  const mon = addDays(new Date(2026, 9, 24), 2)
  assert.equal(clockHours(new Date(2026, 9, 26, 17, 45).getTime(), mon), 17.75)
  const sun = new Date(2026, 9, 25)
  assert.equal(clockHours(new Date(2026, 9, 25, 10, 0).getTime(), sun), 10)
  assert.equal(clockHours(new Date(2026, 9, 25, 1, 0).getTime(), sun), 1)
  assert.equal(clockHours(new Date(2026, 2, 29, 10, 0).getTime(), new Date(2026, 2, 29)), 10)
  assert.equal(clockHours(new Date(2026, 2, 30, 9, 0).getTime(), new Date(2026, 2, 30)), 9)
})

test('a time before the day is 0, after it 24', () => {
  const sun = new Date(2026, 9, 25)
  assert.equal(clockHours(new Date(2026, 9, 24, 23, 0).getTime(), sun), 0)
  assert.equal(clockHours(new Date(2026, 9, 26, 0, 30).getTime(), sun), 24)
})

test('two meetings in the hour that repeats are side by side, not one on the other', () => {
  // 02:10–02:40 summer time, then 02:10–02:40 winter time: an hour apart, drawn at the same hours
  const at = (h, m) => Date.UTC(2026, 9, 25, h, m)
  const [a, b] = layoutDay([{ s: at(0, 10), real: at(0, 40) }, { s: at(1, 10), real: at(1, 40) }], new Date(2026, 9, 25))
  assert.equal(a.y0, b.y0)
  assert.deepEqual([a.lane, b.lane, a.lanes, b.lanes], [0, 1, 2, 2])
})

test('meetings one after another get the whole column; a short one is drawn 20 min long', () => {
  const mon = new Date(2026, 9, 26)
  const at = (h, m = 0) => new Date(2026, 9, 26, h, m).getTime()
  const [a, b, c] = layoutDay([{ s: at(9), real: at(10) }, { s: at(10), real: at(11) }, { s: at(12), real: at(12, 5) }], mon)
  assert.deepEqual([a.y0, a.y1, b.y0, b.y1, a.lanes, b.lanes], [9, 10, 10, 11, 1, 1])
  assert.equal(c.y1, 12 + 1 / 3)
})

test('a short meeting and the next one at its 20-min end stay in one lane, at any minute', () => {
  const mon = new Date(2026, 9, 26)
  for (let m = 0; m + 30 <= 24 * 60; m++) {
    const at = (min) => new Date(2026, 9, 26, 0, min).getTime()
    const [a, b] = layoutDay([{ s: at(m), real: at(m + 10) }, { s: at(m + 20), real: at(m + 30) }], mon)
    assert.deepEqual([a.lanes, b.lanes], [1, 1], `at ${Math.floor(m / 60)}:${String(m % 60).padStart(2, '0')}`)
  }
})

let failed = 0
for (const [name, f] of tests) {
  try {
    f()
    console.log(`ok   ${name}`)
  } catch (e) {
    failed++
    console.log(`FAIL ${name}\n     ${e.message.split('\n').join('\n     ')}`)
  }
}
console.log(`calendar dates: ${tests.length - failed} pass, ${failed} fail`)
if (failed) process.exit(1)
