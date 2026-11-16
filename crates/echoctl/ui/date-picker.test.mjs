// Echo's calendar for a date box (app.js, the date picker block): a month as weeks from Monday, and the quick days.
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/date-picker.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── date picker\n')
const end = src.indexOf('// end of the date picker block', start)
assert.ok(start >= 0 && end > start, 'the date picker block not found in app.js')
const { dpMonth, dpQuick } = new Function(`${src.slice(start, end)}\nreturn { dpMonth, dpQuick }`)()

// November 2026 starts on a Sunday: its first week starts Monday, October 26; five weeks... and the 30th on a Monday.
const nov = dpMonth(2026, 10)
assert.equal(nov[0][0].iso, '2026-10-26')
assert.equal(nov[0][6].iso, '2026-11-01')
assert.equal(nov[0][6].inMonth, true)
assert.equal(nov[0][0].inMonth, false)
assert.equal(nov.at(-1)[0].iso, '2026-11-30')
assert.equal(nov.length, 6, 'Oct 26 … Dec 6: six weeks')
assert.ok(nov.every((w) => w.length === 7))
// A month starting on a Monday starts on its 1st; February 2027 (Monday 1st, 28 days) is four weeks.
const feb = dpMonth(2027, 1)
assert.equal(feb[0][0].iso, '2027-02-01')
assert.equal(feb.length, 4)
// Across the clocks changing (Europe, last Sunday of March/October): every day once, in order.
const days = dpMonth(2027, 2).flat().map((d) => d.iso)
assert.equal(new Set(days).size, days.length)
assert.deepEqual([...days].sort(), days)

// Quick days. Thursday Nov 12, 2026: Today, Tomorrow (Fri 13 — "This Friday" is the same day: named once), Next
// Monday (16), In a week (19).
assert.deepEqual(dpQuick('2026-11-12'), [
  { label: 'Today', iso: '2026-11-12' }, { label: 'Tomorrow', iso: '2026-11-13' },
  { label: 'Next Monday', iso: '2026-11-16' }, { label: 'In a week', iso: '2026-11-19' },
])
// Monday Nov 9: This Friday is the 13th.
assert.deepEqual(dpQuick('2026-11-09').map((q) => q.label + ' ' + q.iso), ['Today 2026-11-09', 'Tomorrow 2026-11-10', 'This Friday 2026-11-13', 'Next Monday 2026-11-16', 'In a week 2026-11-16'].filter((x, i) => i !== 4))
// Saturday Nov 14: Next Friday is the 20th; Next Monday the 16th.
assert.deepEqual(dpQuick('2026-11-14').map((q) => q.label), ['Today', 'Tomorrow', 'Next Friday', 'Next Monday', 'In a week'])
assert.equal(dpQuick('2026-11-14')[2].iso, '2026-11-20')
console.log('date-picker: ok')
