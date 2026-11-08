// Settings → Layout's "Echo's own title bar" (app.js, ownTitleBarCard; #76): a Windows-only row, by the app's platform —
// absent elsewhere, so Settings search can't find it there either. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/own-title-bar.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const block = (from, to) => {
  const start = src.indexOf(from)
  const end = src.indexOf(to, start)
  assert.ok(start >= 0 && end > start, `${from} block not found in app.js`)
  return src.slice(start, end)
}
// The page's own helpers, reduced to what the search reads: a row's title in <b>, a card's head in <h3>.
const setCard = (head, rows) => `<section class="set-sec">${head ? `<h3>${head}</h3>` : ''}${rows.join('')}</section>`
const setRow = (title, sub, ctl) => `<div class="set-row"><div><b>${title}</b><div class="set-sub">${sub}</div></div><div class="set-ctl">${ctl}</div></div>`
const sw = (key, on) => `<button class="sw${on ? ' on' : ''}" role="switch" aria-checked="${!!on}" data-set="${key}" data-val="${!on}"></button>`
const { ownTitleBarCard } = new Function('setCard', 'setRow', 'sw', `${block('// ── own title bar row', '// end of the own-title-bar block')}\nreturn { ownTitleBarCard }`)(setCard, setRow, sw)
const { SettingsSearch } = new Function(`${block('// ── settings search', '// end of the settings-search block')}\nreturn { SettingsSearch }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const layout = (platform) => [{ id: 'layout', label: 'Layout', html: `<h2>Layout</h2>${ownTitleBarCard({ own_title_bar: true }, { platform })}` }]

test('on Windows the row is there, its switch as saved', () => {
  const on = ownTitleBarCard({ own_title_bar: true }, { platform: 'windows' })
  assert.match(on, /<b>Echo's own title bar<\/b>/)
  assert.match(on, /data-set="own_title_bar" data-val="false"/)
  assert.match(ownTitleBarCard({ own_title_bar: false }, { platform: 'windows' }), /data-val="true"/)
})

test('on macOS, Linux, or with no state yet, there is no row at all', () => {
  for (const state of [{ platform: 'macos' }, { platform: 'linux' }, {}, null, undefined]) assert.equal(ownTitleBarCard({ own_title_bar: true }, state), '')
})

test('Settings search finds it on Windows only', () => {
  assert.deepEqual(SettingsSearch.match(layout('windows'), 'title bar').map((s) => s.hits), [["Echo's own title bar"]])
  assert.deepEqual(SettingsSearch.match(layout('macos'), 'title bar'), [])
  assert.deepEqual(SettingsSearch.match(layout('linux'), 'own title'), [])
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
