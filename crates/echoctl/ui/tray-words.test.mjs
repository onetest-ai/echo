// Where Echo's octopus lives, in the window's words (trayWords, #420): the menu bar on macOS, the notification area on
// Windows. Run by scripts/check-mod.sh (`node crates/echoctl/ui/tray-words.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── tray words', to = '// end of the tray-words block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'tray-words block not found in app.js')
const { trayWords } = new Function(`${src.slice(start, end)}\nreturn { trayWords }`)()

const tests = []
const test = (name, f) => tests.push([name, f])

test('macOS keeps its words', () => {
  const w = trayWords(false)
  assert.equal(w.octopus, 'the menu-bar octopus')
  assert.equal(w.startAtLogin, 'Echo runs in the menu bar from when you log in.')
})

test('Windows says the notification area, never the menu bar', () => {
  const w = trayWords(true)
  assert.equal(w.octopus, 'the octopus in the notification area')
  assert.equal(w.startAtLogin, 'Echo runs in the notification area from when you sign in.')
  for (const v of Object.values(w)) assert.doesNotMatch(v, /menu.bar/i)
})

test('the page says where the octopus is only through trayWords', () => {
  const rest = src.slice(0, start) + src.slice(end)
  assert.doesNotMatch(rest, /menu-bar octopus|runs in the menu bar/)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
