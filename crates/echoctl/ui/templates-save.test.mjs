// What a save of a template sends, and what the editor keeps when the app answers (31-templates.js, TplSave). Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/templates-save.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── template save:')
const end = src.indexOf('// end of the template save block', start)
assert.ok(start >= 0 && end > start, 'the template save block not found in app.js')
const { TplSave } = new Function(`${src.slice(start, end)}\nreturn { TplSave }`)()

const d = { id: 'default', header: '# {title}', match: [], sections: [{ heading: 'Summary', instruction: '' }], fixed: false }
const row = (fixed) => ({ id: 'default', kind: 'edited', fixed })
const set = (draft, on) => ({ ...draft, ...TplSave.toggle(on) })

// An edit that isn't the switch sends no `fixed`: the app keeps what the file has, also when it changed meanwhile.
assert.deepEqual(TplSave.message(d), { cmd: 'template_save', id: 'default', header: '# {title}', match: [], sections: d.sections })
// The switch set here is sent, on or off, with its own number, with every save until a save that sent it is answered.
const ticked = set(d, true)
assert.deepEqual([TplSave.message(ticked).fixed, TplSave.message(ticked).fixedGen], [true, ticked.fixedGen])
assert.equal(TplSave.message(set(d, false)).fixed, false)
// A refused save has no answer: the switch is still pending and goes with the next save.
// The save that sent it is answered (`gen`: the number it sent), and the file changed elsewhere before Echo read it
// back: no longer pending, the file's value shown, and a later edit here sends no `fixed`.
const done = { ...ticked, ...TplSave.saved(ticked, 'default', row(false), ticked.fixedGen) }
assert.deepEqual({ fixed: done.fixed, fixedPending: done.fixedPending }, { fixed: false, fixedPending: false })
assert.equal('fixed' in TplSave.message({ ...done, header: '# {title} · {date}' }), false)
// On (saved), off (saved), on again (refused): the late answers to the first two leave the last choice pending, and
// the next save sends it; its answer ends it.
const on1 = set(d, true), off = set(on1, false), on2 = set(off, true)
assert.deepEqual(TplSave.saved(on2, 'default', row(false), on1.fixedGen), { fixed: true, fixedPending: true })
assert.deepEqual(TplSave.saved(on2, 'default', row(false), off.fixedGen), { fixed: true, fixedPending: true })
assert.deepEqual([TplSave.message(on2).fixed, TplSave.message(on2).fixedGen], [true, on2.fixedGen])
assert.deepEqual(TplSave.saved(on2, 'default', row(true), on2.fixedGen), { fixed: true, fixedPending: false })
// Nothing pending: an answer shows the file's value (it may have changed elsewhere).
assert.deepEqual(TplSave.saved(d, 'default', row(true), null), { fixed: true, fixedPending: false })
// The answer to another template's save (Default's, while Discussion is open with its switch pending after a
// refused save) changes nothing in the open one.
const other = { ...set(d, true), id: 'discussion' }
assert.deepEqual(TplSave.saved(other, 'default', { id: 'discussion', fixed: false }, other.fixedGen), {})
// Create sends the switch as set in the new template's form, with its name.
const made = TplSave.create({ ...d, id: null, fixed: true }, 'Board review')
assert.equal(made.fixed, true)
assert.equal(made.name, 'Board review')
assert.equal(TplSave.create({ ...d, id: null }, 'Weekly').fixed, false)
console.log('templates-save: ok')
