// Where a Work board column's menu goes (app.js, colMenuSpot). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/column-menu.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── column menu: where a board column\'s menu goes')
const end = src.indexOf('// end of the column menu block', start)
assert.ok(start >= 0 && end > start, 'the column menu block not found in app.js')
const { colMenuSpot } = new Function(`${src.slice(start, end)}\nreturn { colMenuSpot }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const win = { width: 1440, height: 900 }
const menu = { width: 240, height: 285 }
const inside = (at, m = menu, w = win) => at.left >= 8 && at.top >= 8 && at.left + m.width <= w.width - 8 && at.top + m.height <= w.height - 8

// The board of a thread with one card is about 140 px tall (top 275, bottom 414); its menu twice that.
// Placed against the window, not the board, the whole menu shows: Hide and Delete column too (#109).
test('under its button, right edges lined up, whatever the board\'s height (#109)', () => {
  const at = colMenuSpot({ top: 283, bottom: 307, left: 1380, right: 1404 }, menu, win)
  assert.deepEqual(at, { top: 311, left: 1404 - 240 })
  assert.ok(inside(at))
})

test('above its button when there is no room below', () => {
  const at = colMenuSpot({ top: 760, bottom: 784, left: 600, right: 624 }, menu, win)
  assert.equal(at.top, 760 - 4 - 285)
  assert.ok(inside(at))
})

test('kept inside the window: at its left edge, and in a short window', () => {
  assert.equal(colMenuSpot({ top: 100, bottom: 124, left: 40, right: 64 }, menu, win).left, 8)
  const low = { width: 800, height: 320 }
  const at = colMenuSpot({ top: 150, bottom: 174, left: 400, right: 424 }, menu, low)
  assert.ok(inside(at, menu, low), JSON.stringify(at))
  // Taller than the window: from its top (the menu scrolls, `max-height` in its CSS).
  assert.equal(colMenuSpot({ top: 150, bottom: 174, left: 400, right: 424 }, menu, { width: 800, height: 200 }).top, 8)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`column menu: ${tests.length} passed`)
