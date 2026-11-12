// The plugins' rows of a thread's Profile (app.js, threadSlotRows, threadSlotAfter): each drawn for the thread, one that
// throws left out and logged while the others draw, a duplicate id left out; a ctx.render() inside render ignored; after
// called once per box drawn afresh; a plugin unloaded takes its row with it (unloadPlugin). Run by scripts/check-mod.sh
// (`node crates/echoctl/ui/thread-slot.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── thread slot:'), b = src.indexOf('// end of the thread slot block', a)
assert.ok(a >= 0 && b > a, 'the thread slot block not found in app.js')
const Plugins = { drawing: false, slots: {} }
let renders = 0
// Echo's render and a plugin's ctx.render, as pluginCtx makes it.
const ctxRender = () => { if (!Plugins.drawing) renders++ }
const { rows, after } = new Function('Plugins', `${src.slice(a, b)}\nreturn { rows: threadSlotRows, after: threadSlotAfter }`)(Plugins)
assert.ok(src.includes("'thread.profile']"), 'thread.profile is one of SLOTS')

const thread = { id: 'alpha', name: 'Alpha', client: 'acme' }
const logged = []
const items = [
  { plugin: 'gh', id: 'repos', label: 'Repositories', render: (t) => { ctxRender(); return `<b>${t.id}</b> of ${t.client}` } },
  { plugin: 'boom', id: 'x', label: 'Broken', render: () => { throw new Error('nope') } },
  { plugin: 'jira', id: 'project', label: 'Jira', render: () => undefined },
  { plugin: 'gh', id: 'repos', label: 'Again', render: () => 'twice' },
]
const got = rows(items, thread, (m) => logged.push(m))
assert.deepEqual(got.map((r) => [r.key, r.label, r.html]), [['gh:repos', 'Repositories', '<b>alpha</b> of acme'], ['jira:project', 'Jira', '']])
assert.deepEqual(logged, ['boom: Broken: nope', 'gh: Again: a second row with the id “repos” — left out'], 'the one that threw and the duplicate logged, the rest drawn')
assert.equal(renders, 0, 'a ctx.render() asked for while a row draws is ignored')
assert.equal(Plugins.drawing, false, 'drawing is over, even after a throw')
assert.deepEqual(rows(undefined, thread, () => {}), [], 'no plugin rows: none')

// after: once per box drawn afresh. A fake page whose boxes are made again when its HTML changes (as patch does).
const page = { html: '', boxes: [], querySelectorAll: () => page.boxes }
const draw = (html) => { if (page.html === html) return; page.html = html; page.boxes = ['gh:repos', 'jira:project'].map((k) => ({ dataset: { threadSlot: k } })) }
let calls = 0, cached = null
const slot = [{ plugin: 'gh', id: 'repos', render: () => cached || 'loading', after: () => { calls++; cached = 'repos: 2'; redraw() } }]
// Echo's render(): the page drawn from each row's render, then their after — as drawProjects does.
let depth = 0
function redraw() {
  if (++depth > 20) throw new Error('drawn again forever')
  draw(rows(slot, thread, () => {}).map((r) => r.html).join('|'))
  after(page, slot, thread, (m) => logged.push(m))
  depth--
}
redraw()
assert.equal(calls, 2, 'after on the first box, then once on the box drawn afresh with its data — not again')
const boxes = page.boxes
redraw(); redraw()
assert.equal(calls, 2, 'the page as it was: its boxes kept, after not run again')
assert.equal(page.boxes, boxes)
let forBeta = 0
after(page, [{ ...slot[0], after: () => forBeta++ }], { ...thread, id: 'beta' }, () => {})
assert.equal(forBeta, 1, 'another thread in the same box: after runs for it')

// A plugin switched off: its thread.profile row goes with it.
const u = src.indexOf('function unloadPlugin('), v = src.indexOf('\n}\n', u)
const P = { slots: { 'thread.profile': [{ plugin: 'gh', id: 'repos' }, { plugin: 'jira', id: 'project' }] }, owned: {}, palette: [], sections: [], settings: {}, widgetDraw: {}, loaded: { gh: '1' }, applied: { gh: true } }
const unload = new Function('Plugins', 'ACTIVITIES', 'PANEL_VIEWS', 'S', 'go', 'setTimeout', `${src.slice(u, v + 3)}\nreturn unloadPlugin`)(P, [], [], {}, () => {}, () => {})
unload('gh')
assert.deepEqual(P.slots['thread.profile'], [{ plugin: 'jira', id: 'project' }], 'gh\'s row gone, jira\'s kept')
console.log('thread-slot: ok')
