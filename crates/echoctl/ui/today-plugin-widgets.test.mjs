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
const failedView = new Function('esc', `${src.slice(a, b)}\nreturn pluginWidgetFailed`)(esc)
const tests = []
const test = (name, f) => tests.push([name, f])
const refused = (kind, data, why) => assert.throws(() => draw(kind, data, 'M'), (e) => e.message.includes(why), `${kind}: ${why}`)

test('never answered and its ask failed: why, escaped; answered with nothing, being asked, or with earlier data: none', () => {
  const err = "mix-demo didn't answer __widget:hang within 5 s <b>"
  assert.equal(failedView({ empty: true, error: err, at: null, pending: false }), `<p class="set-sub warn">Couldn't get its data — mix-demo didn&#39;t answer __widget:hang within 5 s &lt;b&gt;</p>`)
  assert.equal(failedView({ empty: true, error: null, at: '2026-11-11T10:00:00Z', pending: false }), '', 'answered with nothing: no place')
  assert.equal(failedView({ empty: true, error: null, at: null, pending: true }), '', 'being asked: the placeholder')
  assert.equal(failedView({ empty: false, error: err, at: '2026-11-11T10:00:00Z', data: { value: 1 } }), '', 'its last data, dated')
  assert.equal(failedView({ empty: true, error: err, at: '2026-11-11T10:00:00Z' }), '', 'cleared, then a failed ask: still no place')
})

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

const table = {
  columns: [
    { key: 'urgency', label: 'Urgency', kind: 'pill' }, { key: 'pos', label: 'Position' }, { key: 'start', label: 'Start', kind: 'date' },
    { key: 'cand', label: 'Candidates', kind: 'number' }, { key: 'why', label: 'Why', hide: 'M' },
  ],
  groups: [{ id: 'nwb', label: 'NWB · Northwind' }, { id: 'ctl', label: 'CTL · Contoso' }],
  rows: [
    { id: 'p3', group: 'ctl', cells: { urgency: { text: 'High', tone: 'warn' }, pos: 'P-3', start: '2026-11-24', cand: 4, why: 'not booked' } },
    { id: 'p1', group: 'nwb', link: 'https://staffing.acme.example/p/1', cells: { urgency: { text: 'Critical', tone: 'bad' }, pos: 'P-1', start: '2026-11-15', cand: 3 }, detail: [{ text: 'Dana Example', sub: 'Proposed' }] },
    { id: 'p9', cells: { urgency: 'Low', pos: 'P-9', start: '', cand: 0, why: { text: 'on hold', tone: 'muted' } }, link: 'echo://thread/acme' },
  ],
}

test('a table: its columns (a pill, a short date, a number at the end), rows under their group in the groups\' order, the rest last', () => {
  const h = draw('table', table, 'L')
  assert.match(h, /<th scope="col" class="k-pill">Urgency<\/th>/)
  assert.match(h, /<span class="pw-pill tone-bad">Critical<\/span>/)
  assert.match(h, /<time datetime="2026-11-15">Nov 15<\/time>/)
  assert.match(h, /<td class="k-number">3<\/td>/)
  const at = (t) => h.indexOf(t)
  assert.ok(at('NWB · Northwind') < at('P-1') && at('P-1') < at('CTL · Contoso') && at('CTL · Contoso') < at('P-3') && at('P-3') < at('Other') && at('Other') < at('P-9'), 'groups in their order, the rest last')
  assert.match(h, /NWB · Northwind <span class="mono">1<\/span>/, "a group's count")
  assert.match(h, /<td class="k-text tone-muted" title="on hold">on hold<\/td>/)
})

test('a table at M leaves out the columns hidden at M; at L it has them', () => {
  assert.ok(!draw('table', table, 'M').includes('>Why<'))
  assert.ok(draw('table', table, 'L').includes('>Why<'))
})

test("a table row's link is on its first text cell (https asks first, echo:// opens in Echo, anything else is text); its details show when opened", () => {
  const h = draw('table', table, 'M')
  assert.match(h, /<td class="k-text" title="P-1"><button type="button" class="link-btn" data-pw-open="https:\/\/staffing\.acme\.example\/p\/1">P-1<\/button><\/td>/)
  assert.match(h, /<a class="link-btn" href="echo:\/\/thread\/acme">P-9<\/a>/)
  const evil = draw('table', { columns: [{ key: 'a', label: 'A' }], rows: [{ id: 1, link: 'javascript:alert(1)', cells: { a: 'x' } }] }, 'M')
  assert.ok(!evil.includes('javascript') && evil.includes('<td class="k-text" title="x">x</td>'))
  assert.match(h, /data-pw-row="p1" aria-expanded="false" aria-label="Show details">▸/)
  assert.ok(!h.includes('Dana Example'), 'closed: no details')
  const open = draw('table', table, 'M', ['p1'])
  assert.match(open, /aria-expanded="true" aria-label="Hide details">▾/)
  assert.match(open, /<span>Dana Example<\/span><span class="set-sub">Proposed<\/span>/)
})

test('a table with no rows says its empty text; its text is escaped; wrong data refused with what is wrong', () => {
  assert.match(draw('table', { columns: [{ key: 'a', label: 'A' }], rows: [], empty: 'Every position is assigned.' }, 'M'), /Every position is assigned\./)
  const evil = '<img src=x onerror=alert(1)>'
  const h = draw('table', { columns: [{ key: 'a', label: evil, kind: 'pill' }], groups: [{ id: 'g', label: evil }], rows: [{ id: 1, group: 'g', cells: { a: evil }, detail: [{ text: evil, sub: evil }] }] }, 'M', ['1'])
  assert.ok(!h.includes('<img'), h)
  const cols = [{ key: 'a', label: 'A' }]
  refused('table', { rows: [] }, 'needs 1–10 columns')
  refused('table', { columns: Array.from({ length: 11 }, (_, i) => ({ key: `c${i}`, label: 'x' })), rows: [] }, 'needs 1–10 columns')
  refused('table', { columns: [{ key: 'a', label: 'A' }, { key: 'a', label: 'B' }], rows: [] }, 'column 2 needs a plain, unique key')
  refused('table', { columns: [{ key: 'a', label: 'A', kind: 'money' }], rows: [] }, 'kind is text, number, pill or date')
  refused('table', { columns: cols, rows: Array.from({ length: 201 }, (_, i) => ({ id: i })) }, '200 rows at most')
  refused('table', { columns: cols, rows: [{ id: 1 }, { id: 1 }] }, 'row 2 needs a unique id')
  refused('table', { columns: cols, rows: [{ id: 1, group: 'nope' }] }, "row 1's group “nope” isn't in groups")
  refused('table', { columns: [{ key: 'n', label: 'N', kind: 'number' }], rows: [{ id: 1, cells: { n: 'many' } }] }, "row 1's cell “n” isn't a number")
  refused('table', { columns: [{ key: 'd', label: 'D', kind: 'date' }], rows: [{ id: 1, cells: { d: '15/11/2026' } }] }, "row 1's cell “d” isn't a date (YYYY-MM-DD)")
  refused('table', { columns: cols, rows: [{ id: 1, cells: { a: { text: 'x', tone: 'red' } } }] }, 'tone is good, warn, bad or muted')
  refused('table', { columns: cols, rows: [{ id: 1, detail: Array(7).fill({ text: 'x' }) }] }, "row 1's detail is a list of 6 lines at most")
})

test("a stats item's link: Details › (echo:// in Echo, https asks first, anything else none)", () => {
  const h = draw('stats', { items: [{ label: 'critical', value: 4, link: 'echo://thread/acme' }, { label: 'high', value: 8, link: 'https://staffing.acme.example/?u=high' }, { label: 'x', value: 1, link: 'javascript:x' }] }, 'M')
  assert.match(h, /<a class="link-btn" href="echo:\/\/thread\/acme">Details ›<\/a>/)
  assert.match(h, /data-pw-open="https:\/\/staffing\.acme\.example\/\?u=high">Details ›/)
  assert.equal((h.match(/Details ›/g) || []).length, 2)
})

let failed = 0
for (const [name, f] of tests) {
  try { f(); console.log(`ok - ${name}`) } catch (e) { failed++; console.log(`not ok - ${name}\n  ${e.message}`) }
}
if (failed) process.exit(1)
