// Today's edit mode (app.js, dropSide): where a dragged widget goes over another — its top or bottom quarter a new row
// above / below, its left or right half before / after it in its row. The rule of the edit-mode prototype (Workbench
// `WidgetsGallery`). Run by scripts/check-mod.sh (`node crates/echoctl/ui/today-edit.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── drop side:')
const end = src.indexOf('// end of the drop side block', start)
assert.ok(start >= 0 && end > start, 'the drop side block not found in app.js')
const dropSide = new Function(`${src.slice(start, end)}\nreturn dropSide`)()

const box = { left: 100, top: 200, width: 400, height: 200 }
assert.equal(dropSide(box, 300, 210), 'above', 'top quarter')
assert.equal(dropSide(box, 300, 395), 'below', 'bottom quarter')
assert.equal(dropSide(box, 150, 300), 'before', 'left half')
assert.equal(dropSide(box, 450, 300), 'after', 'right half')
assert.equal(dropSide(box, 300, 249), 'above', 'just inside the top quarter')
assert.equal(dropSide(box, 300, 251), 'after', 'just under it, at the middle: after')
console.log('today-edit: ok')
