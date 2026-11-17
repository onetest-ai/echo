// The plugins' rows of a client's Profile (`client.profile`; app.js, threadSlotRows, threadSlotAfter — the helpers
// thread.profile draws with): each drawn for the client {id, name}, one that throws left out and logged while the others
// draw, a duplicate id left out; after called once per box drawn afresh, again for another client; a plugin unloaded
// takes its row with it (unloadPlugin). Run by scripts/check-mod.sh (`node crates/echoctl/ui/client-slot.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const a = src.indexOf('// ── thread slot:'), b = src.indexOf('// end of the thread slot block', a)
assert.ok(a >= 0 && b > a, 'the thread slot block not found in app.js')
assert.ok(/const SLOTS = \[[^\]]*'client\.profile'/.test(src), 'client.profile is one of SLOTS')
const c = src.indexOf('const slotClient = '), d = src.indexOf('\n', c)
assert.ok(c >= 0, 'slotClient not found')
const Plugins = { drawing: false, slots: {} }
const { rows, after, slotClient } = new Function('Plugins', `${src.slice(a, b)}\n${src.slice(c, d)}\nreturn { rows: threadSlotRows, after: threadSlotAfter, slotClient }`)(Plugins)

// The client a row sees: its id and name, nothing more of Echo's.
const client = slotClient({ id: 'acme', name: 'Acme', domains: ['acme.example'], aliases: ['ACME'], internal: false })
assert.deepEqual(client, { id: 'acme', name: 'Acme' })

const logged = []
const items = [
  { plugin: 'kb', id: 'vault', label: 'Vault', render: (x) => `<b>${x.name}</b> → vault` },
  { plugin: 'boom', id: 'x', label: 'Broken', render: () => { throw new Error('nope') } },
  { plugin: 'crm', id: 'deal', label: 'Deal', render: () => null },
  { plugin: 'kb', id: 'vault', label: 'Again', render: () => 'twice' },
]
const got = rows(items, client, (m) => logged.push(m))
assert.deepEqual(got.map((r) => [r.key, r.label, r.html]), [['kb:vault', 'Vault', '<b>Acme</b> → vault'], ['crm:deal', 'Deal', '']])
assert.deepEqual(logged, ['boom: Broken: nope', 'kb: Again: a second row with the id “vault” — left out'])
assert.equal(Plugins.drawing, false, 'drawing is over, even after a throw')

// after: once per box drawn afresh, for the client it is drawn for.
const box = { dataset: { profileSlot: 'kb:vault' } }
const page = { querySelectorAll: (sel) => (sel === '[data-profile-slot]' ? [box] : []) }
const seen = []
const slot = [{ plugin: 'kb', id: 'vault', render: () => '', after: (bx, x) => seen.push([bx, x.id]) }]
after(page, slot, client, () => {})
after(page, slot, client, () => {})
assert.deepEqual(seen, [[box, 'acme']], 'once for the same box and client')
after(page, slot, { id: 'globex', name: 'Globex' }, () => {})
assert.deepEqual(seen.map((s) => s[1]), ['acme', 'globex'], 'another client in the same box: after runs for it')
const throwing = [{ plugin: 'kb', id: 'vault', label: 'Vault', after: () => { throw new Error('later') } }]
const err = []
after(page, throwing, { id: 'initech', name: 'Initech' }, (m) => err.push(m))
assert.deepEqual(err, ['kb: Vault: later'], 'an after that throws is logged')

// A plugin switched off: its client.profile row goes with it.
const u = src.indexOf('function unloadPlugin('), v = src.indexOf('\n}\n', u)
const P = { slots: { 'client.profile': [{ plugin: 'kb', id: 'vault' }, { plugin: 'crm', id: 'deal' }] }, targets: {}, owned: {}, palette: [], sections: [], settings: {}, widgetDraw: {}, loaded: { kb: '1' }, applied: { kb: true }, drops: {}, names: { kb: 'Knowledge' } }
const f = src.indexOf('function slotForget('), g = src.indexOf('\n}\n', f)
const unload = new Function('Plugins', 'ACTIVITIES', 'PANEL_VIEWS', 'S', 'go', 'setTimeout', 'followDrops', 'micClose', `${f >= 0 ? src.slice(f, g + 3) : ''}\n${src.slice(u, v + 3)}\nreturn unloadPlugin`)(P, [], [], {}, () => {}, () => {}, () => {}, () => {})
unload('kb')
assert.deepEqual(P.slots['client.profile'], [{ plugin: 'crm', id: 'deal' }], 'kb\'s row gone, crm\'s kept')
console.log('client-slot: ok')
