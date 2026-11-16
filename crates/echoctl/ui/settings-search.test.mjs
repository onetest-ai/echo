// Settings search (app.js, SettingsSearch): which sections a search keeps, by their names and their settings' own
// labels (#103). Run by scripts/check-mod.sh (`node crates/echoctl/ui/settings-search.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── settings search')
const end = src.indexOf('// end of the settings-search block', start)
assert.ok(start >= 0 && end > start, 'the settings-search block not found in app.js')
const { SettingsSearch } = new Function(`${src.slice(start, end)}\nreturn { SettingsSearch }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

// Sections as settingsView draws them: rows' titles in <b>, cards' heads in <h3>.
const row = (title) => `<div class="set-row"><div><b>${title}</b><div class="set-sub">about it, Dock words here too</div></div><div class="set-ctl"></div></div>`
const SECTIONS = [
  { id: 'general', label: 'General', html: `<h2>General</h2>${row('Start at login')}${row('Hide from the Dock when the window is closed')}` },
  { id: 'recording', label: 'Recording', html: `<section class="set-sec"><h3>When to record</h3>${row('Calendar meetings')}${row('Other Teams calls')}</section>` },
  { id: 'transcription', label: 'Transcription', html: `${row('base <span class="tag rec">Recommended for this computer</span>')}<h3>Audio</h3>${row('Keep audio files')}` },
  { id: 'calendar', label: 'Calendar & chat', html: row('Account') },
  { id: 'ai', label: 'AI & minutes', html: row('Where Claude Code runs for minutes') + row('Also write a minutes page &amp; file') },
  { id: 'templates', label: 'Templates', html: '' },
]
const ids = (q) => SettingsSearch.match(SECTIONS, q).map((s) => s.id)

test('nothing typed keeps every section, none picked out', () => {
  assert.deepEqual(ids(''), SECTIONS.map((s) => s.id))
  assert.deepEqual(ids('   '), SECTIONS.map((s) => s.id))
  assert.ok(SettingsSearch.match(SECTIONS, '').every((s) => s.hits.length === 0))
})

test("a setting's own label finds its section, with that setting picked out (#103)", () => {
  const found = SettingsSearch.match(SECTIONS, 'Dock')
  assert.deepEqual(found.map((s) => s.id), ['general'])
  assert.deepEqual(found[0].hits, ['Hide from the Dock when the window is closed'])
})

test('a section name still finds it; a name and a label both', () => {
  assert.deepEqual(ids('templ'), ['templates'])
  assert.deepEqual(ids('calendar'), ['recording', 'calendar'])
  assert.deepEqual(SettingsSearch.match(SECTIONS, 'calendar')[0].hits, ['Calendar meetings'])
})

test("cards' heads count; descriptions and badges don't", () => {
  assert.deepEqual(ids('audio'), ['transcription'])
  assert.deepEqual(SettingsSearch.match(SECTIONS, 'audio')[0].hits, ['Audio', 'Keep audio files'])
  assert.deepEqual(ids('recommended'), [])
  assert.deepEqual(ids('words here'), [])
})

test('case and entities: as the user reads the label', () => {
  assert.deepEqual(ids('KEEP AUDIO'), ['transcription'])
  assert.deepEqual(ids('page & file'), ['ai'])
  assert.deepEqual(ids('chat'), ['calendar'])
})

test('nothing matches: no section', () => {
  assert.deepEqual(ids('zzz'), [])
})

test('hit: a row title the search picks out', () => {
  assert.equal(SettingsSearch.hit('Hide from the Dock when the window is closed', 'dock'), true)
  assert.equal(SettingsSearch.hit('Start at login', 'dock'), false)
  assert.equal(SettingsSearch.hit('Start at login', ''), false)
  assert.equal(SettingsSearch.hit('<span class="set-ok">✓</span> Claude Code', 'claude code'), true)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} failed`); process.exit(1) }
console.log(`settings-search: ${tests.length} passed`)
