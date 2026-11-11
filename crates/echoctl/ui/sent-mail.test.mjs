// A mail sent from an item (sentMailLine, #92): its row on the item says when it went, to whom, and whether it was
// answered. Run by scripts/check-mod.sh (`node crates/echoctl/ui/sent-mail.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── sent mail', to = '// end of the sent-mail block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'sent-mail block not found in app.js')
const { sentMailLine } = new Function(`${src.slice(start, end)}\nreturn { sentMailLine }`)()
const day = (t) => t.slice(0, 10)

const tests = []
const test = (name, f) => tests.push([name, f])

test('not answered yet', () => {
  assert.equal(sentMailLine({ at: '2026-11-08T10:00:00+02:00', to: ['dana@acme.example'], reply_at: null }, day), 'Mail sent 2026-11-08 to dana@acme.example, no reply yet')
})

test('answered', () => {
  assert.equal(sentMailLine({ at: '2026-11-08T10:00:00+02:00', to: ['dana@acme.example', 'omar@acme.example'], reply_at: '2026-11-09T08:00:00+02:00' }, day), 'Mail sent 2026-11-08 to dana@acme.example, omar@acme.example · replied 2026-11-09')
})

test('no recipients recorded', () => {
  assert.equal(sentMailLine({ at: '2026-11-08T10:00:00+02:00' }, day), 'Mail sent 2026-11-08, no reply yet')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
