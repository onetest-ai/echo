// Today's rows (app.js, todayRows): its widgets in the layout's order, a row each where one starts. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/today-rows.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── today rows:')
const end = src.indexOf('// end of the today rows block', start)
assert.ok(start >= 0 && end > start, 'the today rows block not found in app.js')
const todayRows = new Function(`${src.slice(start, end)}\nreturn todayRows`)()
const DRAWN = ['core:needs.now', 'core:routines.cards', 'core:calendar.agenda', 'core:tasks.mine']

// No layout: Today as it comes — Needs you now, the cards, then Your day beside My work.
assert.deepEqual(todayRows(undefined, DRAWN), [['core:needs.now'], ['core:routines.cards'], ['core:calendar.agenda', 'core:tasks.mine']])
// The layout's order and rows, as saved.
const moved = [{ widget: 'core:tasks.mine', row: true }, { widget: 'core:needs.now', row: true }, { widget: 'core:routines.cards' }, { widget: 'core:calendar.agenda', row: true }]
assert.deepEqual(todayRows(moved, DRAWN), [['core:tasks.mine'], ['core:needs.now', 'core:routines.cards'], ['core:calendar.agenda']])
// A widget the page can't draw (yet, or a plugin's that is gone) is left out; its row start doesn't open an empty row.
const odd = [{ widget: 'core:calendar.strip', row: true }, { widget: 'core:tasks.mine' }, { widget: 'core:needs.now', row: true }]
assert.deepEqual(todayRows(odd, DRAWN), [['core:tasks.mine'], ['core:needs.now']])
// Nothing on Today: no rows.
assert.deepEqual(todayRows([], DRAWN), [])
console.log('today-rows: ok')
