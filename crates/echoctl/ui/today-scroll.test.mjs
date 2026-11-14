// Your day's scroll (app.js, tdScrollIsUser): a scroll of the list is the user's unless it is where Today placed it, or
// the browser keeping it at the list's end above that place (the list got taller without a redraw). Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/today-scroll.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf("// ── Your day's scroll:"), b = src.indexOf('// end of the day scroll block', a)
assert.ok(a >= 0 && b > a, "Your day's scroll block not found in app.js")
const tdScrollIsUser = new Function(`${src.slice(a, b)}\nreturn tdScrollIsUser`)()
const tests = []
const test = (name, f) => tests.push([name, f])

test("where Today placed it: Today's own scroll", () => {
  assert.equal(tdScrollIsUser(148, 148, 300), false)
  assert.equal(tdScrollIsUser(0, 0, 300), false)
})

test('elsewhere: the user', () => {
  assert.equal(tdScrollIsUser(0, 148, 300), true)
  assert.equal(tdScrollIsUser(200, 148, 300), true)
  assert.equal(tdScrollIsUser(40, undefined, 300), true)
})

test("at the list's end above where Today placed it (a taller list, no redraw): the browser's, not the user's", () => {
  assert.equal(tdScrollIsUser(100, 148, 100), false)
  assert.equal(tdScrollIsUser(100.5, 148, 100), false)
})

test("at the list's end below where Today placed it, or above it but not at the end: the user", () => {
  assert.equal(tdScrollIsUser(300, 148, 300), true)
  assert.equal(tdScrollIsUser(90, 148, 100), true)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
