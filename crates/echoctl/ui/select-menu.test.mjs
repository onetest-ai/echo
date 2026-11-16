// Echo's menu for the page's selects (app.js, the select menu block): its rows, moving through them, type-ahead and
// where it opens. Run by scripts/check-mod.sh (`node crates/echoctl/ui/select-menu.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── select menu\n')
const end = src.indexOf('// end of the select menu block', start)
assert.ok(start >= 0 && end > start, 'the select menu block not found in app.js')
const { selectItems, selectStep, selectEnd, selectFind, selectPlace } = new Function(`${src.slice(start, end)}\nreturn { selectItems, selectStep, selectEnd, selectFind, selectPlace }`)()

const rows = selectItems([
  { group: 'Claude' },
  { value: 'system', label: 'Claude Code (as configured)', sub: 'sonnet · as Claude Code is set up', selected: true },
  { value: 'acme', label: 'Acme work — dana@acme.example', sub: 'opus' },
  { group: 'Local' },
  { value: 'off', label: 'Old server', disabled: true },
  { value: 'local', label: 'Local', sub: 'qwen3.8:27b · on this computer' },
])

assert.deepEqual(rows.map((r) => r.kind), ['group', 'item', 'item', 'group', 'item', 'item'])
assert.equal(rows[1].sub, 'sonnet · as Claude Code is set up')
assert.equal(rows[5 - 1].sub, '', 'no sub: an empty line, not "undefined"')
assert.equal(rows[1].selected, true)

// Arrows skip group labels and disabled rows, and wrap.
assert.equal(selectStep(rows, 1, 1), 2)
assert.equal(selectStep(rows, 2, 1), 5, 'past the group label and the disabled server')
assert.equal(selectStep(rows, 5, 1), 1, 'wraps to the first')
assert.equal(selectStep(rows, 1, -1), 5, 'wraps to the last')
assert.equal(selectEnd(rows, 1), 1)
assert.equal(selectEnd(rows, -1), 5)
assert.equal(selectStep(selectItems([{ group: 'none' }, { value: 'x', label: 'x', disabled: true }]), 0, 1), -1)

// Type-ahead: by the start of the name, any case, from the next row on, never a disabled one.
assert.equal(selectFind(rows, 1, 'a'), 2)
assert.equal(selectFind(rows, 2, 'l'), 5)
assert.equal(selectFind(rows, 5, 'cl'), 1, 'wraps')
assert.equal(selectFind(rows, 1, 'o'), -1, 'the disabled one is never found')
assert.equal(selectFind(rows, 1, ''), -1)

// Where it opens: on its field, the chosen row's name right over the field's text (as the Mac's own); inside the view.
const view = { width: 1000, height: 800 }
// A 28 px field at y 400; the chosen row's name 60 px down the menu, 16 px high; its text 19 px further in than the field's.
const on = selectPlace({ left: 300, top: 400, height: 28, width: 280 }, { width: 320, height: 200, rowTop: 60, rowHeight: 16, inset: 19 }, view)
assert.equal(on.top, 400 + 14 - (60 + 8), 'the chosen name centred on the field')
assert.equal(on.left, 300 - 19, 'its text on the field\'s text')
assert.equal(on.scroll, 0)
assert.equal(on.minWidth, 280 + 19, 'as wide as the field at least')
// Near the top: kept inside, the rows above the chosen one scrolled away so it still lies on the field.
const high = selectPlace({ left: 300, top: 20, height: 28, width: 280 }, { width: 320, height: 300, rowTop: 200, rowHeight: 16, inset: 0 }, view)
assert.equal(high.top, 8)
assert.equal(high.scroll, 8 - (20 + 14 - 208), 'scrolled by as much as it was moved down')
// Near the bottom: moved up to stay inside (the chosen row then above the field; nothing to scroll).
const low = selectPlace({ left: 300, top: 760, height: 28, width: 280 }, { width: 320, height: 200, rowTop: 10, rowHeight: 16, inset: 0 }, view)
assert.equal(low.top, 800 - 8 - 200)
assert.equal(low.scroll, 0)
// Near the right edge, and a list taller than the view: inside, scrolling.
assert.equal(selectPlace({ left: 900, top: 100, height: 28, width: 90 }, { width: 300, height: 100, rowTop: 0, rowHeight: 16, inset: 0 }, view).left, 1000 - 300 - 8)
const tall = selectPlace({ left: 10, top: 100, height: 28, width: 90 }, { width: 200, height: 2000, rowTop: 0, rowHeight: 16, inset: 0 }, view)
assert.equal(tall.top, 100 + 14 - 8, 'the chosen row on the field')
assert.equal(tall.maxHeight, 800 - 8 - 106, 'down to the view\'s bottom, scrolling')
console.log('select-menu: ok')
