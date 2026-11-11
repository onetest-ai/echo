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
// The switch set here is sent, on or off, with every save until the file has it.
const ticked = { ...d, fixed: true, fixedPending: true }
assert.equal(TplSave.message(ticked).fixed, true)
assert.equal(TplSave.message({ ...d, fixed: false, fixedPending: true }).fixed, false)
// The save was refused (the file still off): still pending, the next save sends it again.
assert.deepEqual(TplSave.saved(ticked, row(false)), { fixed: true, fixedPending: true })
// The file has it: no longer sent. Then the file changes elsewhere and the header is edited here: no `fixed` is sent,
// and the app's answer shows the file's newer value.
const done = { ...ticked, ...TplSave.saved(ticked, row(true)) }
assert.equal(done.fixedPending, false)
assert.equal('fixed' in TplSave.message({ ...done, header: '# {title} · {date}' }), false)
assert.deepEqual(TplSave.saved(done, row(false)), { fixed: false, fixedPending: false })
// Create sends the switch as set in the new template's form, with its name.
const made = TplSave.create({ ...d, id: null, fixed: true }, 'Board review')
assert.equal(made.fixed, true)
assert.equal(made.name, 'Board review')
assert.equal(TplSave.create({ ...d, id: null }, 'Weekly').fixed, false)
console.log('templates-save: ok')
