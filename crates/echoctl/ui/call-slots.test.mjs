// Plugins on a call's page (app.js): the call's facts they get (slotCallPage); their boxes on its header (callHeadBoxes,
// callSlotAfter: each drawn on its own, one that throws left out, after once per box drawn afresh); their lines in the
// minutes box only while the minutes are coming (minutesComing, callMinutesLines); the call's ⋯ menu (slotItems: a label
// as a function of the call, a separator before each plugin's group, static labels as before); the one plugin sheet
// (sheetOpen, sheetClose, sheetTab: one at a time, onClose once, quiet when its plugin unloads, Tab round inside); and
// a plugin activity's badge and editing guard (actBadgeText, actAsk, actHeld, actBadgeHtml); and ctx.where()'s
// `recall`. Run by scripts/check-mod.sh (`node crates/echoctl/ui/call-slots.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const block = (name) => {
  const a = src.indexOf(`// ── ${name}:`), b = src.indexOf(`// end of the ${name} block`, a)
  assert.ok(a >= 0 && b > a, `the ${name} block not found in app.js`)
  return src.slice(a, b)
}
for (const s of ['call.header', 'call.minutes', 'call.menu']) assert.ok(new RegExp(`const SLOTS = \\[[^\\]]*'${s.replace('.', '\\.')}'`).test(src), `${s} is one of SLOTS`)
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const Plugins = { drawing: false, slots: {}, targets: {} }
let renders = 0
const ctxRender = () => { if (!Plugins.drawing) renders++ }
const page = new Function('Plugins', 'esc', `${block('thread slot')}\n${block('call page slots')}\nreturn { slotCallPage, callHeadBoxes, callSlotAfter, minutesComing, callMinutesLines }`)(Plugins, esc)

// The call's facts: only what the page has; its threads and the first thread's client.
const c = {
  name: '2026-11-06_100000-acme-weekly-sync', title: 'Acme weekly sync', started_at: '2026-11-06T10:00:00Z', notes: 'x', memo: false,
  live: { profile: 'work' }, projects: [{ id: 'internal', client: null }, { id: 'acme-rollout', client: 'acme' }], segments: [1, 2], people: ['Dana'],
}
const call = page.slotCallPage(c)
assert.deepEqual(call, { name: c.name, title: 'Acme weekly sync', started_at: '2026-11-06T10:00:00Z', notes: true, profile: 'work', memo: false, threads: ['internal', 'acme-rollout'], client: 'acme' })
assert.deepEqual(page.slotCallPage({ name: 'm', title: 'Memo', memo: true }), { name: 'm', title: 'Memo', started_at: null, notes: false, profile: null, memo: true, threads: [], client: null }, 'a memo in no thread')

// Header boxes: each its own, the one that throws and a duplicate left out and logged, a ctx.render() in render ignored.
const logged = []
const head = [
  { plugin: 'vault', id: 'chip', label: 'Vault', render: (k) => { ctxRender(); return `<button class="btn">${k.notes ? 'In vault' : 'Not yet'}</button>` } },
  { plugin: 'boom', id: 'x', label: 'Broken', render: () => { throw new Error('nope') } },
  { plugin: 'vault', id: 'chip', label: 'Again', render: () => 'twice' },
]
assert.equal(page.callHeadBoxes(head, call, (m) => logged.push(m)), '<span class="plug-head" data-call-slot="vault:chip"><button class="btn">In vault</button></span>')
assert.deepEqual(logged, ['boom: Broken: nope', 'vault: Again: a second row with the id “chip” — left out'])
assert.equal(renders, 0, 'a ctx.render() asked for while a box draws is ignored')
assert.equal(page.callHeadBoxes(undefined, call, () => {}), '', 'no plugin: nothing on the header')

// after: once per box drawn afresh, again for another call; one that throws is logged.
const root = { boxes: [], querySelectorAll: () => root.boxes }
const fresh = () => { root.boxes = [{ dataset: { callSlot: 'vault:chip' } }, { dataset: { callSlot: 'boom:x' } }] }
let afters = []
const withAfter = [
  { plugin: 'vault', id: 'chip', render: () => '', after: (box, k) => afters.push([box.dataset.callSlot, k.name]) },
  { plugin: 'boom', id: 'x', label: 'Broken', render: () => '', after: () => { throw new Error('after failed') } },
]
const errs = []
fresh(); page.callSlotAfter(root, withAfter, call, (m) => errs.push(m))
page.callSlotAfter(root, withAfter, call, (m) => errs.push(m))
assert.deepEqual(afters, [['vault:chip', c.name]], 'after once for a box drawn once')
assert.deepEqual(errs, ['boom: Broken: after failed'], 'logged once, not again on the kept box')
page.callSlotAfter(root, withAfter, { ...call, name: 'other' }, () => {})
assert.deepEqual(afters.at(-1), ['vault:chip', 'other'], 'the same box for another call: after again')
fresh(); page.callSlotAfter(root, withAfter, { ...call, name: 'other' }, () => {})
assert.equal(afters.length, 3, 'a box drawn afresh: after again')

// Minutes coming: a run queued or going, or Echo writing them by itself; not off, held, too thin, failed, or no transcript.
const coming = (o) => page.minutesComing({ name: 'k', ...o })
assert.equal(coming({ notes_run: { state: 'queued' } }), true)
assert.equal(coming({ notes_run: { state: 'running' } }), true)
for (const due of ['now', 'at', 'before', 'not_ready']) assert.equal(coming({ auto: { due } }), true, due)
assert.equal(coming({ auto: { due: 'tried', held_by: { pid: 4 } } }), true, 'a Claude Code session writing them')
for (const due of ['off', 'held', 'gate', 'missing', 'error', 'tried']) assert.equal(coming({ auto: { due } }), false, due)
for (const state of ['failed', 'needs_login', 'skipped']) assert.equal(coming({ notes_run: { state }, auto: { due: 'at' } }), false, state)
assert.equal(coming({ status: 'failed', auto: { due: 'not_ready' } }), false, 'no transcript')
assert.equal(coming({ notes: 'x', notes_run: { state: 'done' }, auto: { due: 'at' } }), false, 'written already')

const lines = [
  { plugin: 'vault', id: 'line', label: 'Vault', render: (k) => `Goes to the vault once written: <b>${esc(k.title)}</b>` },
  { plugin: 'quiet', id: 'q', render: () => '  ' },
  { plugin: 'boom', id: 'x', render: () => { throw new Error('nope') } },
]
const pending = { ...c, notes: null, notes_run: null, auto: { due: 'at', at: '2026-11-06T11:00:00Z' } }
assert.equal(page.callMinutesLines(lines, pending, () => {}), '<p class="alt plug-line" data-call-minutes="vault:line">Goes to the vault once written: <b>Acme weekly sync</b></p>', 'one line; the empty one and the one that throws left out')
assert.equal(page.callMinutesLines(lines, { ...pending, auto: { due: 'held' } }, () => {}), '', 'held: not coming by themselves, no line')
assert.equal(page.callMinutesLines(lines, { ...pending, auto: { due: 'off' } }, () => {}), '', 'automatic minutes off: no line')
assert.ok(src.includes("callMinutesLines(Plugins.slots['call.minutes'], c,"), 'the minutes box draws the lines')
assert.ok(src.includes("callHeadBoxes(Plugins.slots['call.header'], slotCallPage(c),") && src.includes("callSlotAfter($('#r-head'), Plugins.slots['call.header'], slotCallPage(S.call),"), 'the header draws the boxes, then their after')

// The call's ⋯ menu: a label as a function of the target, a separator before each plugin's group, static labels kept.
const slot = new Function('Plugins', 'esc', `${block('plugin slot targets')}\nreturn { slotItems, slotClick, slotLabel }`)(Plugins, esc)
Plugins.slots['call.menu'] = [
  { plugin: 'vault', label: (t) => { ctxRender(); return `Open in ${t.call.title === 'Acme weekly sync' ? 'Acme vault' : 'Notes'}` }, run: () => {} },
  { plugin: 'vault', label: 'Copy its link', run: () => {} },
  { plugin: 'boom', label: () => { throw new Error('nope') }, run: () => {} },
  { plugin: 'gh', label: 'Open the <issue>', run: () => {} },
  { plugin: 'gh', label: () => '', run: () => {} },
]
const target = { kind: 'call', call: c }
const sep = '<div class="menu-sep" role="separator"></div>'
const item = (i, l) => `<button class="menu-item" role="menuitem" data-plug-slot="call.menu" data-plug-i="${i}">${l}</button>`
assert.equal(slot.slotItems('call.menu', target, 'menu'), sep + item(0, 'Open in Acme vault') + item(1, 'Copy its link') + sep + item(3, 'Open the &lt;issue&gt;'),
  "each plugin's group after a separator; one that throws or is empty left out, the others in their place")
assert.equal(renders, 0, 'a ctx.render() inside a label is ignored')
assert.equal(slot.slotClick('call.menu', '', 3).item.label, 'Open the <issue>', 'a click runs the item drawn at its index')
assert.equal(slot.slotLabel({ label: (t) => t.call.name }, target), c.name)
Plugins.slots['task.actions'] = [{ plugin: 'gh', label: (t) => `Link ${t.node.id}`, run: () => {} }]
assert.equal(slot.slotItems('task.actions', { kind: 'task', node: { id: 'T001' } }), '<button class="btn" data-plug-slot="task.actions" data-plug-i="0" title="Link T001">Link T001</button>', 'buttons: no separators, a label function too')

// The sheet: one at a time, onClose once however it closes, quiet when its plugin unloads, focus going back to the opener.
const sh = new Function(`${block('plugin sheet')}\nreturn { Sheet, sheetOpen, sheetClose, sheetTab }`)()
const closed = [], slog = []
const opener = { id: 'chip' }
sh.sheetOpen('vault', () => closed.push('vault 1'), opener, (m) => slog.push(m))
assert.equal(sh.Sheet.now.plugin, 'vault')
assert.equal(sh.sheetClose('gh', false, () => {}), null, "another plugin's closeSheet leaves it open")
sh.sheetOpen('gh', () => closed.push('gh'), { id: 'inside the first sheet' }, () => {})
assert.deepEqual(closed, ['vault 1'], 'a second sheet closes the first, its onClose called')
assert.equal(sh.Sheet.now.back, opener, 'the keyboard goes back to where it was before the first sheet')
assert.equal(sh.sheetClose(null, false, () => {}).plugin, 'gh', 'Esc or a click beside it closes the open one')
assert.deepEqual(closed, ['vault 1', 'gh'])
assert.equal(sh.sheetClose(null, false, () => {}), null, 'nothing open: nothing closed, no onClose twice')
sh.sheetOpen('vault', () => closed.push('vault 2'), opener, () => {})
assert.equal(sh.sheetClose('vault', true, () => {}).plugin, 'vault', 'its plugin unloading closes it')
assert.deepEqual(closed, ['vault 1', 'gh'], '… without its onClose')
sh.sheetOpen('boom', () => { throw new Error('bad close') }, opener, (m) => slog.push(m))
sh.sheetClose(null, false, (m) => slog.push(m))
assert.deepEqual(slog, ["boom: its sheet's onClose: bad close"], 'an onClose that throws is logged, the sheet closed')
assert.equal(sh.Sheet.now, null)
assert.deepEqual([sh.sheetTab(3, 0, false), sh.sheetTab(3, 2, false), sh.sheetTab(3, 0, true), sh.sheetTab(3, -1, false), sh.sheetTab(3, -1, true), sh.sheetTab(0, -1, false)], [1, 0, 2, 0, 2, -1], 'Tab goes round inside')
assert.ok(/sheet: \(html, opts\) => \{[\s\S]*?owned\.cleanups\.push\(\(\) => sheetShut\(id, true\)\)/.test(src), 'a sheet goes with its plugin (a cleanup unloadPlugin runs)')
assert.ok(/closeSheet: \(\) => \{ sheetShut\(id\) \}/.test(src), "closeSheet closes only its plugin's")

// A plugin activity's badge and its editing guard.
const act = new Function('Plugins', 'esc', `${block('plugin activity')}\nreturn { actBadgeText, actAsk, actHeld, actBadgeHtml }`)(Plugins, esc)
assert.deepEqual([3, '7', ' 12 ', 100, 2.6, '!', 0, -1, '', null, 'x', '<b>', NaN, Infinity, true, {}].map(act.actBadgeText),
  ['3', '7', '12', '99+', '2', '!', '', '', '', '', '', '', '', '', '', ''], 'counts clamped, ! kept, the rest none')
const alog = []
const kn = { plugin: 'kn', id: 'vault', label: 'Vault', badge: () => { ctxRender(); return 4 } }
assert.equal(act.actBadgeHtml(kn, (m) => alog.push(m)), '<span class="act-badge new" aria-label="4 new">4</span>')
assert.equal(renders, 0, 'a ctx.render() inside badge is ignored')
assert.equal(act.actBadgeHtml({ ...kn, badge: () => '!' }, () => {}), '<span class="act-badge" aria-label="Needs your attention">!</span>')
assert.equal(act.actBadgeHtml({ id: 'today', badge: () => 3 }, () => {}), '', "Echo's own activities: not this way")
assert.equal(act.actBadgeHtml({ plugin: 'kn', id: 'x' }, () => {}), '', 'no badge')
const bad = { plugin: 'kn', id: 'bad', label: 'Bad', badge: () => { throw new Error('nope') }, editing: () => { throw new Error('lost') } }
for (let i = 0; i < 5; i++) { assert.equal(act.actBadgeHtml(bad, (m) => alog.push(m)), ''); assert.equal(act.actHeld(bad, true, (m) => alog.push(m)), false) }
assert.deepEqual(alog, ['kn: Bad: badge: nope', 'kn: Bad: editing: lost'], 'a hook that throws is logged once, not every tick')
let typing = true
const ed = { plugin: 'kn', id: 'ed', editing: () => typing }
assert.equal(act.actHeld(ed, true, () => {}), true, 'drawn and editing: its box left as it is')
assert.equal(act.actHeld(ed, false, () => {}), false, 'not drawn yet (opened afresh): drawn even while editing')
typing = 'yes'
assert.equal(act.actHeld(ed, true, () => {}), false, 'only true holds it')
assert.equal(act.actHeld({ plugin: 'kn', id: 'plain' }, true, () => {}), false, 'no editing(): drawn as patch draws')
assert.ok(/if \(pact && actHeld\(pact, primary\._html !== undefined,[^\n]*\) \{ \/\* kept \*\/ \}\n\s*else if \(pact\) \{[\s\S]{0,400}patch\(primary,/.test(src), "renderNow keeps a held activity's box, else patches it")
assert.ok(src.includes('actBadgeHtml(a, '), 'the activity bar draws the badge')

// ctx.where() tells whether Recall is on.
assert.ok(/where: \(\) => \(\{[^\n]*recall: brainOn\(\)/.test(src), 'where() has recall')
console.log('call-slots: ok')
