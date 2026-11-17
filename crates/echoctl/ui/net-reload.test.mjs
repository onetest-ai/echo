// When the window loads again for its policy (netReload): the addresses the user listed for plugins changed, the page
// isn't being typed in, and it hasn't tried that key already. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/net-reload.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── net reload', to = '// end of the net-reload block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'net-reload block not found in app.js')
const { netReload, typingIn } = new Function(`${src.slice(start, end)}\nreturn { netReload, typingIn }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('the same key: no reload', () => {
  assert.equal(netReload('a1', 'a1', false, null), false)
})

test('another key: the page loads again, under the new policy', () => {
  assert.equal(netReload('a1', 'b2', false, null), true)
  assert.equal(netReload('a1', 'b2', false, 'a1'), true, 'an earlier key tried: this one not yet')
})

test('not while the user types: the next tick tries again', () => {
  assert.equal(netReload('a1', 'b2', true, null), false)
})

test('typing: any text box or area whose value changed, an editable region typed in', () => {
  const box = (tagName, value, defaultValue, type) => ({ tagName, value, defaultValue, type })
  assert.equal(typingIn(null), false, 'nothing focused')
  assert.equal(typingIn(box('INPUT', 'a', 'a', 'text')), false, 'focused, nothing typed')
  assert.equal(typingIn(box('INPUT', 'ab', 'a', 'text')), true, 'a plugin setting, the search, the ask box: any text box')
  assert.equal(typingIn(box('INPUT', 'x', '', undefined)), true, 'no type: a text box')
  assert.equal(typingIn(box('INPUT', 'p', '', 'password')), true)
  assert.equal(typingIn(box('TEXTAREA', 'my notes, more', 'my notes')), true, 'My notes, the profile, a plugin text area')
  assert.equal(typingIn(box('TEXTAREA', 'same', 'same')), false)
  for (const type of ['checkbox', 'radio', 'range', 'button', 'file']) assert.equal(typingIn(box('INPUT', 'on', 'off', type)), false, type)
  assert.equal(typingIn(box('SELECT', 'b', 'a')), false, 'a choice made is saved, not typed')
  const ed = { tagName: 'DIV', isContentEditable: true }
  assert.equal(typingIn(ed, new WeakSet()), false, 'an editable region only focused')
  assert.equal(typingIn(ed, new WeakSet([ed])), true, 'typed in')
  assert.equal(netReload('a1', 'b2', typingIn(box('INPUT', 'ab', 'a', 'text')), null), false, 'held while typing')
})

test('once per key: a reload that did not take never loops', () => {
  assert.equal(netReload('a1', 'b2', false, 'b2'), false)
})

test('nothing known (no meta, a state read that failed): no reload', () => {
  assert.equal(netReload('', 'b2', false, null), false)
  assert.equal(netReload(null, 'b2', false, null), false)
  assert.equal(netReload('a1', undefined, false, null), false)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
