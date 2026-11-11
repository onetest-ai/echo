// Where Echo's octopus lives, in the window's words (trayWords, #420): the menu bar on macOS, the notification area on
// Windows. Run by scripts/check-mod.sh (`node crates/echoctl/ui/tray-words.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = '// ── tray words', to = '// end of the tray-words block'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'tray-words block not found in app.js')
// The block run as a window with this user agent would run it.
const block = (userAgent) => new Function('navigator', `${src.slice(start, end)}\nreturn { trayWords, TRAY }`)({ userAgent })
const { trayWords } = block('')
const WEBVIEW2 = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0'
const WKWEBVIEW = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)'

const tests = []
const test = (name, f) => tests.push([name, f])

test('the window picks the words by its platform: WebView2 on Windows, WKWebView on macOS', () => {
  assert.equal(block(WEBVIEW2).TRAY.octopus, 'the octopus in the notification area')
  assert.equal(block(WKWEBVIEW).TRAY.octopus, 'the menu-bar octopus')
})

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
