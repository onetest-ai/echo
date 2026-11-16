// Which handler a click on the window runs (app.js): the element it is for and the handler chosen for it
// (`clickFor`, over the CLICKS registry). Run by scripts/check-mod.sh (`node crates/echoctl/ui/clicks.test.mjs`):
// the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── clicks: which handler a click on the page runs')
const end = src.indexOf('// end of the clicks block', start)
assert.ok(start >= 0 && end > start, 'the clicks block not found in app.js')
const { CLICKS, clickFor } = new Function(`${src.slice(start, end)}\nreturn { CLICKS, clickFor }`)()

// A page element as far as the lookup sees it: its tag, its data-* (as `dataset`), its parent; `closest` over the
// selectors the registry uses (`[data-x]`, `a[data-x]`, joined by commas).
const camel = (s) => s.replace(/-([a-z])/g, (_, c) => c.toUpperCase())
const el = (tag, dataset, parent = null) => ({
  tagName: tag.toUpperCase(), dataset, parent,
  matches(sel) {
    return sel.split(',').some((s) => {
      const m = /^([a-z]*)\[data-([a-z-]+)\]$/.exec(s)
      return (!m[1] || m[1].toUpperCase() === this.tagName) && camel(m[2]) in this.dataset
    })
  },
  closest(sel) { for (let x = this; x; x = x.parent) if (x.matches(sel)) return x; return null },
})
/** A handler named `name` for `[data-<key>]`, holding when that value is set (as most of the window's do). */
const H = (name, key, when = (t) => t.dataset[camel(key)]) => ({ sel: `[data-${key}]`, area: 'test', when, run: () => name })
const use = (...entries) => { CLICKS.length = 0; CLICKS.push(...entries) }
/** What a click on `target` runs: `[the element it is for, the handler's name or null]`, or null. */
const which = (target, e = { target }) => { const hit = clickFor(target, e); return hit && [hit.t, hit.c && hit.c.run()] }

const tests = []
const test = (name, f) => tests.push([name, f])

test('the click is for the nearest element a handler matches, even when one further out matches an earlier handler', () => {
  use(H('go', 'go'), H('call', 'call'))
  const row = el('div', { go: 'today' }), btn = el('button', { call: 'c1' }, row), icon = el('span', {}, btn)
  assert.deepEqual(which(icon), [btn, 'call'])
  assert.deepEqual(which(row), [row, 'go'])
})

test('on that element, the first handler in the registry\'s order that holds wins, whatever the selectors\' order', () => {
  const btn = el('button', { tab: 'chat', call: 'c1' })
  use(H('call', 'call'), H('tab', 'tab'))
  assert.deepEqual(which(btn), [btn, 'call'])
  use(H('tab', 'tab'), H('call', 'call'))
  assert.deepEqual(which(btn), [btn, 'tab'])
})

test('a handler matched but not holding passes to the next one that holds, on the same element', () => {
  use(H('set', 'set'), H('copy', 'copy'))
  const btn = el('button', { set: '', copy: 'text' })
  assert.deepEqual(which(btn), [btn, 'copy'])
})

test('when none holds, the click is still for that element and runs nothing: it never goes on to an element further out', () => {
  use(H('setsec', 'setsec'), H('go', 'go'))
  const row = el('div', { go: 'today' }), btn = el('button', { setsec: '' }, row)
  assert.deepEqual(which(btn), [btn, null])
})

test('a click on nothing a handler matches is for nothing', () => {
  use(H('go', 'go'))
  assert.equal(which(el('div', {}, el('main', {}))), null)
})

test('a handler\'s selector may be several, and name a tag; its `when` sees the click itself', () => {
  use({ sel: 'a[data-ext]', area: 'test', when: (t) => t.dataset.ext !== undefined, run: () => 'ext' },
    { sel: '[data-conv],[data-conv-go]', area: 'test', when: (t) => t.dataset.conv || t.dataset.convGo !== undefined, run: () => 'conv' },
    { sel: '[data-off]', area: 'test', when: (t, e) => t.dataset.off && (t.tagName === 'BUTTON' || e.target === t), run: () => 'off' })
  assert.equal(which(el('span', { ext: '' })), null)
  const a = el('a', { ext: '' })
  assert.deepEqual(which(a), [a, 'ext'])
  const go = el('button', { convGo: '' })
  assert.deepEqual(which(go), [go, 'conv'])
  // The dimmed page around a dialog: a click on it closes it, one inside the dialog doesn't.
  const dim = el('div', { off: '1' }), inside = el('p', {}, dim)
  assert.deepEqual(which(dim), [dim, 'off'])
  assert.deepEqual(which(inside), [dim, null])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`clicks: ${tests.length} passed`)
