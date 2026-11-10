// Today's layout engine (app.js, todayCols / todayLines): how many columns Today's width gives, and how the layout's
// rows fill them. Run by scripts/check-mod.sh (`node crates/echoctl/ui/today-layout.test.mjs`): the block is taken
// from app.js as it ships. Rules: the Workbench's TodayLayoutRules / TodayPanels / TodayResponsive (#328).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── today layout:')
const end = src.indexOf('// end of the today layout block', start)
assert.ok(start >= 0 && end > start, 'the today layout block not found in app.js')
const { todayCols, todayLines } = new Function(`${src.slice(start, end)}\nreturn { todayCols, todayLines }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('columns from the width: 340 px a column, at most 3', () => {
  assert.equal(todayCols(1350), 3) // 1512 window, menu folded
  assert.equal(todayCols(1180), 3) // 1440, menu open
  assert.equal(todayCols(900), 2) // a task open on the right
  assert.equal(todayCols(700), 2)
  assert.equal(todayCols(600), 1) // 800 window beside Teams
  assert.equal(todayCols(4000), 3)
  assert.equal(todayCols(0), 1)
})

test('folding and unfolding have room to settle: fold under 340 a column, unfold only at 380', () => {
  assert.equal(todayCols(1000, 3), 2, 'under 3 × 340 + gaps: 2')
  assert.equal(todayCols(1080, 2), 2, 'from 2, wider but under 3 × 380: still 2')
  assert.equal(todayCols(1180, 2), 3, 'from 2, at 3 × 380: 3')
  assert.equal(todayCols(1060, 3), 3, 'from 3, a little narrower but over 3 × 340: still 3')
  assert.equal(todayCols(700, 1), 1, 'from 1, under 2 × 380: still 1')
  assert.equal(todayCols(800, 1), 2)
})

const W = (id, span, extra = {}) => ({ id, span, ...extra })
const ROWS = [[W('needs', 3, { temporal: true })], [W('cards', 3, { temporal: true, empty: true })], [W('day', 2, { fill: true }), W('mine', 1, { fill: true })]]
const show = (lines) => lines.map((l) => l.items.map((i) => `${i.id}:${i.span}`).join(' ') + (l.fill ? ' (fill)' : '')).join(' / ')

test('the default at 3 columns: Needs you now across, Your day 2 beside My work 1; an empty card takes no line', () => {
  assert.equal(show(todayLines(ROWS, 3)), 'needs:3 / day:2 mine:1 (fill)')
})

test('at 2 columns the widest narrows first, so Your day and My work stay side by side', () => {
  assert.equal(show(todayLines(ROWS, 2)), 'needs:2 / day:1 mine:1 (fill)')
})

test('at 1 column everything stacks, in order', () => {
  assert.equal(show(todayLines(ROWS, 1)), 'needs:1 / day:1 (fill) / mine:1 (fill)')
})

test('a row of more widgets than columns wraps after narrowing all to 1', () => {
  const row = [[W('a', 2), W('b', 2), W('c', 1)]]
  assert.equal(show(todayLines(row, 3)), 'a:1 b:1 c:1')
  assert.equal(show(todayLines(row, 2)), 'a:1 b:1 / c:1')
})

test('a row that fits keeps its spans; nothing reorders', () => {
  assert.equal(show(todayLines([[W('a', 1), W('b', 1)], [W('c', 2)]], 3)), 'a:1 b:1 / c:2')
})

test('an empty row takes no line; an empty widget leaves its row to the others', () => {
  assert.equal(show(todayLines([[W('a', 3, { empty: true })], [W('b', 1, { empty: true }), W('c', 1)]], 3)), 'c:1')
  assert.deepEqual(todayLines([], 3), [])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
