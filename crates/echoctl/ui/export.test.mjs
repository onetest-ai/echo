// Export as HTML (app.js, the export file block): the file a dashboard is saved as — Echo's head, the widgets' markup,
// Echo's styles, no script and nothing from outside; each widget's data as tables, every text escaped. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/export.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── export file:'), b = src.indexOf('// end of the export file block', a)
assert.ok(a >= 0 && b > a, 'the export file block not found in app.js')
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const { exportHtml, exportPluginTables, exportCoreTables } = new Function('esc', `${src.slice(a, b)}\nreturn { exportHtml, exportPluginTables, exportCoreTables }`)(esc)
const tests = []
const test = (name, f) => tests.push([name, f])

test('the file: its title, when, the widgets, Echo\'s styles; a CSP that lets no script or outside request in', () => {
  const h = exportHtml({ title: 'Acme · Overview', at: 'Snapshot of Saturday', css: ':root { --accent: #8b62ff; }', body: '<section class="td-sec">W</section>', data: [] })
  assert.match(h, /^<!doctype html>/)
  assert.match(h, /<title>Acme · Overview — Echo<\/title>/)
  assert.match(h, /<h1>Acme · Overview<\/h1><span class="set-sub">Snapshot of Saturday<\/span>/)
  assert.match(h, /<style>:root \{ --accent: #8b62ff; \}<\/style>/)
  assert.match(h, /<div class="ex-view"><section class="td-sec">W<\/section><\/div>/)
  assert.match(h, /content="default-src 'none'; style-src 'unsafe-inline'; img-src data:"/)
  assert.doesNotMatch(h, /<script/i)
  assert.match(h, /No widgets on this page\./)
})

test('a title, a widget\'s label and its cells are escaped; styles can\'t close their own tag', () => {
  const h = exportHtml({ title: '<b>x</b>', at: 'now', css: 'a{}</style><script>alert(1)</script>', body: '', data: [{ label: '<img src=x onerror=1>', tables: [{ cols: ['<th>'], rows: [['<script>alert(1)</script>']] }] }] })
  assert.doesNotMatch(h, /<script>/)
  assert.match(h, /<style>a\{\}\\3c \/style>\\3c script>alert\(1\)\\3c \/script><\/style>/)
  assert.match(h, /<h1>&lt;b&gt;x&lt;\/b&gt;<\/h1>/)
  assert.match(h, /<h3>&lt;img src=x onerror=1&gt;<\/h3>/)
  assert.match(h, /<th scope="col">&lt;th&gt;<\/th>/)
  assert.match(h, /<td>&lt;script&gt;alert\(1\)&lt;\/script&gt;<\/td>/)
})

test('a key / value table has no head row, its keys as row heads; an empty one says so; a page widget: drawn above', () => {
  const h = exportHtml({ title: 't', at: '', css: '', body: '', data: [{ label: 'Open', tables: [{ cols: ['', ''], rows: [['Value', '42']] }, { cols: ['A'], rows: [] }] }, { label: 'Own page', tables: [] }] })
  assert.match(h, /<table class="ex-t"><tbody><tr><th scope="row">Value<\/th><td>42<\/td><\/tr><\/tbody><\/table>/)
  assert.match(h, /Nothing in it\./)
  assert.match(h, /<h3>Own page<\/h3><p class="set-sub">Drawn above; no data to list\.<\/p>/)
})

test('a plugin table: its columns\' labels, a row a row, its group named; cells by text ({text, tone} too)', () => {
  const [t] = exportPluginTables('table', {
    columns: [{ key: 'role', label: 'Role' }, { key: 'n', label: 'Open', kind: 'number' }, { key: 'st', label: 'State', kind: 'pill' }],
    groups: [{ id: 'eng', label: 'Engineering' }],
    rows: [{ id: '1', group: 'eng', cells: { role: 'Backend', n: 2, st: { text: 'hiring', tone: 'good' } } }, { id: '2', cells: { role: 'Designer' } }],
  })
  assert.deepEqual(t.cols, ['Group', 'Role', 'Open', 'State'])
  assert.deepEqual(t.rows, [['Engineering', 'Backend', '2', 'hiring'], ['', 'Designer', '', '']])
})

test('a chart: its points as rows, a column a series (with the unit); its headline above', () => {
  const [head, t] = exportPluginTables('chart', { type: 'bar', unit: 'h', headline: { value: '42', label: 'This week', change: { by: 5, than: 'last week' } }, series: [{ key: 'a', label: 'Billable' }, { key: 'b', label: 'Other' }], points: [{ label: 'Mon', values: { a: 3, b: 1 } }, { label: 'Tue', values: { a: 4 } }] })
  assert.deepEqual(head.rows, [['This week', '42'], ['Change', '5 vs last week']])
  assert.deepEqual(t.cols, ['', 'Billable (h)', 'Other (h)'])
  assert.deepEqual(t.rows, [['Mon', '3', '1'], ['Tue', '4', '']])
  const [one] = exportPluginTables('chart', { type: 'pie', points: [{ label: 'A', value: 1 }] })
  assert.deepEqual(one, { cols: ['', 'Value'], rows: [['A', '1']] })
})

test('stat, stats, list, status and card as rows; a kind Echo doesn\'t know: none', () => {
  assert.deepEqual(exportPluginTables('stat', { value: 7, unit: 'days', label: 'to go-live', progress: { used: 3, of: 10 } })[0].rows, [['Value', '7 days'], ['What', 'to go-live'], ['Progress', '3 of 10']])
  assert.deepEqual(exportPluginTables('stats', { items: [{ label: 'Open', value: 4, trend: [1, 2] }] })[0].rows, [['Open', '4', '1, 2']])
  assert.deepEqual(exportPluginTables('list', { items: [{ title: 'One', sub: 'first' }, { title: 'Two' }] })[0].rows, [['One', 'first'], ['Two', '']])
  assert.deepEqual(exportPluginTables('status', { state: 'warn', title: 'Sync', text: 'late' })[0].rows, [['State', 'warn'], ['Title', 'Sync'], ['Text', 'late']])
  assert.deepEqual(exportPluginTables('card', { title: 'Note' })[0].rows, [['Title', 'Note']])
  assert.deepEqual(exportPluginTables('nope', {}), [])
})

test('Echo\'s own: Your day a line a meeting or call, My work a task with whose and its thread, glance and calls in numbers', () => {
  const [day] = exportCoreTables('core:calendar.agenda', { day: [{ start: '2026-11-14T10:00:00', end: '2026-11-14T10:30:00', subject: 'Acme weekly sync', kind: 'meeting', response: 'tentativelyAccepted', threads: [{ name: 'Acme rollout' }], minutes: { state: 'ready' } }] })
  assert.deepEqual(day.cols, ['Time', 'What', 'Kind', 'Threads', 'Answer', 'Minutes'])
  assert.equal(day.rows[0][1], 'Acme weekly sync')
  assert.deepEqual(day.rows[0].slice(2), ['meeting', 'Acme rollout', 'maybe', 'ready'])
  assert.match(day.rows[0][0], /10.*–.*10/)
  const [work] = exportCoreTables('core:tasks.mine', { mine: { items: [{ title: 'Send Acme the SSO checklist', project: 'acme-rollout', due: '2026-11-14' }] }, owed: { items: [{ title: 'Answer the security questionnaire', project: 'acme-rollout', owner: 'Omar Haddad', due: '2026-11-10', late: true }] } }, { 'acme-rollout': 'Acme rollout' })
  assert.deepEqual(work.rows, [['Send Acme the SSO checklist', 'I owe', 'Acme rollout', '2026-11-14'], ['Answer the security questionnaire', 'owed to me · Omar Haddad', 'Acme rollout', 'late (2026-11-10)']])
  assert.deepEqual(exportCoreTables('core:tasks.glance', { threads: [{ name: 'Acme rollout', done: 1, todo: 2, waiting: 0, total: 3 }] })[0].rows, [['Acme rollout', '1', '2', '0', '3']])
  const [n, days] = exportCoreTables('core:calls.week', { calls: 3, memos: 1, recorded_min: 95, with_minutes: 2, days: [{ date: '2026-11-10', calls: 2 }] })
  assert.deepEqual(n.rows, [['Calls', '3'], ['Memos among them', '1'], ['Minutes recorded', '95'], ['With minutes', '2']])
  assert.deepEqual(days.rows, [['2026-11-10', '2']])
  assert.deepEqual(exportCoreTables('core:calendar.next', { next: null }), [])
  assert.deepEqual(exportCoreTables('core:something.new', {}), [])
})

for (const [name, f] of tests) {
  try { f(); console.log(`ok   ${name}`) } catch (e) { console.error(`FAIL ${name}\n${e.stack}`); process.exitCode = 1 }
}
