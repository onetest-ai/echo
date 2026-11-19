// A Claude panel's ended session: the card and the bar offer New conversation beside Resume — only for a `claude:` key
// whose session ended on its own, not on an error or a sign-in. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/ended-card.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const from = 'function termBody(', to = '// ── Claude in the window'
const start = src.indexOf(from), end = src.indexOf(to, start)
assert.ok(start >= 0 && end > start, 'termBody not found in app.js')
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const S = {}
const body = new Function('esc', 'S', `${src.slice(start, end)}\nreturn termBody`)(esc, S)
const card = (e, key = 'claude:folder', kind = 'claude-folder', extra = {}) => body(e, key, kind, 'Start', '', extra, false, `data-term-start="${kind}" data-term-key="${key}"`)

const tests = []
const test = (name, f) => tests.push([name, f])

test('the ended card: Resume, New conversation, Show output, in that order', () => {
  const h = card({ exited: true })
  const [r, n, o] = ['>Resume<', '>New conversation<', '>Show output<'].map((t) => h.indexOf(t))
  assert.ok(r > 0 && n > r && o > n, h)
  assert.match(h, /<button class="btn" data-conv="new" data-term-kind="claude-folder" data-term-key="claude:folder">New conversation<\/button>/)
})

test('a plugin session and a call keep what the handler needs, escaped', () => {
  assert.match(card({ exited: true }, 'claude:plugin:a"b', 'claude-plugin'), /data-term-key="claude:plugin:a&quot;b"/)
  assert.match(card({ exited: true }, 'claude:folder', 'claude-folder', { call: 'c1' }), /data-conv="new"[^>]*data-term-call="c1"/)
})

test('no New conversation on an error, a sign-in, or a key that is not Claude', () => {
  assert.ok(!card({ exited: true, error: 'boom' }).includes('New conversation'))
  assert.ok(!card({ exited: true, signin: 'p1' }).includes('New conversation'))
  assert.ok(!card({ exited: true }, 'shell:1', 'shell').includes('New conversation'))
})

test('the output bar has it next to Resume', () => {
  S.termOutput = { 'claude:folder': true }
  const h = card({ exited: true })
  assert.ok(h.indexOf('>Resume<') < h.indexOf('>New conversation<'), h)
  S.termOutput = {}
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) { console.log(`${failed} of ${tests.length} failed`); process.exit(1) }
console.log(`all ${tests.length} passed`)
