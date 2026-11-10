// When an open call may write its own address (app.js, callUrl). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/call-url.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── call url: ')
const end = src.indexOf('// end of the call url block', start)
assert.ok(start >= 0 && end > start, 'the call url block not found in app.js')
const { callUrl } = new Function(`${src.slice(start, end)}\nreturn { callUrl }`)()

const A = '2026-11-10_090000-acme-sync'
const own = (tab = 'minutes', name = A) => `#call/${name}/${tab}`
const tests = []
const test = (name, f) => tests.push([name, f])

test('the address the window routed is corrected to the open call\'s own', () => {
  // A tab the window chose itself (#148): routed on Minutes, the call opens on Transcript while it is transcribed.
  assert.equal(callUrl(own('minutes'), own('minutes'), own('transcript')), own('transcript'))
  // A fresh load on the list, which opens the newest call.
  assert.equal(callUrl('#calls', '#calls', own()), own())
  // A call renamed under its open address (its minutes retitled the folder).
  assert.equal(callUrl(own('minutes', '2026-11-10_090000-old-title'), own('minutes', '2026-11-10_090000-old-title'), own()), own())
})

test('an address already the call\'s own is left as it is', () => {
  assert.equal(callUrl(own(), own(), own()), null)
})

// A click sets the address first and the view follows on hashchange: a redraw in between still shows this call, and
// writing its address back would lose the click (#355) — whether it goes to another call, another tab or another view.
test('an address changed since it was routed is a click on its way: left alone (#355)', () => {
  for (const clicked of [own('minutes', '2026-11-10_120000-memo'), own('transcript'), '#today', '#project/acme-rollout/work']) {
    assert.equal(callUrl(clicked, own('minutes'), own('minutes')), null, clicked)
  }
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`call url: ${tests.length} passed`)
