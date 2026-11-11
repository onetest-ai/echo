// A plugin's widget drawn by its kind (app.js, pluginWidgetHtml): each kind's shape, text escaped, links only to
// echo://, wrong data refused with what is wrong. Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/today-plugin-widgets.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── plugin widget:'), b = src.indexOf('// end of the plugin widget block', a)
assert.ok(a >= 0 && b > a, 'the plugin widget block not found in app.js')
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const draw = new Function('esc', `${src.slice(a, b)}\nreturn pluginWidgetHtml`)(esc)
const tests = []
const test = (name, f) => tests.push([name, f])
const refused = (kind, data, why) => assert.throws(() => draw(kind, data, 'M'), (e) => e.message.includes(why), `${kind}: ${why}`)

test('a stat: the value big, its label; a tone colours it', () => {
  const h = draw('stat', { value: 42, label: 'open tickets', tone: 'warn' }, 'M')
  assert.match(h, /<b class="tone-warn">42<\/b>/)
  assert.match(h, /open tickets/)
  refused('stat', { label: 'x' }, 'needs value')
})

test('stats: up to 2 at S, 4 at M, 6 at L', () => {
  const items = Array.from({ length: 8 }, (_, i) => ({ label: `L${i}`, value: i }))
  for (const [size, n] of [['S', 2], ['M', 4], ['L', 6]]) assert.equal((draw('stats', { items }, size).match(/<div><b/g) || []).length, n, size)
  refused('stats', {}, 'need items')
})

test('a list: titles (an echo:// link opens in Echo, any other is text), 3 / 5 / 10 by size, its empty line', () => {
  const h = draw('list', { items: [{ title: 'Deploy', link: 'echo://task/acme/T1' }, { title: 'Phish', link: 'https://evil.example' }, { title: 'Plain', sub: 'note' }] }, 'M')
  assert.match(h, /<a class="link-btn" href="echo:\/\/task\/acme\/T1">Deploy<\/a>/)
  assert.ok(!h.includes('evil.example'), 'an outside link is not drawn')
  assert.equal((draw('list', { items: Array(12).fill({ title: 'x' }) }, 'L').match(/role="listitem"/g) || []).length, 10)
  assert.match(draw('list', { items: [], empty: 'All clear' }, 'M'), /All clear/)
  refused('list', { items: [{ title: { html: 1 } }] }, "item 1's title is not text")
})

test("others' text is escaped everywhere", () => {
  const evil = '<img src=x onerror=alert(1)>'
  for (const [kind, data] of [['stat', { value: evil, label: evil }], ['list', { items: [{ title: evil, sub: evil }] }], ['status', { state: 'bad', title: evil, text: evil }], ['card', { title: evil, text: evil }]]) {
    const h = draw(kind, data, 'M')
    assert.ok(!h.includes('<img'), `${kind}: ${h}`)
  }
})

test('a status: ok / warn / bad; a card: title and text, lines kept; an unknown kind refused', () => {
  assert.match(draw('status', { state: 'ok', title: 'CI green' }, 'M'), /st-ok/)
  refused('status', { state: 'red' }, 'state: ok, warn or bad')
  assert.match(draw('card', { title: 'Brief', text: 'one\ntwo' }, 'M'), /one<br>two/)
  refused('card', {}, 'title or text')
  refused('map', { x: 1 }, "doesn't draw the kind map")
  refused('stat', 'not an object', 'not an object')
})

test('a chart: bar and line as SVG with each point titled, bars as rows; wrong points refused', () => {
  const points = [{ label: 'Mon', value: 3 }, { label: 'Tue', value: 7 }, { label: 'Wed', value: 5 }]
  const bar = draw('chart', { type: 'bar', points, unit: 'calls' }, 'M')
  assert.equal((bar.match(/<rect /g) || []).length, 3)
  assert.match(bar, /<title>Tue: 7 calls<\/title>/)
  assert.match(draw('chart', { type: 'line', points }, 'M'), /<polyline points="/)
  assert.equal((draw('chart', { type: 'bars', points }, 'M').match(/class="pw-bl"/g) || []).length, 3)
  refused('chart', { type: 'pie', points }, 'type: bar, bars or line')
  refused('chart', { type: 'bar', points: [{ label: 'x', value: 'many' }] }, "point 1's value is not a number")
  assert.ok(!draw('chart', { type: 'bar', points: [{ label: '<b>x</b>', value: 1 }] }, 'M').includes('<b>x'), 'labels escaped')
})

test("a stat's progress; any kind's open (echo:// a link, https a button that asks) and actions (its plugin's commands)", () => {
  assert.match(draw('stat', { value: '$500', progress: { used: 300, of: 800 } }, 'M'), /width:37\.5%/)
  assert.match(draw('stat', { value: 1, open: 'echo://task/acme/T1' }, 'M'), /<a class="link-btn" href="echo:\/\/task\/acme\/T1">Open<\/a>/)
  const https = draw('stat', { value: 1, open: 'https://ci.example/run/1' }, 'M')
  assert.match(https, /data-pw-open="https:\/\/ci\.example\/run\/1"/)
  assert.ok(!https.includes('<a '), 'never a plain outside link')
  assert.ok(!draw('stat', { value: 1, open: 'javascript:alert(1)' }, 'M').includes('Open'))
  const acts = draw('status', { state: 'bad', title: 'CI', actions: [{ label: 'Re-run', command: 'rerun', args: { id: '"><img>' } }] }, 'M')
  assert.match(acts, /data-pw-cmd="rerun"/)
  assert.ok(!acts.includes('"><img>'), 'args escaped in the attribute')
  refused('stat', { value: 1, actions: [{ label: 'x' }] }, 'action 1 needs a command')
  assert.match(draw('list', { items: [], actions: [{ label: 'Refresh', command: 'refresh' }] }, 'M'), /data-pw-cmd="refresh"/, 'an empty list keeps its actions')
  assert.ok(!draw('stat', { value: 1, open: 'https://good.example\\@evil.example' }, 'M').includes('data-pw-open'), 'a backslash link is dropped')
  refused('chart', { type: 'bar', points: [{ label: 'x', value: null }] }, "point 1's value is not a number")
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
