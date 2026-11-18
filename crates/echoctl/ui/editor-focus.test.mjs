// A field editor opened in the work panel takes the focus with its text selected, so typing replaces it (#793).
// Run by scripts/check-mod.sh (`node crates/echoctl/ui/editor-focus.test.mjs`): the block is taken from app.js as it ships.
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── editor focus')
const end = src.indexOf('// end of the editor focus block', start)
assert.ok(start >= 0 && end > start, 'the editor focus block not found in app.js')
const { focusEditor } = new Function(`${src.slice(start, end)}\nreturn { focusEditor }`)()

const field = (tag) => { const log = []; return { tagName: tag, log, focus: () => log.push('focus'), select: () => log.push('select') } }

{
  const i = field('INPUT'); focusEditor(i, () => {})
  assert.deepEqual(i.log, ['focus', 'select'], 'a text box is focused, then its text selected')
}
{
  const i = field('TEXTAREA'); focusEditor(i, () => {})
  assert.deepEqual(i.log, ['focus', 'select'])
}
{
  const i = field('SELECT'), opened = []
  focusEditor(i, (x) => opened.push(x))
  assert.deepEqual(i.log, ['focus'], 'a list is not selected')
  assert.deepEqual(opened, [i], 'a list opens at once')
}
{
  const i = field('INPUT'); i.select = () => { throw new Error('not selectable') }
  assert.doesNotThrow(() => focusEditor(i, () => {}), 'a box that cannot select (a date) still opens')
}
console.log('editor-focus: ok')
