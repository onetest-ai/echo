// Which dashboards a thread shows, what the "+" offers from plugins, and a preset dashboard's line (app.js, the
// board-presets block). Run by scripts/check-mod.sh (`node crates/echoctl/ui/board-presets.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── board-presets:')
const end = src.indexOf('// end of the board-presets block', start)
assert.ok(start >= 0 && end > start, 'the board-presets block not found in app.js')
const { boardShows, presetOffers, presetNote } = new Function(`${src.slice(start, end)}\nreturn { boardShows, presetOffers, presetNote }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test("a board for pursuit threads shows only on them; one without `for` on every thread", () => {
  assert.equal(boardShows({ for: ['pursuit'] }, 'pursuit'), true)
  assert.equal(boardShows({ for: ['pursuit'] }, 'delivery'), false)
  assert.equal(boardShows({}, 'closed'), true)
  assert.equal(boardShows({ for: [] }, 'delivery'), true)
})

test('the "+" offers the presets not added yet, by name', () => {
  const p = (name, added, plugin_name = 'Acme CRM') => ({ name, added, plugin_name, plugin: 'acme-crm', preset: name.toLowerCase() })
  assert.deepEqual(presetOffers([p('Staffing', false), p('Delivery', true), p('Pipeline', false), p('Pipeline', false, 'Globex Board')]).map((x) => `${x.name}/${x.plugin_name}`), ['Pipeline/Acme CRM', 'Pipeline/Globex Board', 'Staffing/Acme CRM'])
  assert.deepEqual(presetOffers(undefined), [])
})

test("a preset dashboard says when its plugin changed it — and that an update replaces the user's changes — or is off or went", () => {
  assert.equal(presetNote(undefined), null, "the user's own: no line")
  assert.equal(presetNote({ state: 'ok', plugin_name: 'Acme CRM' }), null)
  assert.equal(presetNote({ state: 'dropped', plugin_name: 'Acme CRM' }), null, 'no longer offered: it stays the user\'s, quietly')
  const up = presetNote({ state: 'update', plugin_name: 'Acme CRM', edited: false })
  assert.equal(up.text, 'Acme CRM updated this dashboard')
  assert.deepEqual(up.acts.map((a) => a[1]), ['Keep mine', 'Update'])
  assert.equal(presetNote({ state: 'update', plugin_name: 'Acme CRM', edited: true }).text, 'Acme CRM updated this dashboard — Update replaces your changes')
  assert.equal(presetNote({ state: 'gone', plugin_name: 'acme-crm' }).text, 'Its plugin was removed: its widgets show nothing')
  assert.equal(presetNote({ state: 'off', plugin_name: 'Acme CRM' }).text, 'Its plugin, Acme CRM, is off: its widgets show nothing')
  assert.deepEqual(presetNote({ state: 'off', plugin_name: 'Acme CRM' }).acts.map((a) => a[0]), ['remove-ask'])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
console.log(`board-presets: ${tests.length - failed} passed${failed ? `, ${failed} failed` : ''}`)
if (failed) process.exit(1)
