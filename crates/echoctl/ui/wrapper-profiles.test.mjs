// AI & minutes' wrapper profiles: what plugins offer, a profile's update from its plugin, the last check, add or update
// (app.js, the wrapper-profiles block). Run by scripts/check-mod.sh (`node crates/echoctl/ui/wrapper-profiles.test.mjs`).
import assert from 'node:assert/strict'
import { appSource } from './source.mjs'

const src = appSource()
const start = src.indexOf('// ── wrapper-profiles:')
const end = src.indexOf('// end of the wrapper-profiles block', start)
assert.ok(start >= 0 && end > start, 'the wrapper-profiles block not found in app.js')
const { wrapperOffers, wrapperUpdate, wrapperState, WRAPPER_WARNING } = new Function(`${src.slice(start, end)}\nreturn { wrapperOffers, wrapperUpdate, wrapperState, WRAPPER_WARNING }`)()

const tests = []
const test = (name, f) => tests.push([name, f])
const offer = (name, plugin_name, profile = null, update = null) => ({ name, plugin_name, plugin: plugin_name.toLowerCase().replace(/ /g, '-'), command: 'acme-claude', profile, update })

test('offered: the wrappers not added yet, by name, then plugin', () => {
  const a = { wrapper_offers: [offer('Acme Claude', 'Acme Tools'), offer('Acme Claude', 'Acme Admin'), offer('Globex Claude', 'Globex', 'globex'), offer('Aardvark', 'Zoo')] }
  assert.deepEqual(wrapperOffers(a).map((o) => `${o.name}/${o.plugin_name}`), ['Aardvark/Zoo', 'Acme Claude/Acme Admin', 'Acme Claude/Acme Tools'])
  assert.deepEqual(wrapperOffers(undefined), [])
  assert.deepEqual(wrapperOffers({}), [])
})

test("a profile's plugin offering another program: said on that profile, until taken", () => {
  const a = { wrapper_offers: [offer('Acme Claude', 'Acme Tools', 'acme', '/opt/acme/bin/acme-claude'), offer('Globex', 'Globex', 'globex')] }
  assert.deepEqual(wrapperUpdate(a, 'acme'), { plugin_name: 'Acme Tools', command: 'acme-claude', from: '/opt/acme/bin/acme-claude' })
  assert.equal(wrapperUpdate(a, 'globex'), null, 'it starts what its plugin offers')
  assert.equal(wrapperUpdate(a, 'other'), null)
  assert.equal(wrapperUpdate(undefined, 'acme'), null)
})

test('the form says each step: checking, found where with what version, refused why, added, updated', () => {
  assert.equal(wrapperState(null), null)
  assert.equal(wrapperState({ busy: true }).kind, 'busy')
  const ok = wrapperState({ checked: true, command: 'acme-claude', path: '/opt/acme/bin/acme-claude', version: '2.1.400 (Claude Code)' })
  assert.deepEqual([ok.kind, ok.text, ok.command], ['ok', 'Found /opt/acme/bin/acme-claude — 2.1.400 (Claude Code)', 'acme-claude'])
  const old = wrapperState({ checked: true, command: 'acme-old', path: '/opt/acme/bin/acme-old', version: '2.1.281', warning: 'Claude Code 2.1.281 is older than Echo asks for: … — update it to 2.1.323 or later' })
  assert.deepEqual([old.kind, old.text], ['ok', 'Found /opt/acme/bin/acme-old — 2.1.281. Claude Code 2.1.281 is older than Echo asks for: … — update it to 2.1.323 or later'], 'an older Claude Code can be added: the form says what it lacks')
  assert.deepEqual(wrapperState({ error: "/x/node doesn't answer --version as Claude Code does" }), { kind: 'error', text: "/x/node doesn't answer --version as Claude Code does" })
  assert.equal(wrapperState({ added: true, name: 'Acme Claude' }).text, 'Added Acme Claude')
  assert.equal(wrapperState({ added: true, name: 'Acme Claude', path: '/opt/acme/bin/acme-claude' }).text, 'Added Acme Claude — it starts /opt/acme/bin/acme-claude')
  assert.equal(wrapperState({ updated: true, id: 'acme', path: '/opt/acme/bin/acme-claude2' }).text, 'It now starts /opt/acme/bin/acme-claude2')
  assert.match(WRAPPER_WARNING, /sees and can change everything Echo sends to Claude, call transcripts included/)
})

let failed = 0
for (const [name, f] of tests) {
  try {
    f()
    console.log(`ok - ${name}`)
  } catch (e) {
    failed++
    console.log(`not ok - ${name}\n  ${e.message}`)
  }
}
if (failed) process.exit(1)
