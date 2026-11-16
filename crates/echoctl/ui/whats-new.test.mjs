// What's new in the window (`whats-new` block): its heading, where Show me goes, and when it opens by itself (once per
// version the app has for it). Run by scripts/check-mod.sh (`node crates/echoctl/ui/whats-new.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── whats-new\n', to = '// end of the whats-new block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'whats-new block not found in app.js')
const { WhatsNew } = new Function(`${src.slice(start, end)}\nreturn { WhatsNew }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const e = (version) => ({ version, items: [{ icon: 'filter', title: 'Filters', text: 'Set it once.' }] })

test('one version names the heading; several are each under a header of their own', () => {
  assert.equal(WhatsNew.heading([e('0.9.8')]), "What's new in Echo 0.9.8")
  assert.equal(WhatsNew.heading([e('0.9.8'), e('0.9.7')]), "What's new in Echo")
})

test('Show me goes to a window route, or opens an echo:// link; an item without a place goes nowhere', () => {
  assert.deepEqual(WhatsNew.target('settings/plugins'), { route: 'settings/plugins' })
  assert.deepEqual(WhatsNew.target('echo://thread/acme-rollout'), { link: 'echo://thread/acme-rollout' })
  assert.equal(WhatsNew.target(undefined), null)
})

test('it opens by itself once: when the app has entries this page has not shown', () => {
  const p = { version: '0.9.8', entries: [e('0.9.8'), e('0.9.7')] }
  assert.equal(WhatsNew.due(p, undefined), true, 'after an update')
  assert.equal(WhatsNew.due(p, '0.9.8'), false, 'shown on this page: not again before the app has it seen')
  assert.equal(WhatsNew.due(null, undefined), false, 'nothing new')
  assert.equal(WhatsNew.due({ version: '0.9.8', entries: [] }, undefined), false)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (err) { failed++; console.log(`not ok - ${name}\n  ${err.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
