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

// An edit that isn't the switch sends no `fixed`: the app keeps what the file has, also when it changed meanwhile.
assert.deepEqual(TplSave.message(d), { cmd: 'template_save', id: 'default', header: '# {title}', match: [], sections: d.sections })
// The switch set here is sent, on or off, with every save until a save that wrote it is answered.
const ticked = { ...d, fixed: true, fixedPending: true }
assert.equal(TplSave.message(ticked).fixed, true)
assert.equal(TplSave.message({ ...d, fixed: false, fixedPending: true }).fixed, false)
// A refused save has no answer: the switch is still pending and goes with the next save.
// The save that wrote it is answered (`sent`: what it wrote), and the file changed elsewhere before Echo read it back:
// no longer pending, the file's value shown, and a later edit here sends no `fixed`.
const done = { ...ticked, ...TplSave.saved(ticked, row(false), true) }
assert.deepEqual({ fixed: done.fixed, fixedPending: done.fixedPending }, { fixed: false, fixedPending: false })
assert.equal('fixed' in TplSave.message({ ...done, header: '# {title} · {date}' }), false)
// Unticked again before the first save was answered: that answer (it wrote `true`) leaves the new choice pending.
const back = { ...d, fixed: false, fixedPending: true }
assert.deepEqual(TplSave.saved(back, row(true), true), { fixed: false, fixedPending: true })
// Nothing pending: an answer shows the file's value (it may have changed elsewhere).
assert.deepEqual(TplSave.saved(d, row(true), null), { fixed: true, fixedPending: false })
// Create sends the switch as set in the new template's form, with its name.
const made = TplSave.create({ ...d, id: null, fixed: true }, 'Board review')
assert.equal(made.fixed, true)
assert.equal(made.name, 'Board review')
assert.equal(TplSave.create({ ...d, id: null }, 'Weekly').fixed, false)
console.log('templates-save: ok')
