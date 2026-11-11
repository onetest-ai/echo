// What a save of the open template sends (31-templates.js, TplSave). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/templates-save.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── template save:')
const end = src.indexOf('// end of the template save block', start)
assert.ok(start >= 0 && end > start, 'the template save block not found in app.js')
const { TplSave } = new Function(`${src.slice(start, end)}\nreturn { TplSave }`)()

const d = { id: 'default', header: '# {title}', match: [], sections: [{ heading: 'Summary', instruction: '' }], fixed: false }
// An edit that isn't the switch sends no `fixed`: the app keeps what the file has, also when it changed meanwhile.
assert.deepEqual(TplSave.message(d), { cmd: 'template_save', id: 'default', header: '# {title}', match: [], sections: d.sections })
// Once the switch is set in this editor, every save carries it.
assert.equal(TplSave.message({ ...d, fixed: true, fixedSet: true }).fixed, true)
assert.equal(TplSave.message({ ...d, fixed: false, fixedSet: true }).fixed, false)
console.log('templates-save: ok')
