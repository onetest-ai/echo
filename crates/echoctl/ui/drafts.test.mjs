// Where mail drafts go (#135): the setting's line (draftsNow) and a run's draft as its outcome says it (draftLine).
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/drafts.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── drafts:', to = '// end of the drafts block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'drafts block not found in app.js')
const { draftsNow, draftLine } = new Function(`${src.slice(start, end)}\nreturn { draftsNow, draftLine }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('the setting says where drafts go now, and when that is the default', () => {
  assert.match(draftsNow({ drafts: null, drafts_now: 'm365' }), /Now: Microsoft 365 Drafts \(the default when Microsoft 365 is connected\)\.$/)
  assert.match(draftsNow({ drafts: null, drafts_now: 'mail-app' }), /Now: the default mail app \(the default\)\.$/)
  assert.match(draftsNow({ drafts: 'mail-app', drafts_now: 'mail-app' }), /Now: the default mail app\.$/)
  assert.match(draftsNow({ drafts: 'app:Microsoft Outlook', drafts_now: 'app' }), /Now: Microsoft Outlook\.$/)
  assert.match(draftsNow({ drafts: null, drafts_now: 'm365' }), /you send it/)
})

test("a run's draft: where it is, to whom, about what", () => {
  assert.equal(draftLine({ where: 'm365', url: 'https://outlook.office.com/x', to: ['kim@acme.example'], subject: 'Training slots?' }), 'A draft in Microsoft 365 Drafts — “Training slots?” to kim@acme.example')
  assert.equal(draftLine({ where: 'mail-app', to: ['kim@acme.example'], subject: 'Hi' }), 'A draft opened in your mail app — “Hi” to kim@acme.example')
  assert.equal(draftLine({ where: 'app', app: 'Spark', to: [], subject: '' }), 'A draft opened in Spark')
})

test('a draft that could not go to Microsoft 365 says why', () => {
  assert.match(draftLine({ where: 'mail-app', note: 'not in Microsoft 365 Drafts (not signed in): opened in the mail app instead', to: ['a@b.example'] }), /\(not in Microsoft 365 Drafts/)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
