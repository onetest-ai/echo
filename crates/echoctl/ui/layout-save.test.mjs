// The window's layout save (app.js): a burst of changes is one write a moment later, and a close, quit or update writes
// a pending one first (#300). Run by scripts/check-mod.sh (`node crates/echoctl/ui/layout-save.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const block = (from, to) => {
  const a = src.indexOf(from), b = src.indexOf(to, a)
  assert.ok(a >= 0 && b > a, `${from} not found in app.js`)
  return src.slice(a, b)
}
const save = block('let layoutTimer = null', '/** The activity bar open or folded')
const flush = block('/** A layout change still waiting for its save', '\n}\n') + '\n}\n'
const make = () => {
  const sent = [], timers = new Map()
  let id = 0
  const env = {
    send: (m) => sent.push(m.cmd),
    render: () => {},
    Layout: { toJSON: () => ({}) },
    setTimeout: (f) => { timers.set(++id, f); return id },
    clearTimeout: (t) => timers.delete(t),
    timers,
  }
  const f = new Function(...Object.keys(env), `${save}\nfunction layoutChanged() {\n  clearTimeout(layoutTimer)\n  layoutTimer = setTimeout(() => { layoutTimer = null; send({ cmd: 'save_layout', layout: Layout.toJSON() }) }, 500)\n  render()\n}\n${flush}\nreturn { layoutChanged, flushLayout, fire: () => { for (const [k, g] of [...timers]) { timers.delete(k); g() } } }`)
  return { ...f(...Object.values(env)), sent }
}
const tests = []
const test = (name, f) => tests.push([name, f])

test('the copy of layoutChanged here is the one app.js ships', () => {
  assert.ok(src.includes("function layoutChanged() {\n  clearTimeout(layoutTimer)\n  layoutTimer = setTimeout(() => { layoutTimer = null; send({ cmd: 'save_layout', layout: Layout.toJSON() }) }, 500)\n  render()\n}"))
})

test('a burst of changes is one save, a moment later', () => {
  const w = make()
  w.layoutChanged(); w.layoutChanged()
  assert.deepEqual(w.sent, [])
  w.fire()
  assert.deepEqual(w.sent, ['save_layout'])
})

test('a pending save is sent at once by flushLayout, and not again when its moment comes', () => {
  const w = make()
  w.layoutChanged()
  w.flushLayout()
  assert.deepEqual(w.sent, ['save_layout'])
  w.fire()
  assert.deepEqual(w.sent, ['save_layout'])
})

test('nothing pending: flushLayout sends nothing', () => {
  const w = make()
  w.flushLayout()
  assert.deepEqual(w.sent, [])
})

test("the close handshake flushes the layout before it says it is ready", () => {
  const close = block('prepareClose(n, close) {', 'NotesSync.flushAll()')
  assert.ok(close.includes('flushLayout()'), 'prepareClose sends a pending layout save before close_ready')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
