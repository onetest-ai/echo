// The optimistic mark on a calendar card after Record / Don't record (app.js, markScheduled): the card reads as the
// server's answer would, language included. Run by scripts/check-mod.sh (`node crates/echoctl/ui/schedule-mark.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── schedule mark: ')
const end = src.indexOf('// end of the schedule mark block', start)
assert.ok(start >= 0 && end > start, 'the schedule mark block not found in app.js')
const { markScheduled } = new Function(`${src.slice(start, end)}\nreturn { markScheduled }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('scheduling carries the language, as the calendar answer does', () => {
  const e = { id: 'evt-1', scheduled: false }
  markScheduled(e, true, 'en')
  assert.deepEqual(e, { id: 'evt-1', scheduled: true, scheduled_languages: ['en'] })
})

test('not recording drops the language again', () => {
  const e = { id: 'evt-1', scheduled: true, scheduled_languages: ['uk'] }
  markScheduled(e, false)
  assert.deepEqual(e, { id: 'evt-1', scheduled: false })
})

test("Join and record for me marks the card so, and Don't record clears it", () => {
  const e = { id: 'evt-1', scheduled: false }
  markScheduled(e, true, 'en', true)
  assert.deepEqual(e, { id: 'evt-1', scheduled: true, scheduled_languages: ['en'], scheduled_for_me: true })
  markScheduled(e, false)
  assert.deepEqual(e, { id: 'evt-1', scheduled: false })
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`schedule mark: ${tests.length} passed`)
