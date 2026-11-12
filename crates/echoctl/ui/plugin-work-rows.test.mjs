// A plugin's work row (app.js, the plugin work rows block): its plugin's icon only as an image (or its initial), "From
// <plugin>", its text escaped, its title opening only an https page and only once the user confirms it (its host
// asked first, as a plugin widget's link), no Echo actions, "not updated" when its plugin's last ask failed; the
// plugin slot targets (only items that ask for a plugin row get it, a click goes to its own row); a thread's Work tab
// shows the rows in every layout. Run by scripts/check-mod.sh (`node crates/echoctl/ui/plugin-work-rows.test.mjs`).
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── plugin work rows:'), b = src.indexOf('// end of the plugin work rows block', a)
assert.ok(a >= 0 && b > a, 'the plugin work rows block not found in app.js')
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const { workRowHtml: row, linkAskStep, linkAsked } = new Function('esc', `${src.slice(a, b)}\nreturn { workRowHtml, linkAskStep, linkAsked }`)(esc)

const r = { id: 'pr-7', title: 'Review: fix login', kind: 'review', where: 'acme-test/api', url: 'https://git.acme.example/pr/7', source: 'gh', source_name: 'GitHub', icon: 'plugin/gh/icon.svg' }
const h = row(r, '<i>due</i>', '')
assert.match(h, /<img class="wr-ic" src="plugin\/gh\/icon.svg" alt="From GitHub" title="From GitHub"/, 'its icon as an image, never inlined')
assert.match(h, /data-open-url="https:\/\/git.acme.example\/pr\/7"/, 'its title opens its page')
assert.match(h, /review · acme-test\/api/)
assert.ok(h.startsWith('<div class="td-task td-plug"') && !/data-tk=|data-today-close|data-claude-task/.test(h), "no Echo task's open, Close or Claude")
assert.ok(!h.includes('pw-ask') && !h.includes('not updated'), 'nothing asked, up to date')

const evil = row({ ...r, title: '<img src=x onerror=alert(1)>', kind: '<b>', where: '"q"', url: 'javascript:alert(1)', icon: null, source_name: '<s>', stale: '<i>down</i>' }, '', '')
assert.ok(!/<img src=x|<b>|<s>|<i>down/.test(evil), 'text escaped')
assert.ok(!evil.includes('data-open-url') && !evil.includes('javascript:'), 'only an https page opens')
assert.match(evil, /class="wr-ic wr-letter" title="From &lt;s&gt;"/, 'no icon: its initial')
assert.match(evil, /<span class="wr-stale" title="Not updated: &lt;i&gt;down&lt;\/i&gt;">not updated<\/span>/, 'its last ask failed: marked, with why')
assert.ok(row({ ...r, url: 'http://acme.example' }, '', '').indexOf('data-open-url') < 0, 'plain http does not open')
assert.match(row(r, '', '<button>Act</button>'), /<span class="td-icos"><button>Act<\/button><\/span>/, "its plugins' actions")

// Its link: the first click asks (its host named), a second within 30 s opens; later, or another link, asks again.
const st = {}
assert.equal(linkAskStep(st, 'wrOpen', r.url, 1000), false, 'first click: asked')
const asked = row(r, '', '', linkAsked(st, 'wrOpen', 2000))
assert.match(asked, /Open git\.acme\.example in the browser\? Click its title again · <button type="button" class="link-btn" data-open-url-no="1">Cancel<\/button>/)
assert.ok(!row({ ...r, id: 'x', url: 'https://other.example/x' }, '', '', st.wrOpen).includes('pw-ask'), 'only its own row asks')
assert.equal(linkAskStep(st, 'wrOpen', r.url, 5000), true, 'second click: opened')
assert.equal(st.wrOpen, null, 'and the question goes')
linkAskStep(st, 'wrOpen', r.url, 0)
assert.equal(linkAskStep(st, 'wrOpen', r.url, 31000), false, 'after 30 s it asks again')
assert.equal(linkAskStep(st, 'wrOpen', 'https://other.example/x', 32000), false, 'another link asks for itself')
assert.equal(linkAsked({ wrOpen: r.url, wrOpenAt: 0 }, 'wrOpen', 30001), null, 'a lapsed question is not drawn')

// The window's half around it (13-today-work.js): a title's click and its ⌘K entry ask, then open — never at once.
const part = readFileSync(new URL('./app/13-today-work.js', import.meta.url), 'utf8')
const sent = [], S = { view: 'today' }
let focused = null
const doc = { querySelector: (q) => ({ focus: () => { focused = q } }) }
const win = new Function('esc', 'S', 'send', 'render', 'document', 'CSS', 'setTimeout', 'slotItems', 'shortDay', `${part}\nreturn { workRowOpen, threadWorkRows }`)(
  esc, S, (m) => sent.push(m), () => {}, doc, { escape: (x) => x }, (f) => f(), () => '', (d) => d.slice(0, 10))
win.workRowOpen(r.url)
assert.deepEqual(sent, [], 'not opened on the first click')
assert.match(win.threadWorkRows({ plugin_rows: [r] }), /Open git\.acme\.example in the browser\?/, "the thread's Work row asks too")
assert.ok(focused && focused.includes(r.url), 'the keyboard stays on its title')
win.workRowOpen(r.url)
assert.deepEqual(sent, [{ cmd: 'open_url', url: r.url }], 'opened on the second')
const click = src.split('\n').find((l) => l.includes("sel: '[data-open-url]'"))
assert.ok(click && click.includes('workRowOpen(t.dataset.openUrl)'), "the title's click goes through the question")
assert.ok(src.includes("press({ openUrl: r.url })"), "⌘K's Open runs as the title's click (asked first)")

// The plugin slot targets: an item without `when` stays on Echo's tasks; keyed targets route a click to its own row.
const c = src.indexOf('// ── plugin slot targets:'), d = src.indexOf('// end of the plugin slot targets block', c)
assert.ok(c >= 0 && d > c, 'the plugin slot targets block not found in app.js')
const Plugins = { slots: {}, targets: {} }
const slot = new Function('Plugins', 'esc', `${src.slice(c, d)}\nreturn { slotFor, slotItems, slotTarget, slotForget }`)(Plugins, esc)
const ran = []
Plugins.slots['task.actions'] = [
  { plugin: 'other', label: 'Echo only', run: () => ran.push('echo') },
  { plugin: 'gh', label: 'Snooze', when: (t) => t.kind === 'plugin' && t.source === 'gh', run: (t) => ran.push(t.row.id) },
  { plugin: 'bad', label: 'Throws', when: () => { throw new Error('x') }, run: () => {} },
]
const row7 = { kind: 'plugin', source: 'gh', row: { id: 'pr-7' } }, row8 = { kind: 'plugin', source: 'gh', row: { id: 'pr-8' } }
assert.deepEqual(slot.slotFor('task.actions', row7).map((x) => x.label), ['Snooze'], 'no `when`: not on a plugin row; a throwing `when`: left out')
assert.deepEqual(slot.slotFor('task.actions', { kind: 'task', id: 't1' }).map((x) => x.label), ['Echo only'], "Echo's task: only the items written for it")
assert.deepEqual(slot.slotFor('task.actions', { kind: 'plugin', source: 'jira', row: {} }), [], "another plugin's row: none")
const b7 = slot.slotItems('task.actions', row7, 'buttons', 'gh:pr-7'), b8 = slot.slotItems('task.actions', row8, 'buttons', 'gh:pr-8')
assert.match(b7, /data-plug-key="gh:pr-7"/)
for (const [html, want] of [[b8, 'pr-8'], [b7, 'pr-7']]) {
  const key = /data-plug-key="([^"]+)"/.exec(html)[1], i = +/data-plug-i="(\d+)"/.exec(html)[1]
  const t = slot.slotTarget('task.actions', key)
  slot.slotFor('task.actions', t)[i].run(t)
  assert.equal(ran.at(-1), want, 'a click goes to the row it was drawn for')
}
Plugins.targets['task.actions'] = { kind: 'task' }
Plugins.targets['task.actions#jira:1'] = row8
slot.slotForget(Plugins.targets, 'gh')
assert.deepEqual(Object.keys(Plugins.targets).sort(), ['task.actions', 'task.actions#jira:1'], "a plugin unloaded: only its rows' targets go")

// A thread's Work tab: the plugins' rows in every layout — no work yet, Board and Tree.
const pw = src.indexOf('function projectWork(p) {'), pwEnd = src.indexOf('\n}\n', pw)
const deps = ['S', 'esc', 'workToReview', 'hm', 'wv', 'workOwners', 'viewTabs', 'boardChips', 'workFilter', 'threadWorkRows', 'workBoard', 'workRow', 'KIND_FILTERS']
const work = (layout, nodes) => new Function(...deps, `${src.slice(pw, pwEnd + 2)}\nreturn projectWork`)(
  { pj: { work: { nodes, roots: Object.keys(nodes), plugin_rows: [r] } } }, esc, () => [], () => '', () => ({ layout }), () => [], () => '', () => '',
  () => ({ pass: () => true, board: {} }), (w) => w.plugin_rows.length ? '<section class="wk-plug">plugin rows</section>' : '', () => '<div class="board"></div>', () => '<div class="wk-row"></div>', [['', 'All kinds']])({})
const story = { s1: { id: 's1', kind: 'story', title: 'Story', children: [] } }
for (const [name, html] of [['no work', work('tree', {})], ['board', work('board', story)], ['tree', work('tree', story)]]) {
  assert.ok(html.includes('plugin rows'), `${name}: the plugins' rows shown`)
}
console.log('plugin-work-rows: ok')
