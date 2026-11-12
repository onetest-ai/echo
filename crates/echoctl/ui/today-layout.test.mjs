// Today's layout engine (app.js, todayCols / todayLines): how many columns Today's width gives, and how the layout's
// rows fill them. Run by scripts/check-mod.sh (`node crates/echoctl/ui/today-layout.test.mjs`): the block is taken
// from app.js as it ships. Rules: the Workbench's TodayLayoutRules / TodayPanels / TodayResponsive (#328).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── today layout:')
const end = src.indexOf('// end of the today layout block', start)
assert.ok(start >= 0 && end > start, 'the today layout block not found in app.js')
const { todayCols, todayLines, todayPlan, todayTallIds } = new Function(`${src.slice(start, end)}\nreturn { todayCols, todayLines, todayPlan, todayTallIds }`)()

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

test('widgets after an empty one move up into its place when they fit, in order and at their size', () => {
  // Next meeting S beside an empty Needs you now; the choices empty beside GitHub M; routine cards empty (the report).
  const rows = [[W('next', 1), W('needs', 2, { empty: true })], [W('choices', 2, { empty: true }), W('github', 2)], [W('cards', 3, { empty: true })], [W('mine', 3, { fill: true })]]
  assert.equal(show(todayLines(rows, 3)), 'next:1 github:2 / mine:3 (fill)')
  // One that doesn't fit stays on its line, and so do the ones after it.
  assert.equal(show(todayLines([[W('a', 2), W('b', 1, { empty: true })], [W('c', 2), W('d', 1)]], 3)), 'a:2 / c:2 d:1')
  // The line it left has room for the next ones in turn.
  assert.equal(show(todayLines([[W('a', 1), W('b', 2, { empty: true })], [W('c', 1), W('d', 2)], [W('e', 1)]], 3)), 'a:1 c:1 / d:2 e:1')
  // Nothing empty: the rows as laid out.
  assert.equal(show(todayLines([[W('a', 1)], [W('b', 1)]], 3)), 'a:1 / b:1')
  // A fill widget never moves up beside others, and nothing moves up into a fill line.
  assert.equal(show(todayLines([[W('a', 1), W('b', 2, { empty: true })], [W('day', 2, { fill: true })]], 3)), 'a:1 / day:2 (fill)')
  assert.equal(show(todayLines([[W('day', 2, { fill: true }), W('b', 1, { empty: true })], [W('c', 1)]], 3)), 'day:2 (fill) / c:1')
})

test('a fill alone on the first row at span 1 holds the first column; the rest beside it in the others (TodayResponsive)', () => {
  const W = (id, span, fill = false, empty = false) => ({ id, span, fill, empty })
  const rows = [[W('day', 1, true)], [W('next', 1), W('needs', 1)], [W('choices', 2)], [W('mine', 2, true)]]
  const ids = (p) => p.lines.map((l) => l.items.map((i) => `${i.id}:${i.span}`).join(' ')).join(' | ')
  // A: 3 columns — day holds column 1, the rest in 2 columns.
  let p = todayPlan(rows, 3)
  assert.equal(p.hold.id, 'day')
  assert.equal(ids(p), 'next:1 needs:1 | choices:2 | mine:2')
  // B: 2 columns — day keeps column 1, the rest in one, spans clamp.
  p = todayPlan(rows, 2)
  assert.equal(p.hold.id, 'day')
  assert.equal(ids(p), 'next:1 | needs:1 | choices:1 | mine:1')
  // C: 1 column — nothing held; day steps down (the strip) and comes first, as a hug line.
  p = todayPlan(rows, 1)
  assert.equal(p.hold, null)
  assert.equal(p.down, 'day')
  assert.equal(ids(p), 'day:1 | next:1 | needs:1 | choices:1 | mine:1')
  assert.equal(p.lines[0].fill, false)
  // Not a holder: span 2, or not alone, or not a fill, or nothing beside it — rows as before.
  assert.equal(todayPlan([[W('day', 2, true)], [W('next', 1)]], 3).hold, null)
  assert.equal(todayPlan([[W('day', 1, true), W('next', 1)]], 3).hold, null)
  assert.equal(todayPlan([[W('needs', 1)], [W('next', 1)]], 3).hold, null)
  assert.equal(todayPlan([[W('day', 1, true)]], 3).hold, null)
  // Everything beside it empty now (temporal, nothing to show): no column held, Your day across.
  const p2 = todayPlan([[W('day', 1, true)], [W('next', 1, false, true)], [W('mine', 2, true, true)]], 3)
  assert.equal(p2.hold, null)
  assert.equal(ids(p2), 'day:1')
})

// A widget two rows high (#441): its row and the next, the rest beside it.
const tallShow = (lines) => lines.map((l) => (l.tall ? `[${l.tall.id}:${l.tall.span} ${l.tall.side} | ${show(l.lines)}]` : show([l]))).join(' / ')

test('a widget two rows high takes its row and the next, the rest of its row and the next row beside it', () => {
  const rows = [[W('mine', 1, { height: 2, fill: true }), W('next', 1), W('needs', 1)], [W('choices', 2)], [W('glance', 3)]]
  assert.equal(tallShow(todayLines(rows, 3)), '[mine:1 left | next:1 needs:1 / choices:2] / glance:3')
  const l = todayLines(rows, 3)[0]
  assert.equal(l.fill, true, 'a fill widget makes its line fill')
  assert.deepEqual(l.items, [{ id: 'mine', span: 1 }], 'its span counts as the line\'s')
})

test('not first in its row: on the right; the widgets before it beside it, in order', () => {
  const rows = [[W('next', 1), W('needs', 1), W('mine', 1, { height: 2 })], [W('choices', 2)]]
  assert.equal(tallShow(todayLines(rows, 3)), '[mine:1 right | next:1 needs:1 / choices:2]')
})

test('two rows high at M beside one column; wider than the page leaves a column beside it', () => {
  assert.equal(tallShow(todayLines([[W('day', 2, { height: 2 })], [W('a', 1), W('b', 1)]], 3)), '[day:2 left | a:1 / b:1]', 'b wraps under a in one column')
  assert.equal(tallShow(todayLines([[W('day', 3, { height: 2 })], [W('a', 1)]], 3)), '[day:2 left | a:1]', 'L narrows to leave one column')
})

test('one row high when nothing is beside it, at one column, or empty; only one of a row counts', () => {
  assert.equal(tallShow(todayLines([[W('day', 2, { height: 2 })]], 3)), 'day:2', 'nothing beside it')
  assert.equal(tallShow(todayLines([[W('day', 2, { height: 2 })], [W('a', 1)]], 1)), 'day:1 / a:1', 'one column: stacked')
  assert.equal(tallShow(todayLines([[W('day', 2, { height: 2, empty: true })], [W('a', 1)]], 3)), 'a:1', 'empty: no space')
  // The next row's own tall widget is one row high beside the first.
  assert.equal(tallShow(todayLines([[W('a', 1, { height: 2 })], [W('b', 1, { height: 2 }), W('c', 1)], [W('d', 3)]], 3)), '[a:1 left | b:1 c:1] / d:3')
  // An empty next row: the one after is next.
  assert.equal(tallShow(todayLines([[W('a', 1, { height: 2 })], [W('x', 1, { empty: true })], [W('b', 2)]], 3)), '[a:1 left | b:2]')
})

test('in the middle of its row it is one row high: nothing reorders', () => {
  const rows = [[W('next', 1), W('mine', 1, { height: 2 }), W('needs', 1)], [W('choices', 2)]]
  assert.equal(tallShow(todayLines(rows, 3)), 'next:1 mine:1 needs:1 / choices:2')
  assert.deepEqual(todayTallIds(todayLines(rows, 3)), [])
})

test('todayTallIds names the widgets drawn two rows high, not those set so but drawn one', () => {
  const rows = [[W('a', 1, { height: 2 }), W('b', 1, { height: 2 })], [W('c', 1, { height: 2 })], [W('d', 2, { height: 2 })]]
  // a takes its row and the next: b and c beside it are one row high; d has nothing beside it.
  assert.deepEqual(todayTallIds(todayLines(rows, 3)), ['a'])
  assert.deepEqual(todayTallIds(todayLines(rows, 1)), [], 'one column: none')
})

test('the plan lays two-rows-high widgets beside a held column too', () => {
  const rows = [[W('day', 1, { fill: true })], [W('mine', 1, { height: 2 }), W('next', 1)], [W('needs', 1)]]
  const p = todayPlan(rows, 3)
  assert.equal(p.hold.id, 'day')
  assert.equal(tallShow(p.lines), '[mine:1 left | next:1 / needs:1]')
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
