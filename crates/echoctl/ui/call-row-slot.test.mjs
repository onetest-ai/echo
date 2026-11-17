// The plugins' chips on a call's row in the calls list (app.js, callRowChips): each item's chip(call) drawn for the
// call, text only and cut at 24 characters; nothing or blank leaves no chip; one that throws left out and logged while
// the others draw; a duplicate id left out; a ctx.render() inside chip ignored; drawn in the row through esc. Run by
// scripts/check-mod.sh (`node crates/echoctl/ui/call-row-slot.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── call row slot:'), b = src.indexOf('// end of the call row slot block', a)
assert.ok(a >= 0 && b > a, 'the call row slot block not found in app.js')
const Plugins = { drawing: false, slots: {} }
let renders = 0
const ctxRender = () => { if (!Plugins.drawing) renders++ }
const { chips, slotCall } = new Function('Plugins', `${src.slice(a, b)}\nreturn { chips: callRowChips, slotCall }`)(Plugins)
assert.ok(src.includes("'call.row']"), 'call.row is one of SLOTS')

const call = slotCall({ name: '2026-10-28_110000-aurora', title: 'Aurora weekly', started_at: '2026-10-28T11:00:00+02:00', status: 'notes', memo: false, people: ['Lena'], chat: true })
assert.deepEqual(call, { name: '2026-10-28_110000-aurora', title: 'Aurora weekly', started_at: '2026-10-28T11:00:00+02:00', status: 'notes', memo: false }, "the row's own facts, nothing more")
const logged = []
const items = [
  { plugin: 'vault', id: 'in', label: 'In the vault', chip: (c) => { ctxRender(); return c.status === 'notes' ? 'In vault ✓' : null } },
  { plugin: 'boom', id: 'x', label: 'Broken', chip: () => { throw new Error('nope') } },
  { plugin: 'quiet', id: 'q', label: 'Quiet', chip: () => '   ' },
  { plugin: 'long', id: 'l', label: 'Long', chip: () => 'A chip far longer than a row can hold' },
  { plugin: 'vault', id: 'in', label: 'Again', chip: () => 'twice' },
  { plugin: 'html', id: 'h', chip: () => '<img src=x onerror=alert(1)>' },
]
const got = chips(items, call, (m) => logged.push(m))
assert.deepEqual(got, [
  { key: 'vault:in', text: 'In vault ✓', title: 'In the vault' },
  { key: 'long:l', text: 'A chip far longer than …', title: 'Long' },
  { key: 'html:h', text: '<img src=x onerror=aler…', title: 'h' },
])
assert.deepEqual(logged, ['boom: Broken: nope', 'vault: Again: a second chip with the id “in” — left out'], 'the one that threw and the duplicate logged, the rest drawn')
assert.equal(renders, 0, 'a ctx.render() asked for while a chip draws is ignored')
assert.equal(Plugins.drawing, false, 'drawing is over, even after a throw')
assert.deepEqual(chips(undefined, call, () => {}), [], 'no plugin chips: none')

// The row draws each chip's text and label through esc (window-ui R1).
const shell = src.slice(src.indexOf('function list()'), src.indexOf('/** In a narrow window the list folds away'))
assert.match(shell, /callRowChips\(Plugins\.slots\['call\.row'\], slotCall\(c\)/, 'the calls list draws the call.row chips')
assert.match(shell, /title="\$\{esc\(x\.title\)\}">\$\{esc\(x\.text\)\}<\/span>/, "a chip's label and text go through esc")
// A plugin unloaded takes its chips with it: its slot items are filtered out by unloadPlugin, whatever the slot.
assert.match(src, /for \(const k of Object\.keys\(Plugins\.slots\)\) Plugins\.slots\[k\] = Plugins\.slots\[k\]\.filter\(\(x\) => x\.plugin !== id\)/)
console.log('call-row-slot: ok')
